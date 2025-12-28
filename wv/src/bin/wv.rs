use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{Parser, Subcommand};
use werkverzeichnis::{
	add_composition, build_index, collection_path_from_id, expand_title, format_catalog,
	generate_id, load_catalog_def, load_collection, load_composer, load_composition,
	merge_attribution_with_collections, resolve_data_dir, resolve_editor, scaffold_composition,
	sort_key, sort_numbers, validate_all, validate_file, write_composer_index,
	write_edition_indexes, write_index, Config, ExpansionContext,
};

#[derive(Parser)]
#[command(name = "wv")]
#[command(about = "Werkverzeichnis - Classical music catalog tools")]
#[command(version)]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand)]
enum Commands {
	/// Parse a composition file
	ParseComposition {
		path: PathBuf,
	},

	/// Parse a composer file
	ParseComposer {
		path: PathBuf,
	},

	/// Parse a collection file
	ParseCollection {
		path: PathBuf,
	},

	/// Sort catalog numbers from stdin
	Sort {
		scheme: String,
		#[arg(long)]
		composer: Option<String>,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	/// Show sort key for a catalog number
	SortKey {
		scheme: String,
		number: String,
		#[arg(long)]
		composer: Option<String>,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	/// Show merged attribution for a composition
	Merge {
		path: PathBuf,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	/// Build index files
	Index {
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	/// Get compositions by ID or catalog number
	Get {
		#[arg(help = "Composer slug, or composition ID(s)")]
		target: Option<String>,
		#[arg(help = "Catalog scheme (e.g., bwv, op)")]
		scheme: Option<String>,
		#[arg(help = "Catalog number or range (e.g., 812, 2-10)")]
		number: Option<String>,
		#[arg(long)]
		edition: Option<String>,
		#[arg(long, help = "Filter to a group (e.g., op 2 includes 2, 2/1, 2/2)")]
		group: Option<String>,
		#[arg(long)]
		sorted: bool,
		#[arg(short, long, help = "Terse output (scheme:number and ID only)")]
		terse: bool,
		#[arg(short, long, help = "Show movement structure")]
		movements: bool,
		#[arg(long, help = "Full JSON output")]
		json: bool,
		#[arg(short, long, help = "Quiet mode (suppress messages)")]
		quiet: bool,
		#[arg(short, long, help = "Open in editor")]
		edit: bool,
		#[arg(long, help = "Read IDs from stdin")]
		stdin: bool,
		#[arg(long, help = "Only match current catalog numbers (no superseded)")]
		strict: bool,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	/// Format JSON input as pretty output
	Format {
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	/// Validate composition file(s)
	Validate {
		path: Option<PathBuf>,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	/// Add composition to repository
	Add {
		path: PathBuf,
		#[arg(short, long)]
		force: bool,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	/// Create new composition scaffold
	New {
		form: String,
		composer: String,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	/// Generate a new random ID
	Id,

	/// List compositions in a collection
	Collection {
		id: String,
		#[arg(long, help = "Verify all members exist in index")]
		verify: bool,
		#[arg(long, help = "Show full composition details")]
		hydrate: bool,
		#[arg(short, long, help = "Terse output (scheme:number only)")]
		terse: bool,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	/// Find collections containing a composition
	Collections {
		#[arg(help = "Composition ID or catalog number (e.g., bwv:812)")]
		query: String,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},
}

fn main() {
	let cli = Cli::parse();

	match cli.command {
		Commands::ParseComposition { path } => cmd_parse_composition(&path),
		Commands::ParseComposer { path } => cmd_parse_composer(&path),
		Commands::ParseCollection { path } => cmd_parse_collection(&path),
		Commands::Sort { scheme, composer, data_dir } => {
			cmd_sort(&scheme, composer.as_deref(), data_dir)
		}
		Commands::SortKey { scheme, number, composer, data_dir } => {
			cmd_sort_key(&scheme, &number, composer.as_deref(), data_dir)
		}
		Commands::Merge { path, data_dir } => cmd_merge(&path, data_dir),
		Commands::Index { data_dir } => cmd_index(data_dir),
		Commands::Get { target, scheme, number, edition, group, sorted, terse, movements, json, quiet, edit, stdin, strict, data_dir } => {
			cmd_get(target.as_deref(), scheme.as_deref(), number.as_deref(), edition.as_deref(), group.as_deref(), sorted, terse, movements, json, quiet, edit, stdin, strict, data_dir)
		}
		Commands::Format { data_dir } => cmd_format(data_dir),
		Commands::Validate { path, data_dir } => cmd_validate(path.as_deref(), data_dir),
		Commands::Add { path, force, data_dir } => cmd_add(&path, force, data_dir),
		Commands::New { form, composer, data_dir } => cmd_new(&form, &composer, data_dir),
		Commands::Id => cmd_id(),
		Commands::Collection { id, verify, hydrate, terse, data_dir } => {
			cmd_collection(&id, verify, hydrate, terse, data_dir)
		}
		Commands::Collections { query, data_dir } => cmd_collections(&query, data_dir),
	}
}

fn find_data_dir(specified: Option<&PathBuf>) -> PathBuf {
	if let Some(dir) = specified {
		return dir.clone();
	}

	let current = std::env::current_dir().unwrap_or_default();
	if current.join("composers").exists() {
		return current;
	}

	let parent = current.parent().map(|p| p.to_path_buf()).unwrap_or(current.clone());
	if parent.join("composers").exists() {
		return parent;
	}

	current
}

fn cmd_parse_composition(path: &PathBuf) {
	match load_composition(path) {
		Ok(comp) => println!("{:#?}", comp),
		Err(e) => {
			eprintln!("Error: {}", e);
			std::process::exit(1);
		}
	}
}

fn cmd_parse_composer(path: &PathBuf) {
	match load_composer(path) {
		Ok(comp) => println!("{:#?}", comp),
		Err(e) => {
			eprintln!("Error: {}", e);
			std::process::exit(1);
		}
	}
}

fn cmd_parse_collection(path: &PathBuf) {
	match load_collection(path) {
		Ok(coll) => println!("{:#?}", coll),
		Err(e) => {
			eprintln!("Error: {}", e);
			std::process::exit(1);
		}
	}
}

fn cmd_sort(scheme: &str, composer: Option<&str>, data_dir: Option<PathBuf>) {
	let data_dir = find_data_dir(data_dir.as_ref());
	let defn = load_catalog_def(&data_dir, scheme, composer);

	let stdin = io::stdin();
	let mut numbers: Vec<String> = stdin
		.lock()
		.lines()
		.map_while(Result::ok)
		.filter(|s| !s.trim().is_empty())
		.map(|s| s.trim().to_string())
		.collect();

	sort_numbers(&mut numbers, defn.as_ref());

	for n in numbers {
		println!("{}", n);
	}
}

fn cmd_sort_key(scheme: &str, number: &str, composer: Option<&str>, data_dir: Option<PathBuf>) {
	let data_dir = find_data_dir(data_dir.as_ref());

	let defn = match load_catalog_def(&data_dir, scheme, composer) {
		Some(d) => d,
		None => {
			eprintln!("Unknown catalog: {}", scheme);
			std::process::exit(1);
		}
	};

	let key = sort_key(number, &defn);
	println!("{:?}", key);
}

fn cmd_merge(path: &PathBuf, data_dir: Option<PathBuf>) {
	let data_dir = find_data_dir(data_dir.as_ref());
	let collections_dir = data_dir.join("collections");

	let comp = match load_composition(path) {
		Ok(c) => c,
		Err(e) => {
			eprintln!("Error loading composition: {}", e);
			std::process::exit(1);
		}
	};

	let merged = merge_attribution_with_collections(&comp.attribution, &collections_dir);

	println!("ID: {}", comp.id);
	println!("Form: {}", comp.form);
	if let Some(key) = &comp.key {
		println!("Key: {}", key);
	}
	println!();
	println!("Merged attribution:");
	if let Some(composer) = &merged.composer {
		println!("  Composer: {}", composer);
	}
	if let Some(composed) = merged.dates.composed {
		println!("  Composed: {}", composed);
	}
	if let Some(published) = merged.dates.published {
		println!("  Published: {}", published);
	}
	if let Some(status) = &merged.status {
		println!("  Status: {:?}", status);
	}
	if !merged.catalog.is_empty() {
		println!("  Catalog entries:");
		for cat in &merged.catalog {
			let edition_str = cat.edition.as_ref().map(|e| format!(" (ed. {})", e)).unwrap_or_default();
			println!("    {}:{}{}", cat.scheme, cat.number, edition_str);
		}
	}
	if !merged.notes.is_empty() {
		println!("  Notes:");
		for note in &merged.notes {
			println!("    - {}", note);
		}
	}
}

fn cmd_index(data_dir: Option<PathBuf>) {
	let data_dir = find_data_dir(data_dir.as_ref());

	println!("Building index from {:?}...", data_dir);

	let index = build_index(&data_dir);

	let mut total_compositions = 0;
	let mut total_catalog_entries = 0;

	for ids in index.by_composer.values() {
		total_compositions += ids.len();
	}

	for schemes in index.catalog.values() {
		for scheme_index in schemes.values() {
			total_catalog_entries += scheme_index.current.len() + scheme_index.superseded.len();
		}
	}

	println!("Found {} compositions", total_compositions);
	println!("Found {} catalog entries", total_catalog_entries);

	let indexes_dir = data_dir.join(".indexes");
	if let Err(e) = std::fs::create_dir_all(&indexes_dir) {
		eprintln!("Error creating .indexes directory: {}", e);
		std::process::exit(1);
	}

	let index_path = indexes_dir.join("index.json");
	let composer_path = indexes_dir.join("composer-index.json");
	let editions_dir = indexes_dir.join("editions");

	if let Err(e) = write_index(&index, &index_path) {
		eprintln!("Error writing index: {}", e);
		std::process::exit(1);
	}
	println!("Wrote {}", index_path.display());

	if let Err(e) = write_composer_index(&index, &composer_path) {
		eprintln!("Error writing composer index: {}", e);
		std::process::exit(1);
	}
	println!("Wrote {}", composer_path.display());

	if !index.editions.is_empty() {
		if let Err(e) = write_edition_indexes(&index, &editions_dir) {
			eprintln!("Error writing edition indexes: {}", e);
			std::process::exit(1);
		}
		println!("Wrote edition indexes to {}", editions_dir.display());
	}

	println!("Done.");
}

fn is_composition_id(s: &str) -> bool {
	s.len() == 8 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn id_to_path(data_dir: &Path, id: &str) -> PathBuf {
	data_dir
		.join("compositions")
		.join(&id[..2])
		.join(format!("{}.json", &id[2..]))
}

fn cmd_get(
	target: Option<&str>,
	scheme: Option<&str>,
	number: Option<&str>,
	edition: Option<&str>,
	group: Option<&str>,
	sorted: bool,
	terse: bool,
	movements: bool,
	json: bool,
	quiet: bool,
	edit: bool,
	stdin: bool,
	strict: bool,
	data_dir: Option<PathBuf>,
) {
	let config = Config::load();
	let data_dir = resolve_data_dir(data_dir.as_ref(), &config);

	// Collect IDs from stdin if requested
	if stdin {
		let ids: Vec<String> = io::stdin()
			.lock()
			.lines()
			.filter_map(|l| l.ok())
			.map(|l| l.trim().to_string())
			.filter(|l| !l.is_empty())
			.collect();

		if ids.is_empty() {
			if !quiet {
				eprintln!("No IDs provided.");
			}
			return;
		}

		output_by_ids(&ids, &data_dir, &config, terse, movements, json, edit);
		return;
	}

	// Must have a target
	let target = match target {
		Some(t) => t,
		None => {
			eprintln!("Usage: wv get <composer> [scheme] [number]");
			eprintln!("       wv get <id> [id...]");
			eprintln!("       wv get --stdin");
			std::process::exit(1);
		}
	};

	// Check if target is an ID (or multiple IDs via positional args)
	if is_composition_id(target) {
		// Collect any additional IDs from scheme/number positions
		let mut ids = vec![target.to_string()];
		if let Some(s) = scheme {
			if is_composition_id(s) {
				ids.push(s.to_string());
			}
		}
		if let Some(n) = number {
			if is_composition_id(n) {
				ids.push(n.to_string());
			}
		}

		output_by_ids(&ids, &data_dir, &config, terse, movements, json, edit);
		return;
	}

	// Otherwise, target is a composer
	let composer = target;

	// Check for implicit range in number (e.g., "2-10")
	let (number, range) = if let Some(n) = number {
		if let Some((start, end)) = n.split_once('-') {
			// Make sure both sides look like numbers (not just a hyphenated number)
			let start_valid = start.chars().next().map_or(false, |c| c.is_ascii_digit());
			let end_valid = end.chars().next().map_or(false, |c| c.is_ascii_digit());
			if start_valid && end_valid {
				(None, Some((start, end)))
			} else {
				(Some(n), None)
			}
		} else {
			(Some(n), None)
		}
	} else {
		(None, None)
	};

	// Require scheme for range/group queries
	if (range.is_some() || group.is_some()) && scheme.is_none() {
		eprintln!("Error: range and group queries require a catalog scheme");
		eprintln!("Usage: wv get <composer> <scheme> <range>");
		std::process::exit(1);
	}

	let index = build_index(&data_dir);

	let mut builder = index.query().composer(composer).data_dir(&data_dir);

	if let Some(s) = scheme {
		builder = builder.scheme(s);
	}

	if let Some(n) = number {
		builder = builder.number(n);
	}

	if let Some(e) = edition {
		builder = builder.edition(e);
	}

	if let Some(g) = group {
		builder = builder.group(g);
	}

	if let Some((start, end)) = range {
		builder = builder.range(start.trim(), end.trim());
	}

	if sorted || group.is_some() || range.is_some() {
		builder = builder.sorted(&data_dir);
	}

	builder = builder.strict(strict);

	let results = builder.fetch();

	if results.is_empty() {
		if !quiet {
			eprintln!("No results found.");
		}
		return;
	}

	// Emit warnings for superseded results
	if !quiet {
		for r in &results {
			if r.superseded {
				if let (Some(num), Some(current)) = (&r.number, &r.current_number) {
					let scheme_upper = scheme.map(|s| s.to_uppercase()).unwrap_or_default();
					eprintln!(
						"warning: {} {} is superseded (current: {})",
						scheme_upper, num, current
					);
				}
			}
		}
	}

	// Handle --edit
	if edit {
		let paths: Vec<PathBuf> = results
			.iter()
			.map(|r| id_to_path(&data_dir, &r.id))
			.collect();
		open_in_editor(&config, &paths);
		return;
	}

	let catalog_defn = scheme.and_then(|s| load_catalog_def(&data_dir, s, Some(composer)));
	let multi = results.len() > 1;

	if json {
		let mut output: Vec<serde_json::Value> = Vec::new();

		for r in &results {
			let comp_path = id_to_path(&data_dir, &r.id);
			if let Ok(comp) = load_composition(&comp_path) {
				output.push(serde_json::to_value(&comp).unwrap_or(serde_json::Value::Null));
			}
		}

		if output.len() == 1 {
			println!("{}", serde_json::to_string_pretty(&output[0]).unwrap());
		} else {
			println!("{}", serde_json::to_string_pretty(&output).unwrap());
		}
	} else if movements {
		for r in &results {
			let comp_path = id_to_path(&data_dir, &r.id);

			if let Ok(comp) = load_composition(&comp_path) {
				if multi {
					let header = match (&r.number, scheme) {
						(Some(n), Some(s)) => format_catalog(s, n, catalog_defn.as_ref()),
						(Some(n), None) => n.clone(),
						(None, _) => r.id.clone(),
					};
					println!("{}:", header);
				}

				if let Some(mvmts) = &comp.movements {
					for (i, mvmt) in mvmts.iter().enumerate() {
						let mvmt_title = mvmt.title.as_deref()
							.or(mvmt.form.as_deref())
							.unwrap_or("?");
						let prefix = if multi { "  " } else { "" };
						println!("{}{}. {}", prefix, i + 1, mvmt_title);
					}
				} else if let Some(sects) = &comp.sections {
					for (i, sect) in sects.iter().enumerate() {
						let sect_title = sect.title.as_deref()
							.or(sect.form.as_deref())
							.unwrap_or("?");
						let prefix = if multi { "  " } else { "" };
						println!("{}{}. {}", prefix, i + 1, sect_title);
					}
				}

				if multi {
					println!();
				}
			}
		}
	} else if terse {
		for r in results {
			match (&r.number, scheme) {
				(Some(n), Some(s)) => println!("{}:{}\t{}", s, n, r.id),
				(Some(n), None) => println!("{}\t{}", n, r.id),
				(None, _) => println!("{}", r.id),
			}
		}
	} else {
		// Default: pretty output
		for r in results {
			let comp_path = id_to_path(&data_dir, &r.id);

			if let Ok(comp) = load_composition(&comp_path) {
				let ctx = ExpansionContext {
					composition: &comp,
					collection: None,
					position_in_collection: None,
					config: &config.display,
				};
				let title = expand_title(&ctx);
				match (&r.number, scheme) {
					(Some(n), Some(s)) => {
						let formatted = format_catalog(s, n, catalog_defn.as_ref());
						println!("{}, {}", title, formatted);
					}
					(Some(n), None) => println!("{}, {}", title, n),
					(None, _) => println!("{} [{}]", title, r.id),
				}
			} else {
				match (&r.number, scheme) {
					(Some(n), Some(s)) => {
						let formatted = format_catalog(s, n, catalog_defn.as_ref());
						println!("{}", formatted);
					}
					(Some(n), None) => println!("{}", n),
					(None, _) => println!("{}", r.id),
				}
			}
		}
	}
}

fn output_by_ids(
	ids: &[String],
	data_dir: &Path,
	config: &Config,
	terse: bool,
	movements: bool,
	json: bool,
	edit: bool,
) {
	if edit {
		let paths: Vec<PathBuf> = ids.iter().map(|id| id_to_path(data_dir, id)).collect();
		open_in_editor(config, &paths);
		return;
	}

	let multi = ids.len() > 1;

	if json {
		let mut output: Vec<serde_json::Value> = Vec::new();
		for id in ids {
			let comp_path = id_to_path(data_dir, id);
			if let Ok(comp) = load_composition(&comp_path) {
				output.push(serde_json::to_value(&comp).unwrap_or(serde_json::Value::Null));
			}
		}
		if output.len() == 1 {
			println!("{}", serde_json::to_string_pretty(&output[0]).unwrap());
		} else {
			println!("{}", serde_json::to_string_pretty(&output).unwrap());
		}
	} else if movements {
		for id in ids {
			let comp_path = id_to_path(data_dir, id);
			if let Ok(comp) = load_composition(&comp_path) {
				if multi {
					let header = format_id_header(&comp, id, data_dir);
					println!("{}:", header);
				}

				if let Some(mvmts) = &comp.movements {
					for (i, mvmt) in mvmts.iter().enumerate() {
						let mvmt_title = mvmt.title.as_deref()
							.or(mvmt.form.as_deref())
							.unwrap_or("?");
						let prefix = if multi { "  " } else { "" };
						println!("{}{}. {}", prefix, i + 1, mvmt_title);
					}
				} else if let Some(sects) = &comp.sections {
					for (i, sect) in sects.iter().enumerate() {
						let sect_title = sect.title.as_deref()
							.or(sect.form.as_deref())
							.unwrap_or("?");
						let prefix = if multi { "  " } else { "" };
						println!("{}{}. {}", prefix, i + 1, sect_title);
					}
				}

				if multi {
					println!();
				}
			}
		}
	} else if terse {
		for id in ids {
			let comp_path = id_to_path(data_dir, id);
			if let Ok(comp) = load_composition(&comp_path) {
				if let Some(cat) = first_catalog_entry(&comp) {
					println!("{}:{}\t{}", cat.scheme, cat.number, id);
				} else {
					println!("{}", id);
				}
			} else {
				println!("{}", id);
			}
		}
	} else {
		// Default: pretty output (same format as catalog-based queries)
		for id in ids {
			let comp_path = id_to_path(data_dir, id);
			if let Ok(comp) = load_composition(&comp_path) {
				let ctx = ExpansionContext {
					composition: &comp,
					collection: None,
					position_in_collection: None,
					config: &config.display,
				};
				let title = expand_title(&ctx);

				if let Some(cat) = first_catalog_entry(&comp) {
					let catalog_defn = load_catalog_def(
						data_dir,
						&cat.scheme,
						comp.attribution.first().and_then(|a| a.composer.as_deref()),
					);
					let formatted = format_catalog(&cat.scheme, &cat.number, catalog_defn.as_ref());
					println!("{}, {}", title, formatted);
				} else {
					println!("{} [{}]", title, id);
				}
			} else {
				eprintln!("Not found: {}", id);
			}
		}
	}
}

fn first_catalog_entry(comp: &werkverzeichnis::Composition) -> Option<&werkverzeichnis::CatalogEntry> {
	comp.attribution
		.first()
		.and_then(|a| a.catalog.as_ref())
		.and_then(|c| c.first())
}

fn format_id_header(comp: &werkverzeichnis::Composition, id: &str, data_dir: &Path) -> String {
	if let Some(cat) = first_catalog_entry(comp) {
		let catalog_defn = load_catalog_def(
			data_dir,
			&cat.scheme,
			comp.attribution.first().and_then(|a| a.composer.as_deref()),
		);
		format_catalog(&cat.scheme, &cat.number, catalog_defn.as_ref())
	} else {
		id.to_string()
	}
}

fn open_in_editor(config: &Config, paths: &[PathBuf]) {
	let editor = resolve_editor(config);
	let path_strs: Vec<&str> = paths.iter().filter_map(|p| p.to_str()).collect();

	let status = Command::new(&editor)
		.args(&path_strs)
		.status();

	match status {
		Ok(s) if !s.success() => {
			eprintln!("Editor exited with status: {}", s);
		}
		Err(e) => {
			eprintln!("Failed to open editor '{}': {}", editor, e);
			std::process::exit(1);
		}
		_ => {}
	}
}

fn cmd_format(data_dir: Option<PathBuf>) {
	let config = Config::load();
	let data_dir = resolve_data_dir(data_dir.as_ref(), &config);

	let input: String = io::stdin()
		.lock()
		.lines()
		.filter_map(|l| l.ok())
		.collect::<Vec<_>>()
		.join("\n");

	if input.trim().is_empty() {
		return;
	}

	// Try parsing as array first, then as single object
	let compositions: Vec<werkverzeichnis::Composition> = 
		if let Ok(arr) = serde_json::from_str::<Vec<werkverzeichnis::Composition>>(&input) {
			arr
		} else if let Ok(comp) = serde_json::from_str::<werkverzeichnis::Composition>(&input) {
			vec![comp]
		} else {
			eprintln!("Error: Invalid JSON input");
			std::process::exit(1);
		};

	for comp in &compositions {
		let ctx = ExpansionContext {
			composition: comp,
			collection: None,
			position_in_collection: None,
			config: &config.display,
		};
		let title = expand_title(&ctx);

		// Try to get catalog info from first attribution
		if let Some(attr) = comp.attribution.first() {
			if let Some(cat) = attr.catalog.as_ref().and_then(|c| c.first()) {
				let catalog_defn = load_catalog_def(&data_dir, &cat.scheme, attr.composer.as_deref());
				let formatted = format_catalog(&cat.scheme, &cat.number, catalog_defn.as_ref());
				println!("{}, {}", title, formatted);
				continue;
			}
		}
		println!("{} [{}]", title, comp.id);
	}
}

fn cmd_validate(path: Option<&Path>, data_dir: Option<PathBuf>) {
	let data_dir = find_data_dir(data_dir.as_ref());

	let errors = if let Some(p) = path {
		validate_file(p, &data_dir)
	} else {
		println!("Validating all compositions in {:?}...", data_dir);
		validate_all(&data_dir)
	};

	if errors.is_empty() {
		println!("No validation errors found.");
	} else {
		eprintln!("Found {} validation error(s):", errors.len());
		for err in &errors {
			eprintln!("  {}", err);
		}
		std::process::exit(1);
	}
}

fn cmd_add(path: &PathBuf, force: bool, data_dir: Option<PathBuf>) {
	let data_dir = find_data_dir(data_dir.as_ref());

	match add_composition(path, &data_dir, force) {
		Ok(result) => {
			println!("Added {} -> {}", result.source.display(), result.destination.display());
			println!("ID: {}", result.id);
		}
		Err(e) => {
			eprintln!("Error: {}", e);
			std::process::exit(1);
		}
	}
}

fn cmd_new(form: &str, composer: &str, data_dir: Option<PathBuf>) {
	let data_dir = find_data_dir(data_dir.as_ref());

	let id = generate_id();
	let json = scaffold_composition(&id, form, composer);

	let prefix = &id[..2];
	let suffix = &id[2..];
	let dest_dir = data_dir.join("compositions").join(prefix);
	let dest_path = dest_dir.join(format!("{}.json", suffix));

	if let Err(e) = std::fs::create_dir_all(&dest_dir) {
		eprintln!("Error creating directory: {}", e);
		std::process::exit(1);
	}

	if let Err(e) = std::fs::write(&dest_path, &json) {
		eprintln!("Error writing file: {}", e);
		std::process::exit(1);
	}

	println!("Created {}", dest_path.display());
	println!("ID: {}", id);
}

fn cmd_id() {
	println!("{}", generate_id());
}

fn cmd_collection(id: &str, verify: bool, hydrate: bool, terse: bool, data_dir: Option<PathBuf>) {
	let config = Config::load();
	let data_dir = resolve_data_dir(data_dir.as_ref(), &config);
	let collections_dir = data_dir.join("collections");
	let path = collection_path_from_id(&collections_dir, id);

	let collection = match load_collection(&path) {
		Ok(c) => c,
		Err(e) => {
			eprintln!("Error loading collection: {}", e);
			std::process::exit(1);
		}
	};

	let index = if verify || hydrate || !terse {
		Some(build_index(&data_dir))
	} else {
		None
	};

	let composer = collection.composer.as_deref().unwrap_or_else(|| {
		id.split_once('-').map(|(c, _)| c).unwrap_or(id)
	});

	let catalog_defn = if !terse {
		load_catalog_def(&data_dir, &collection.scheme, Some(composer))
	} else {
		None
	};

	let mut missing = Vec::new();

	for (i, num) in collection.compositions.iter().enumerate() {
		let position = i + 1;

		if verify || hydrate {
			let idx = index.as_ref().unwrap();
			let found = idx
				.query()
				.composer(composer)
				.scheme(&collection.scheme)
				.number(num)
				.fetch_one();

			if let Some(comp_id) = found {
				let comp_path = data_dir
					.join("compositions")
					.join(&comp_id[..2])
					.join(format!("{}.json", &comp_id[2..]));

				if hydrate {
					if let Ok(comp) = load_composition(&comp_path) {
						println!("{}:{} [{}]", collection.scheme, num, comp_id);
						println!("  Form: {}", comp.form);
						if let Some(key) = &comp.key {
							println!("  Key: {}", key);
						}
					} else {
						println!("{}:{} [{}] (file not found)", collection.scheme, num, comp_id);
					}
				} else {
					// verify mode
					println!("{}:{} ✓", collection.scheme, num);
				}
			} else {
				missing.push(num.clone());
				println!("{}:{} ✗ NOT FOUND", collection.scheme, num);
			}
		} else if terse {
			println!("{}:{}", collection.scheme, num);
		} else {
			// Default: pretty output
			let idx = index.as_ref().unwrap();
			let found = idx
				.query()
				.composer(composer)
				.scheme(&collection.scheme)
				.number(num)
				.fetch_one();

			if let Some(comp_id) = found {
				let comp_path = data_dir
					.join("compositions")
					.join(&comp_id[..2])
					.join(format!("{}.json", &comp_id[2..]));

				let formatted_cat = format_catalog(&collection.scheme, num, catalog_defn.as_ref());
				if let Ok(comp) = load_composition(&comp_path) {
					let ctx = ExpansionContext {
						composition: &comp,
						collection: Some(&collection),
						position_in_collection: Some(position),
						config: &config.display,
					};
					let title = expand_title(&ctx);
					println!("{}, {}", title, formatted_cat);
				} else {
					println!("{}", formatted_cat);
				}
			} else {
				let formatted_cat = format_catalog(&collection.scheme, num, catalog_defn.as_ref());
				println!("{} (not found)", formatted_cat);
			}
		}
	}

	if verify && !missing.is_empty() {
		eprintln!();
		eprintln!("Missing {} composition(s)", missing.len());
		std::process::exit(1);
	}
}

fn cmd_collections(query: &str, data_dir: Option<PathBuf>) {
	let config = Config::load();
	let data_dir = resolve_data_dir(data_dir.as_ref(), &config);
	let collections_dir = data_dir.join("collections");

	// Parse query: either composition ID or scheme:number
	let (scheme, number) = if let Some((s, n)) = query.split_once(':') {
		(Some(s), Some(n))
	} else {
		(None, None)
	};

	let mut found = Vec::new();

	// Scan all collection files
	if let Ok(composer_dirs) = std::fs::read_dir(&collections_dir) {
		for composer_entry in composer_dirs.flatten() {
			if !composer_entry.path().is_dir() {
				continue;
			}

			if let Ok(coll_files) = std::fs::read_dir(composer_entry.path()) {
				for file_entry in coll_files.flatten() {
					let path = file_entry.path();
					if path.extension().map_or(true, |e| e != "json") {
						continue;
					}

					if let Ok(coll) = load_collection(&path) {
						let matches = if let (Some(s), Some(n)) = (scheme, number) {
							coll.scheme == s && coll.compositions.contains(&n.to_string())
						} else {
							// Query is a composition ID - need to check index
							// For now, just check if any composition matches by ID
							false // TODO: implement ID lookup
						};

						if matches {
							found.push(coll.id.clone());
						}
					}
				}
			}
		}
	}

	if found.is_empty() {
		println!("No collections found containing '{}'", query);
	} else {
		println!("Collections containing '{}':", query);
		for id in found {
			println!("  {}", id);
		}
	}
}

use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use werkverzeichnis::{
	add_composition, build_index, collection_path_from_id, generate_id, load_catalog_def,
	load_collection, load_composer, load_composition, merge_attribution_with_collections,
	scaffold_composition, sort_key, sort_numbers, validate_all, validate_file,
	write_composer_index, write_edition_indexes, write_index,
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

	/// Query the index
	Query {
		composer: String,
		scheme: Option<String>,
		number: Option<String>,
		#[arg(long)]
		edition: Option<String>,
		#[arg(long, help = "Filter to a group (e.g., op 2 includes 2, 2/1, 2/2)")]
		group: Option<String>,
		#[arg(long, value_name = "START-END", help = "Filter to range (e.g., 2-10)")]
		range: Option<String>,
		#[arg(long)]
		sorted: bool,
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
		Commands::Query { composer, scheme, number, edition, group, range, sorted, data_dir } => {
			cmd_query(&composer, scheme.as_deref(), number.as_deref(), edition.as_deref(), group.as_deref(), range.as_deref(), sorted, data_dir)
		}
		Commands::Validate { path, data_dir } => cmd_validate(path.as_deref(), data_dir),
		Commands::Add { path, force, data_dir } => cmd_add(&path, force, data_dir),
		Commands::New { form, composer, data_dir } => cmd_new(&form, &composer, data_dir),
		Commands::Id => cmd_id(),
		Commands::Collection { id, verify, hydrate, data_dir } => {
			cmd_collection(&id, verify, hydrate, data_dir)
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
		for numbers in schemes.values() {
			total_catalog_entries += numbers.len();
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

fn cmd_query(
	composer: &str,
	scheme: Option<&str>,
	number: Option<&str>,
	edition: Option<&str>,
	group: Option<&str>,
	range: Option<&str>,
	sorted: bool,
	data_dir: Option<PathBuf>,
) {
	let data_dir = find_data_dir(data_dir.as_ref());

	// Require explicit scheme for range/group queries
	if (range.is_some() || group.is_some()) && scheme.is_none() {
		eprintln!("Error: --range and --group require a catalog scheme");
		eprintln!("Usage: wv query <composer> <scheme> [--range START-END]");
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

	if let Some(r) = range {
		if let Some((start, end)) = r.split_once('-') {
			builder = builder.range(start.trim(), end.trim());
		} else {
			eprintln!("Invalid range format. Use START-END (e.g., 2-10)");
			std::process::exit(1);
		}
	}

	if sorted || group.is_some() || range.is_some() {
		builder = builder.sorted(&data_dir);
	}

	let results = builder.fetch();

	if results.is_empty() {
		println!("No results found.");
	} else {
		for r in results {
			match r.number {
				Some(n) => println!("{}\t{}", n, r.id),
				None => println!("{}", r.id),
			}
		}
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

fn cmd_collection(id: &str, verify: bool, hydrate: bool, data_dir: Option<PathBuf>) {
	let data_dir = find_data_dir(data_dir.as_ref());
	let collections_dir = data_dir.join("collections");
	let path = collection_path_from_id(&collections_dir, id);

	let collection = match load_collection(&path) {
		Ok(c) => c,
		Err(e) => {
			eprintln!("Error loading collection: {}", e);
			std::process::exit(1);
		}
	};

	if let Some(en) = collection.title.get("en") {
		println!("{}", en);
	} else if let Some((_, v)) = collection.title.iter().next() {
		println!("{}", v);
	}

	println!("Scheme: {}", collection.scheme);
	println!();

	let index = if verify || hydrate {
		Some(build_index(&data_dir))
	} else {
		None
	};

	let composer = collection.composer.as_deref().unwrap_or_else(|| {
		id.split_once('-').map(|(c, _)| c).unwrap_or(id)
	});

	let mut missing = Vec::new();

	for (i, num) in collection.compositions.iter().enumerate() {
		if verify || hydrate {
			let idx = index.as_ref().unwrap();
			let found = idx
				.query()
				.composer(composer)
				.scheme(&collection.scheme)
				.number(num)
				.fetch_one();

			if let Some(comp_id) = found {
				if hydrate {
					let comp_path = data_dir
						.join("compositions")
						.join(&comp_id[..2])
						.join(format!("{}.json", &comp_id[2..]));

					if let Ok(comp) = load_composition(&comp_path) {
						println!("{}. {}:{} [{}]", i + 1, collection.scheme, num, comp_id);
						println!("   Form: {}", comp.form);
						if let Some(key) = &comp.key {
							println!("   Key: {}", key);
						}
					} else {
						println!("{}. {}:{} [{}] (file not found)", i + 1, collection.scheme, num, comp_id);
					}
				} else {
					println!("  {}. {}:{} ✓", i + 1, collection.scheme, num);
				}
			} else {
				missing.push(num.clone());
				println!("  {}. {}:{} ✗ NOT FOUND", i + 1, collection.scheme, num);
			}
		} else {
			println!("  {}. {}:{}", i + 1, collection.scheme, num);
		}
	}

	if verify && !missing.is_empty() {
		eprintln!();
		eprintln!("Missing {} composition(s)", missing.len());
		std::process::exit(1);
	}
}

fn cmd_collections(query: &str, data_dir: Option<PathBuf>) {
	let data_dir = find_data_dir(data_dir.as_ref());
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

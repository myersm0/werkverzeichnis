use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::catalog::{load_catalog_def, validate_catalog_domain};
use crate::commands::collection;
use crate::config::{resolve_editor, Config};
use crate::display::{expand_title, format_catalog, ExpansionContext};
use crate::index::{get_or_build_index, mark_index_dirty, Index};
use crate::inventory::InventoryLookup;
use crate::output::{
	id_to_path, output_by_ids, output_json, output_movements, output_pretty, output_terse,
	print, OutputContext,
};
use crate::parse::load_composition;
use crate::types::CatalogDefinition;
use crate::xref::{check_duplicates, MbLookup};

pub struct GetArgs {
	pub target: Option<String>,
	pub scheme: Option<String>,
	pub number: Vec<String>,
	pub edition: Option<String>,
	pub group: Option<String>,
	pub sorted: bool,
	pub terse: bool,
	pub movements: bool,
	pub json: bool,
	pub quiet: bool,
	pub edit: bool,
	pub stdin: bool,
	pub strict: bool,
	pub xref: Option<String>,
	pub collection: Option<Vec<String>>,
}

enum Input {
	Stdin(Vec<String>),
	Ids(Vec<String>),
	Query(ComposerQuery),
}
struct ComposerQuery {
	composer: String,
	scheme: Option<String>,
	number: Option<NumberSpec>,
	edition: Option<String>,
	group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NumberSpec {
	Single(String),
	Range { start: String, end: String },
}

fn is_composition_id(s: &str) -> bool {
	s.len() == 8 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn parse_number_spec(s: &str) -> NumberSpec {
	fn try_split(s: &str) -> Option<(&str, &str)> {
		s.split_once('-').or_else(|| s.split_once(".."))
	}
	if let Some((start, end)) = try_split(s) {
		let looks_like_catalog = |s: &str| {
			let s = s.trim();
			s.chars().next().map_or(false, |c| c.is_ascii_digit())
				|| s.contains(':')
				|| s.chars().next().map_or(false, |c| c.is_ascii_uppercase())
				|| s.chars().next().map_or(false, |c| c.is_ascii_lowercase())
		};
		let start = start.trim();
		let end = end.trim();
		if looks_like_catalog(start) && looks_like_catalog(end) && !end.is_empty() {
			return NumberSpec::Range {
				start: start.to_string(),
				end: end.to_string(),
			};
		}
	}
	NumberSpec::Single(s.to_string())
}

fn find_category<'a>(value: &str, defn: &'a CatalogDefinition) -> Option<&'a str> {
	defn.categories
		.as_ref()?
		.keys()
		.find(|category| category.eq_ignore_ascii_case(value))
		.map(String::as_str)
}

fn normalize_number_piece(value: &str, defn: Option<&CatalogDefinition>) -> String {
	let normalized = value.trim().to_lowercase();
	let Some(defn) = defn else {
		return normalized;
	};

	let mut parts = normalized.split_whitespace();
	let Some(first) = parts.next() else {
		return normalized;
	};
	let rest: Vec<&str> = parts.collect();

	if let Some(category) = find_category(first, defn) {
		let category = category.to_lowercase();
		if rest.is_empty() {
			category
		} else {
			format!("{}:{}", category, rest.join(" "))
		}
	} else {
		normalized
	}
}

fn normalize_number_spec(spec: &NumberSpec, defn: Option<&CatalogDefinition>) -> NumberSpec {
	match spec {
		NumberSpec::Single(number) => NumberSpec::Single(normalize_number_piece(number, defn)),
		NumberSpec::Range { start, end } => {
			let start = normalize_number_piece(start, defn);
			let mut end = normalize_number_piece(end, defn);
			if !end.contains(':') {
				if let (Some(defn), Some((prefix, _))) = (defn, start.rsplit_once(':')) {
					if find_category(prefix, defn).is_some() {
						end = format!("{}:{}", prefix, end);
					}
				}
			}
			NumberSpec::Range { start, end }
		}
	}
}

fn category_for_spec(spec: &NumberSpec, defn: &CatalogDefinition) -> Option<String> {
	let value = match spec {
		NumberSpec::Single(number) => number,
		NumberSpec::Range { start, .. } => start,
	};
	let prefix = value.split_once(':').map_or(value.as_str(), |(prefix, _)| prefix);
	find_category(prefix, defn).map(str::to_lowercase)
}

fn resolve_input(args: &GetArgs) -> Option<Input> {
	if args.stdin {
		let mut ids = Vec::new();
		for line in io::stdin().lock().lines() {
			let Ok(line) = line else {
				continue;
			};
			let line = line.trim();
			if line.is_empty() {
				continue;
			}
			if is_composition_id(line) {
				ids.push(line.to_string());
			} else if !args.quiet {
				eprintln!("warning: ignoring malformed composition ID: {}", line);
			}
		}
		return Some(Input::Stdin(ids));
	}

	let target = args.target.as_ref()?;
	if is_composition_id(target) {
		let mut ids = vec![target.clone()];
		if let Some(s) = &args.scheme {
			if is_composition_id(s) {
				ids.push(s.clone());
			}
		}
		for n in &args.number {
			if is_composition_id(n) {
				ids.push(n.clone());
			}
		}
		return Some(Input::Ids(ids));
	}

	let number_spec = if args.number.is_empty() {
		None
	} else {
		Some(parse_number_spec(&args.number.join(" ")))
	};
	Some(Input::Query(ComposerQuery {
		composer: target.clone(),
		scheme: args.scheme.clone(),
		number: number_spec,
		edition: args.edition.clone(),
		group: args.group.clone(),
	}))
}

fn open_in_editor(config: &Config, paths: &[PathBuf], data_dir: &Path) {
	let editor = resolve_editor(config);
	let path_strs: Vec<&str> = paths.iter().filter_map(|p| p.to_str()).collect();

	let status = Command::new(&editor).args(&path_strs).status();
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

	if let Err(e) = mark_index_dirty(data_dir) {
		eprintln!("warning: failed to mark index stale after edit: {}", e);
	}
}

pub fn run(args: GetArgs, data_dir: PathBuf, config: &Config) {
	if let Some(collection_ids) = &args.collection {
		run_collections(collection_ids, &args, &data_dir, config);
		return;
	}

	let input = match resolve_input(&args) {
		Some(i) => i,
		None => {
			eprintln!("Usage: wv get <composer> [scheme] [number...]");
			eprintln!("       wv get <id> [id...]");
			eprintln!("       wv get --stdin");
			eprintln!("       wv get --collection <id>...");
			std::process::exit(1);
		}
	};
	match input {
		Input::Stdin(ids) | Input::Ids(ids) => {
			if ids.is_empty() {
				if !args.quiet {
					eprintln!("No IDs provided.");
				}
				return;
			}
			if args.edit {
				let paths: Vec<PathBuf> = ids.iter().map(|id| id_to_path(&data_dir, id)).collect();
				open_in_editor(config, &paths, &data_dir);
			} else {
				output_by_ids(&ids, &data_dir, config, args.terse, args.movements, args.json);
			}
		}
		Input::Query(query) => {
			run_query(query, &args, &data_dir, config);
		}
	}
}

fn print_query_examples(
	query: &ComposerQuery,
	number_spec: Option<&NumberSpec>,
	index: &Index,
	data_dir: &Path,
	defn: Option<&CatalogDefinition>,
) {
	let Some(scheme) = query.scheme.as_deref() else {
		return;
	};

	eprintln!();
	if let Some(defn) = defn {
		eprintln!("Examples for {} / {}:", query.composer, defn.name);
	} else {
		eprintln!("Examples for {} / {}:", query.composer, scheme);
	}
	eprintln!("  wv get {} {}", query.composer, scheme);

	let requested_category = number_spec
		.and_then(|spec| defn.and_then(|defn| category_for_spec(spec, defn)));

	let samples = index
		.query()
		.composer(&query.composer)
		.scheme(scheme)
		.data_dir(data_dir)
		.strict(true)
		.sorted(data_dir)
		.fetch();

	if let Some(category) = requested_category {
		eprintln!("  wv get {} {} {}", query.composer, scheme, category);
		let prefix = format!("{}:", category);
		let numbers: Vec<String> = samples
			.into_iter()
			.filter_map(|result| result.number)
			.filter(|number| number.starts_with(&prefix))
			.take(3)
			.collect();
		if let Some(first) = numbers.first() {
			if let Some((_, number)) = first.split_once(':') {
				eprintln!("  wv get {} {} {} {}", query.composer, scheme, category, number);
			}
		}
		if numbers.len() >= 2 {
			let start = numbers[0].split_once(':').map(|(_, number)| number);
			let end = numbers[numbers.len() - 1].split_once(':').map(|(_, number)| number);
			if let (Some(start), Some(end)) = (start, end) {
				eprintln!(
					"  wv get {} {} {} {}-{}",
					query.composer, scheme, category, start, end
				);
			}
		}
		return;
	}

	if let Some(defn) = defn {
		if let Some(examples) = &defn.examples {
			for example in examples.iter().take(3) {
				eprintln!(
					"  wv get {} {} {}",
					query.composer,
					scheme,
					example.number.to_lowercase()
				);
			}
			return;
		}
	}

	for number in samples.into_iter().filter_map(|result| result.number).take(3) {
		eprintln!("  wv get {} {} {}", query.composer, scheme, number);
	}
}

fn inventory_stub_value(query: &ComposerQuery, number: &str) -> serde_json::Value {
	serde_json::json!({
		"composer": query.composer.as_str(),
		"scheme": query.scheme.as_deref().unwrap_or_default(),
		"edition": query.edition.as_deref(),
		"number": number,
		"catalogued": true,
		"populated": false
	})
}

fn output_inventory_stub(
	query: &ComposerQuery,
	number: &str,
	args: &GetArgs,
	defn: Option<&CatalogDefinition>,
) {
	let scheme = query.scheme.as_deref().unwrap_or_default();
	if args.json {
		let values = vec![inventory_stub_value(query, number)];
		print(&serde_json::to_string_pretty(&values).unwrap());
		return;
	}

	print(&format_catalog(scheme, number, defn));
}

fn warn_inventory_only_entries(count: usize, args: &GetArgs) {
	if args.quiet || count == 0 {
		return;
	}
	if count == 1 {
		eprintln!("catalog entry known; detailed record not yet available");
	} else {
		eprintln!(
			"{} catalog entries known; detailed records not yet available",
			count
		);
	}
}

fn count_inventory_only_members(
	members: &[String],
	results: &[crate::query::QueryResult],
) -> usize {
	let populated: std::collections::HashSet<&str> = results
		.iter()
		.filter_map(|result| result.number.as_deref())
		.collect();
	members
		.iter()
		.filter(|member| !populated.contains(member.as_str()))
		.count()
}

fn output_inventory_group_overlay(
	query: &ComposerQuery,
	members: &[String],
	results: &[crate::query::QueryResult],
	args: &GetArgs,
	ctx: &OutputContext,
) -> usize {
	let populated: std::collections::HashMap<&str, &crate::query::QueryResult> = results
		.iter()
		.filter_map(|result| result.number.as_deref().map(|number| (number, result)))
		.collect();
	let missing = count_inventory_only_members(members, results);

	if args.json {
		let mut values = Vec::with_capacity(members.len());
		for member in members {
			if let Some(result) = populated.get(member.as_str()) {
				let path = id_to_path(ctx.data_dir, &result.id);
				if let Ok(comp) = load_composition(&path) {
					values.push(serde_json::to_value(&comp).unwrap_or(serde_json::Value::Null));
				}
			} else {
				values.push(inventory_stub_value(query, member));
			}
		}
		print(&serde_json::to_string_pretty(&values).unwrap());
		return missing;
	}

	if args.terse || args.movements {
		return missing;
	}

	for member in members {
		if let Some(result) = populated.get(member.as_str()) {
			output_pretty(std::slice::from_ref(*result), ctx);
		} else {
			output_inventory_stub(query, member, args, ctx.catalog_defn);
		}
	}
	missing
}


fn reject_structural_domain(
	query: &ComposerQuery,
	number: &str,
	args: &GetArgs,
	defn: Option<&CatalogDefinition>,
) -> bool {
	let Some(defn) = defn else {
		return false;
	};

	if defn.categories.is_some() && !number.contains(':') && find_category(number, defn).is_none() {
		if !args.quiet {
			eprintln!(
				"Invalid catalog category for {} / {}: \"{}\"",
				query.composer, defn.name, number
			);
		}
		return true;
	}

	if let Err(error) = validate_catalog_domain(number, defn) {
		if !args.quiet {
			eprintln!(
				"Catalog number out of range for {} / {}: \"{}\" ({})",
				query.composer, defn.name, number, error
			);
		}
		return true;
	}

	false
}

fn handle_inventory_miss(
	query: &ComposerQuery,
	number: &str,
	args: &GetArgs,
	index: &Index,
	defn: Option<&CatalogDefinition>,
) -> bool {
	let Some(scheme) = query.scheme.as_deref() else {
		return false;
	};

	if let Some(defn) = defn {
		if find_category(number, defn).is_none()
			&& crate::catalog::is_fallback_key(&crate::catalog::sort_key(number, defn))
		{
			if !args.quiet {
				eprintln!(
					"Invalid catalog number for {} / {}: \"{}\"",
					query.composer, defn.name, number
				);
			}
			return true;
		}
	}

	match index.inventory.lookup(
		&query.composer,
		scheme,
		query.edition.as_deref(),
		number,
		defn,
	) {
		InventoryLookup::Known => {
			if args.terse {
				if !args.quiet {
					eprintln!("Catalog entry is known, but no composition ID is available.");
				}
				return true;
			}
			if args.edit || args.movements || args.xref.is_some() {
				eprintln!("Catalog entry is known, but no detailed composition record is available.");
				return true;
			}
			output_inventory_stub(query, number, args, defn);
			if !args.quiet && !args.json {
				eprintln!("catalog entry known; detailed record not yet available");
			}
			true
		}
		InventoryLookup::KnownGroup(members) => {
			if args.terse {
				if !args.quiet {
					eprintln!("Catalog group is known, but no composition IDs are available.");
				}
				return true;
			}
			if args.edit || args.movements || args.xref.is_some() {
				eprintln!("Catalog group is known, but no detailed composition records are available.");
				return true;
			}
			if args.json {
				let values: Vec<_> = members
					.iter()
					.map(|member| inventory_stub_value(query, member))
					.collect();
				print(&serde_json::to_string_pretty(&values).unwrap());
			} else {
				for member in &members {
					output_inventory_stub(query, member, args, defn);
				}
				warn_inventory_only_entries(members.len(), args);
			}
			true
		}
		InventoryLookup::Absent => {
			if !args.quiet {
				eprintln!("No such catalog entry: {}", format_catalog(scheme, number, defn));
			}
			true
		}
		InventoryLookup::Unknown => false,
	}
}

fn run_query(query: ComposerQuery, args: &GetArgs, data_dir: &Path, config: &Config) {
	if (matches!(&query.number, Some(NumberSpec::Range { .. })) || query.group.is_some())
		&& query.scheme.is_none()
	{
		eprintln!("Error: range and group queries require a catalog scheme");
		eprintln!("Usage: wv get <composer> <scheme> <range>");
		std::process::exit(1);
	}

	let index = get_or_build_index(data_dir);
	let catalog_defn = query
		.scheme
		.as_ref()
		.and_then(|s| load_catalog_def(data_dir, s, Some(&query.composer)));
	let number_spec = query
		.number
		.as_ref()
		.map(|spec| normalize_number_spec(spec, catalog_defn.as_ref()));
	let category_query = number_spec.as_ref().and_then(|spec| {
		let NumberSpec::Single(number) = spec else {
			return None;
		};
		catalog_defn
			.as_ref()
			.and_then(|defn| find_category(number, defn))
			.map(str::to_lowercase)
	});

	match &number_spec {
		Some(NumberSpec::Single(number)) => {
			if reject_structural_domain(&query, number, args, catalog_defn.as_ref()) {
				return;
			}
		}
		Some(NumberSpec::Range { start, end }) => {
			if reject_structural_domain(&query, start, args, catalog_defn.as_ref())
				|| reject_structural_domain(&query, end, args, catalog_defn.as_ref())
			{
				return;
			}
		}
		None => {}
	}

	let mut builder = index.query().composer(&query.composer).data_dir(data_dir);
	if let Some(s) = &query.scheme {
		builder = builder.scheme(s);
	}

	match &number_spec {
		Some(NumberSpec::Single(n)) => {
			if category_query.is_some() {
				builder = builder.group(n);
			} else {
				builder = builder.number(n);
			}
		}
		Some(NumberSpec::Range { start, end }) => {
			builder = builder.range(start, end);
		}
		None => {}
	}

	if let Some(e) = &query.edition {
		builder = builder.edition(e);
	}

	if let Some(g) = &query.group {
		builder = builder.group(g);
	}
	let needs_sort = args.sorted
		|| query.group.is_some()
		|| category_query.is_some()
		|| matches!(&number_spec, Some(NumberSpec::Range { .. }));
	if needs_sort {
		builder = builder.sorted(data_dir);
	}

	builder = builder.strict(args.strict);

	let results = builder.fetch();

	if results.is_empty() {
		if let Some(NumberSpec::Single(number)) = number_spec.as_ref() {
			if handle_inventory_miss(
				&query,
				number,
				args,
				&index,
				catalog_defn.as_ref(),
			) {
				return;
			}
		}
		if !args.quiet {
			eprintln!("No results found.");
			print_query_examples(
				&query,
				number_spec.as_ref(),
				&index,
				data_dir,
				catalog_defn.as_ref(),
			);
		}
		return;
	}

	let inventory_group_members = match (query.scheme.as_deref(), number_spec.as_ref()) {
		(Some(scheme), Some(NumberSpec::Single(number))) => match index.inventory.lookup(
			&query.composer,
			scheme,
			query.edition.as_deref(),
			number,
			catalog_defn.as_ref(),
		) {
			InventoryLookup::KnownGroup(members) => Some(members),
			_ => None,
		},
		_ => None,
	};
	let inventory_only_count = inventory_group_members
		.as_ref()
		.map(|members| count_inventory_only_members(members, &results))
		.unwrap_or(0);

	if let Some(xref_type) = &args.xref {
		if xref_type == "mb" {
			run_xref_mb(&results, &query, data_dir, config, catalog_defn.as_ref());
			warn_inventory_only_entries(inventory_only_count, args);
			return;
		} else {
			eprintln!("Unknown xref type: {}", xref_type);
			std::process::exit(1);
		}
	}
	if !args.quiet {
		for result in &results {
			if result.superseded {
				if let (Some(num), Some(current), Some(scheme)) =
					(&result.number, &result.current_number, &query.scheme)
				{
					let formatted_current =
						format_catalog(scheme, current, catalog_defn.as_ref());
					let scheme_upper = scheme.to_uppercase();
					eprintln!(
						"warning: {} {} is superseded (current: {})",
						scheme_upper, num, formatted_current
					);
				}
			}
			if let Some(note) = &result.note {
				eprintln!("note: {}", note);
			}
		}
	}
	if args.edit {
		let paths: Vec<PathBuf> = results.iter().map(|r| id_to_path(data_dir, &r.id)).collect();
		warn_inventory_only_entries(inventory_only_count, args);
		open_in_editor(config, &paths, data_dir);
		return;
	}

	let ctx = OutputContext {
		data_dir,
		config,
		scheme: query.scheme.as_deref(),
		catalog_defn: catalog_defn.as_ref(),
	};

	if let Some(members) = inventory_group_members.as_ref() {
		if args.json {
			output_inventory_group_overlay(&query, members, &results, args, &ctx);
		} else if args.movements {
			output_movements(&results, &ctx);
			warn_inventory_only_entries(inventory_only_count, args);
		} else if args.terse {
			output_terse(&results);
			warn_inventory_only_entries(inventory_only_count, args);
		} else {
			let missing = output_inventory_group_overlay(&query, members, &results, args, &ctx);
			warn_inventory_only_entries(missing, args);
		}
	} else if args.json {
		output_json(&results, &ctx);
	} else if args.movements {
		output_movements(&results, &ctx);
	} else if args.terse {
		output_terse(&results);
	} else {
		output_pretty(&results, &ctx);
	}
}

fn run_xref_mb(
	results: &[crate::query::QueryResult],
	query: &ComposerQuery,
	_data_dir: &Path,
	config: &Config,
	catalog_defn: Option<&crate::types::CatalogDefinition>,
) {
	let db_path = match &config.xref.mb_database {
		Some(p) => p,
		None => {
			eprintln!("Error: mb_database not configured in config.toml");
			eprintln!("Add: [xref]");
			eprintln!("     mb_database = \"/path/to/mb.db\"");
			std::process::exit(1);
		}
	};
	let mb = match MbLookup::new(db_path) {
		Ok(m) => m,
		Err(e) => {
			eprintln!("Error opening MB database: {}", e);
			std::process::exit(1);
		}
	};

	let scheme = match &query.scheme {
		Some(s) => s,
		None => {
			eprintln!("Error: --xref requires a catalog scheme");
			std::process::exit(1);
		}
	};

	let numbers: Vec<String> = results
		.iter()
		.filter_map(|r| r.number.clone())
		.collect();

	let mb_results = mb.lookup_batch(&query.composer, scheme, &numbers, catalog_defn);
	let mut matched = 0;
	let mut not_found = 0;

	for r in &mb_results {
		if let Some(mb_id) = &r.mb_id {
			print(&format!("{}\t{}", r.catalog_number, mb_id));
			matched += 1;
		} else {
			print(&format!("{}\t", r.catalog_number));
			not_found += 1;
		}
	}

	let duplicates = check_duplicates(&mb_results);
	if !duplicates.is_empty() {
		eprintln!("\nwarning: duplicate MBIDs found:");
		for (mb_id, nums) in &duplicates {
			eprintln!("  {} -> {}", mb_id, nums.join(", "));
		}
	}
	eprintln!("\nmatched: {}, not found: {}", matched, not_found);
}

fn run_collections(collection_ids: &[String], args: &GetArgs, data_dir: &Path, config: &Config) {
	let refs = collection::expand(collection_ids, data_dir);

	if refs.is_empty() {
		if !args.quiet {
			eprintln!("No compositions found in specified collection(s).");
		}
		return;
	}

	let index = get_or_build_index(data_dir);
	if args.terse {
		for r in &refs {
			let results = index
				.query()
				.composer(&r.composer)
				.scheme(&r.scheme)
				.number(&r.number)
				.data_dir(data_dir)
				.fetch();

			for result in results {
				print(&result.id);
			}
		}
		return;
	}

	if args.json {
		let mut all_results = Vec::new();
		for r in &refs {
			let results = index
				.query()
				.composer(&r.composer)
				.scheme(&r.scheme)
				.number(&r.number)
				.data_dir(data_dir)
				.fetch();
			all_results.extend(results);
		}
		let ctx = OutputContext {
			data_dir,
			config,
			scheme: None,
			catalog_defn: None,
		};
		output_json(&all_results, &ctx);
		return;
	}

	if args.edit {
		let mut paths = Vec::new();
		for r in &refs {
			let results = index
				.query()
				.composer(&r.composer)
				.scheme(&r.scheme)
				.number(&r.number)
				.data_dir(data_dir)
				.fetch();

			for result in results {
				paths.push(id_to_path(data_dir, &result.id));
			}
		}
		open_in_editor(config, &paths, data_dir);
		return;
	}
	for r in &refs {
		let results = index
			.query()
			.composer(&r.composer)
			.scheme(&r.scheme)
			.number(&r.number)
			.data_dir(data_dir)
			.fetch();

		let catalog_defn = load_catalog_def(data_dir, &r.scheme, Some(&r.composer));

		for result in results {
			let comp_path = id_to_path(data_dir, &result.id);
			if args.movements {
				if let Ok(comp) = load_composition(&comp_path) {
					let formatted_cat = format_catalog(&r.scheme, &r.number, catalog_defn.as_ref());
					print(&format!("{}:", formatted_cat));
					if let Some(movements) = &comp.movements {
						for (i, movement) in movements.iter().enumerate() {
							let title = movement
								.title
								.as_deref()
								.or(movement.form.as_deref())
								.unwrap_or("?");
							print(&format!("  {}. {}", i + 1, title));
						}
					}
				}
			} else {
				if let Ok(comp) = load_composition(&comp_path) {
					let expansion_ctx = ExpansionContext {
						composition: &comp,
						collection: None,
						position_in_collection: None,
						config: &config.display,
					};
					let title = expand_title(&expansion_ctx);
					let formatted_cat = format_catalog(&r.scheme, &r.number, catalog_defn.as_ref());
					print(&format!("{}, {}", title, formatted_cat));
				} else {
					let formatted_cat = format_catalog(&r.scheme, &r.number, catalog_defn.as_ref());
					print(&formatted_cat);
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use std::collections::HashMap;

	use super::*;

	fn test_args(number: &[&str]) -> GetArgs {
		GetArgs {
			target: Some("bach".into()),
			scheme: Some("bwv".into()),
			number: number.iter().map(|value| value.to_string()).collect(),
			edition: None,
			group: None,
			sorted: false,
			terse: false,
			movements: false,
			json: false,
			quiet: false,
			edit: false,
			stdin: false,
			strict: false,
			xref: None,
			collection: None,
		}
	}

	fn hoboken_defn() -> CatalogDefinition {
		let mut categories = HashMap::new();
		categories.insert("III".into(), "string quartets".into());
		CatalogDefinition {
			name: "Hoboken-Verzeichnis".into(),
			categories: Some(categories),
			..Default::default()
		}
	}

	#[test]
	fn resolves_unquoted_multi_part_number() {
		let input = resolve_input(&test_args(&["anh.", "iii", "141"])).unwrap();
		match input {
			Input::Query(query) => {
				assert_eq!(query.number, Some(NumberSpec::Single("anh. iii 141".into())))
			}
			_ => panic!("expected query input"),
		}
	}

	#[test]
	fn normalizes_split_hoboken_number() {
		let defn = hoboken_defn();
		let spec = normalize_number_spec(&parse_number_spec("iii 32"), Some(&defn));
		assert_eq!(spec, NumberSpec::Single("iii:32".into()));
	}

	#[test]
	fn recognizes_hoboken_category() {
		let defn = hoboken_defn();
		let spec = normalize_number_spec(&parse_number_spec("iii"), Some(&defn));
		assert_eq!(category_for_spec(&spec, &defn).as_deref(), Some("iii"));
	}

	#[test]
	fn normalizes_split_hoboken_range() {
		let defn = hoboken_defn();
		let spec = normalize_number_spec(&parse_number_spec("iii 31-33"), Some(&defn));
		assert_eq!(
			spec,
			NumberSpec::Range {
				start: "iii:31".into(),
				end: "iii:33".into()
			}
		);
	}

	#[test]
	fn normalizes_abbreviated_hoboken_range() {
		let defn = hoboken_defn();
		let spec = normalize_number_spec(&parse_number_spec("iii:31-33"), Some(&defn));
		assert_eq!(
			spec,
			NumberSpec::Range {
				start: "iii:31".into(),
				end: "iii:33".into()
			}
		);
	}
}

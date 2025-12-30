use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::catalog::load_catalog_def;
use crate::config::{resolve_editor, Config};
use crate::index::get_or_build_index;
use crate::output::{
	id_to_path, output_by_ids, output_json, output_movements, output_pretty, output_terse,
	OutputContext,
};

pub struct GetArgs {
	pub target: Option<String>,
	pub scheme: Option<String>,
	pub number: Option<String>,
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

enum NumberSpec {
	Single(String),
	Range { start: String, end: String },
}

fn is_composition_id(s: &str) -> bool {
	s.len() == 8 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn parse_number_spec(s: &str) -> NumberSpec {
	fn try_split(s: &str) -> Option<(&str, &str)> {
		s.split_once('-')
			.or_else(|| s.split_once(".."))
			.or_else(|| s.split_once(' '))
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

fn resolve_input(args: &GetArgs) -> Option<Input> {
	if args.stdin {
		let ids: Vec<String> = io::stdin()
			.lock()
			.lines()
			.filter_map(|l| l.ok())
			.map(|l| l.trim().to_string())
			.filter(|l| !l.is_empty())
			.collect();
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
		if let Some(n) = &args.number {
			if is_composition_id(n) {
				ids.push(n.clone());
			}
		}
		return Some(Input::Ids(ids));
	}

	let number_spec = args.number.as_ref().map(|n| parse_number_spec(n));

	Some(Input::Query(ComposerQuery {
		composer: target.clone(),
		scheme: args.scheme.clone(),
		number: number_spec,
		edition: args.edition.clone(),
		group: args.group.clone(),
	}))
}

fn open_in_editor(config: &Config, paths: &[PathBuf]) {
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
}

pub fn run(args: GetArgs, data_dir: PathBuf, config: &Config) {
	let input = match resolve_input(&args) {
		Some(i) => i,
		None => {
			eprintln!("Usage: wv get <composer> [scheme] [number]");
			eprintln!("       wv get <id> [id...]");
			eprintln!("       wv get --stdin");
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
				open_in_editor(config, &paths);
			} else {
				output_by_ids(&ids, &data_dir, config, args.terse, args.movements, args.json);
			}
		}
		Input::Query(query) => {
			run_query(query, &args, &data_dir, config);
		}
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

	let mut builder = index.query().composer(&query.composer).data_dir(data_dir);

	if let Some(s) = &query.scheme {
		builder = builder.scheme(s);
	}

	match &query.number {
		Some(NumberSpec::Single(n)) => {
			builder = builder.number(n);
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

	let needs_sort =
		args.sorted || query.group.is_some() || matches!(&query.number, Some(NumberSpec::Range { .. }));
	if needs_sort {
		builder = builder.sorted(data_dir);
	}

	builder = builder.strict(args.strict);

	let results = builder.fetch();

	if results.is_empty() {
		if !args.quiet {
			eprintln!("No results found.");
		}
		return;
	}

	if !args.quiet {
		for result in &results {
			if result.superseded {
				if let (Some(num), Some(current)) = (&result.number, &result.current_number) {
					let scheme_upper = query.scheme.as_ref().map(|s| s.to_uppercase()).unwrap_or_default();
					eprintln!("warning: {} {} is superseded (current: {})", scheme_upper, num, current);
				}
			}
		}
	}

	if args.edit {
		let paths: Vec<PathBuf> = results.iter().map(|r| id_to_path(data_dir, &r.id)).collect();
		open_in_editor(config, &paths);
		return;
	}

	let catalog_defn = query
		.scheme
		.as_ref()
		.and_then(|s| load_catalog_def(data_dir, s, Some(&query.composer)));

	let ctx = OutputContext {
		data_dir,
		config,
		scheme: query.scheme.as_deref(),
		catalog_defn: catalog_defn.as_ref(),
	};

	if args.json {
		output_json(&results, &ctx);
	} else if args.movements {
		output_movements(&results, &ctx);
	} else if args.terse {
		output_terse(&results, query.scheme.as_deref());
	} else {
		output_pretty(&results, &ctx);
	}
}

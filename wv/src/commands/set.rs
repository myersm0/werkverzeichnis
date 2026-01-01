use std::fs;
use std::path::{Path, PathBuf};

use crate::catalog::load_catalog_def;
use crate::config::Config;
use crate::index::get_or_build_index;
use crate::output::id_to_path;
use crate::xref::{check_duplicates, MbLookup};

pub struct SetArgs {
	pub target: String,
	pub scheme: Option<String>,
	pub number: Option<String>,
	pub xref: Option<String>,
}

pub fn run(args: SetArgs, data_dir: PathBuf, config: &Config) {
	let Some(xref_type) = &args.xref else {
		eprintln!("Error: --xref is required for set command");
		eprintln!("Usage: wv set <composer> <scheme> [number] --xref=mb");
		std::process::exit(1);
	};

	if xref_type != "mb" {
		eprintln!("Unknown xref type: {}", xref_type);
		std::process::exit(1);
	}

	let Some(scheme) = &args.scheme else {
		eprintln!("Error: set --xref requires a catalog scheme");
		std::process::exit(1);
	};

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

	let index = get_or_build_index(&data_dir);
	let composer = &args.target;

	let catalog_defn = load_catalog_def(&data_dir, scheme, Some(composer));

	let mut builder = index.query().composer(composer).scheme(scheme).data_dir(&data_dir);

	if let Some(n) = &args.number {
		if let Some((start, end)) = n.split_once('-').or_else(|| n.split_once("..")) {
			builder = builder.range(start, end).sorted(&data_dir);
		} else {
			builder = builder.number(n);
		}
	}

	let results = builder.fetch();

	if results.is_empty() {
		eprintln!("No results found.");
		return;
	}

	let numbers: Vec<String> = results.iter().filter_map(|r| r.number.clone()).collect();
	let mb_results = mb.lookup_batch(composer, scheme, &numbers, catalog_defn.as_ref());

	let duplicates = check_duplicates(&mb_results);
	if !duplicates.is_empty() {
		eprintln!("warning: duplicate MBIDs found:");
		for (mb_id, nums) in &duplicates {
			eprintln!("  {} -> {}", mb_id, nums.join(", "));
		}
		eprintln!();
	}

	let mut updated = 0;
	let mut unchanged = 0;
	let mut not_found = 0;

	for (result, mb_result) in results.iter().zip(mb_results.iter()) {
		let status = if let Some(mb_id) = &mb_result.mb_id {
			match update_composition_xref(&data_dir, &result.id, mb_id) {
				UpdateResult::Updated => {
					updated += 1;
					"[updated]"
				}
				UpdateResult::Unchanged => {
					unchanged += 1;
					"[unchanged]"
				}
				UpdateResult::Error(e) => {
					eprintln!("error updating {}: {}", result.id, e);
					"[error]"
				}
			}
		} else {
			not_found += 1;
			"[not found]"
		};

		let num = mb_result.catalog_number.as_str();
		let mb_id = mb_result.mb_id.as_deref().unwrap_or("");
		println!("{}\t{}\t{}", num, mb_id, status);
	}

	eprintln!("\nupdated: {}, unchanged: {}, not found: {}", updated, unchanged, not_found);
}

enum UpdateResult {
	Updated,
	Unchanged,
	Error(String),
}

fn update_composition_xref(data_dir: &Path, id: &str, mb_id: &str) -> UpdateResult {
	let path = id_to_path(data_dir, id);

	let content = match fs::read_to_string(&path) {
		Ok(c) => c,
		Err(e) => return UpdateResult::Error(e.to_string()),
	};

	let mut data: serde_json::Value = match serde_json::from_str(&content) {
		Ok(d) => d,
		Err(e) => return UpdateResult::Error(e.to_string()),
	};

	let xref = data
		.as_object_mut()
		.unwrap()
		.entry("xref")
		.or_insert_with(|| serde_json::json!({}));

	let xref_obj = xref.as_object_mut().unwrap();

	if xref_obj.get("mb").and_then(|v| v.as_str()) == Some(mb_id) {
		return UpdateResult::Unchanged;
	}

	xref_obj.insert("mb".to_string(), serde_json::json!(mb_id));

	let output = match serde_json::to_string_pretty(&data) {
		Ok(s) => s,
		Err(e) => return UpdateResult::Error(e.to_string()),
	};

	match fs::write(&path, output + "\n") {
		Ok(_) => UpdateResult::Updated,
		Err(e) => UpdateResult::Error(e.to_string()),
	}
}

// src/commands/index.rs
use std::path::Path;

use crate::index::{build_index, save_index};
use crate::output::print;

pub fn run(data_dir: &Path) {
	print(&format!("Building index from {:?}...", data_dir));

	let index = build_index(data_dir);

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

	print(&format!("Found {} compositions", total_compositions));
	print(&format!("Found {} catalog entries", total_catalog_entries));

	if let Err(e) = save_index(&index, data_dir) {
		eprintln!("Error writing index: {}", e);
		std::process::exit(1);
	}

	let indexes_dir = data_dir.join(".indexes");
	print(&format!("Wrote {}", indexes_dir.join("index.json").display()));
	print(&format!(
		"Wrote {}",
		indexes_dir.join("composer-index.json").display()
	));
	if !index.editions.is_empty() {
		print(&format!(
		"Wrote edition indexes to {}",
		indexes_dir.join("editions").display()
		));
	}
	print(&format!(
		"Wrote {}",
		indexes_dir.join("metadata.json").display()
	));

	print("Done.");
}

// src/commands/index.rs
use std::path::Path;

use crate::index::{build_index, write_composer_index, write_edition_indexes, write_index};
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
	print(&format!("Wrote {}", index_path.display()));

	if let Err(e) = write_composer_index(&index, &composer_path) {
		eprintln!("Error writing composer index: {}", e);
		std::process::exit(1);
	}
	print(&format!("Wrote {}", composer_path.display()));

	if !index.editions.is_empty() {
		if let Err(e) = write_edition_indexes(&index, data_dir) {
			eprintln!("Error writing edition indexes: {}", e);
			std::process::exit(1);
		}
		print(&format!("Wrote edition indexes to {}", editions_dir.display()));
	}

	print("Done.");
}

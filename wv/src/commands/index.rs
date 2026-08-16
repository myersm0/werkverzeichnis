// src/commands/index.rs
use std::path::Path;

use crate::index::{build_index, save_index};

pub fn run(data_dir: &Path) {
	eprintln!("Building index from {:?}...", data_dir);

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

	eprintln!("Found {} compositions", total_compositions);
	eprintln!("Found {} catalog entries", total_catalog_entries);

	let total_inventory_entries: usize = index
		.inventory
		.catalogs
		.values()
		.flat_map(|schemes| schemes.values())
		.map(|scheme| {
			scheme.default.as_ref().map_or(0, |catalog| catalog.entries.len())
				+ scheme.editions.values().map(|catalog| catalog.entries.len()).sum::<usize>()
		})
		.sum();
	eprintln!("Found {} inventory entries", total_inventory_entries);

	if let Err(e) = save_index(&index, data_dir) {
		eprintln!("Error writing index: {}", e);
		std::process::exit(1);
	}

	let indexes_dir = data_dir.join(".indexes");
	eprintln!("Wrote {}", indexes_dir.join("index.json").display());
	eprintln!(
		"Wrote {}",
		indexes_dir.join("composer-index.json").display()
	);
	eprintln!(
		"Wrote {}",
		indexes_dir.join("inventory-index.json").display()
	);
	if !index.editions.is_empty() {
		eprintln!(
			"Wrote edition indexes to {}",
			indexes_dir.join("editions").display()
		);
	}
	eprintln!(
		"Wrote {}",
		indexes_dir.join("metadata.json").display()
	);

	eprintln!("Done.");
}

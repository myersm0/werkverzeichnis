use std::path::Path;

use crate::merge::merge_attribution_with_collections;
use crate::parse::load_composition;

pub fn run(path: &Path, data_dir: &Path) {
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
			let edition_str = cat
				.edition
				.as_ref()
				.map(|e| format!(" (ed. {})", e))
				.unwrap_or_default();
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

use std::path::Path;

use crate::merge::merge_attribution;
use crate::parse::load_composition;

pub fn run(path: &Path, _data_dir: &Path) {
	let comp = match load_composition(path) {
		Ok(c) => c,
		Err(e) => {
			eprintln!("Error loading composition: {}", e);
			std::process::exit(1);
		}
	};

	let merged = merge_attribution(&comp.attribution);

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

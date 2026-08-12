use std::path::Path;

use crate::merge::merge_attribution;
use crate::output::print;
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

	print(&format!("ID: {}", comp.id));
	print(&format!("Form: {}", comp.form));
	if let Some(key) = &comp.key {
		print(&format!("Key: {}", key));
	}
	print("");
	print("Merged attribution:");
	if let Some(composer) = &merged.composer {
		print(&format!("  Composer: {}", composer));
	}
	if let Some(composed) = merged.dates.composed {
		print(&format!("  Composed: {}", composed));
	}
	if let Some(published) = merged.dates.published {
		print(&format!("  Published: {}", published));
	}
	if let Some(status) = &merged.status {
		print(&format!("  Status: {:?}", status));
	}
	if !merged.catalog.is_empty() {
		print("  Catalog entries:");
		for cat in &merged.catalog {
			let edition_str = cat
				.edition
				.as_ref()
				.map(|e| format!(" (ed. {})", e))
				.unwrap_or_default();
			print(&format!("    {}:{}{}", cat.scheme, cat.number, edition_str));
		}
	}
	if !merged.notes.is_empty() {
		print("  Notes:");
		for note in &merged.notes {
			print(&format!("    - {}", note));
		}
	}
}

use std::path::Path;

use crate::add::{generate_unique_id, scaffold_composition};
use crate::parse::path_for_id;
use crate::index::mark_index_dirty;
use crate::output::print;

pub fn run(form: &str, composer: &str, data_dir: &Path) {
	let id = generate_unique_id(data_dir);
	let json = scaffold_composition(&id, form, composer);

	let dest_path = match path_for_id(data_dir.join("compositions"), &id) {
		Ok(path) => path,
		Err(e) => {
			eprintln!("Error building destination path: {}", e);
			std::process::exit(1);
		}
	};

	if dest_path.exists() {
		eprintln!("Error: {} already exists; refusing to overwrite", dest_path.display());
		std::process::exit(1);
	}

	if let Err(e) = std::fs::create_dir_all(dest_path.parent().unwrap_or(data_dir)) {
		eprintln!("Error creating directory: {}", e);
		std::process::exit(1);
	}

	if let Err(e) = std::fs::write(&dest_path, &json) {
		eprintln!("Error writing file: {}", e);
		std::process::exit(1);
	}

	if let Err(e) = mark_index_dirty(data_dir) {
		eprintln!("warning: failed to mark index stale: {}", e);
	}

	print(&format!("Created {}", dest_path.display()));
	print(&format!("ID: {}", id));
}

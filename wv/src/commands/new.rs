use std::path::Path;

use crate::add::{generate_id, scaffold_composition};

pub fn run(form: &str, composer: &str, data_dir: &Path) {
	let id = generate_id();
	let json = scaffold_composition(&id, form, composer);

	let prefix = &id[..2];
	let suffix = &id[2..];
	let dest_dir = data_dir.join("compositions").join(prefix);
	let dest_path = dest_dir.join(format!("{}.json", suffix));

	if let Err(e) = std::fs::create_dir_all(&dest_dir) {
		eprintln!("Error creating directory: {}", e);
		std::process::exit(1);
	}

	if let Err(e) = std::fs::write(&dest_path, &json) {
		eprintln!("Error writing file: {}", e);
		std::process::exit(1);
	}

	println!("Created {}", dest_path.display());
	println!("ID: {}", id);
}

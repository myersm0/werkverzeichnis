use std::path::Path;

use crate::add::add_composition;

pub fn run(path: &Path, force: bool, data_dir: &Path) {
	match add_composition(path, data_dir, force) {
		Ok(result) => {
			println!(
				"Added {} -> {}",
				result.source.display(),
				result.destination.display()
			);
			println!("ID: {}", result.id);
		}
		Err(e) => {
			eprintln!("Error: {}", e);
			std::process::exit(1);
		}
	}
}

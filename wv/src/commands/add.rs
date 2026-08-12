use std::path::Path;

use crate::add::add_composition;
use crate::output::print;

pub fn run(path: &Path, force: bool, data_dir: &Path) {
	match add_composition(path, data_dir, force) {
		Ok(result) => {
			print(&format!(
				"Added {} -> {}",
				result.source.display(),
				result.destination.display()
			));
			print(&format!("ID: {}", result.id));
		}
		Err(e) => {
			eprintln!("Error: {}", e);
			std::process::exit(1);
		}
	}
}

use std::path::Path;

use crate::add::add_composition;
use crate::index::mark_index_dirty;
use crate::output::print;

pub fn run(path: &Path, force: bool, data_dir: &Path) {
	match add_composition(path, data_dir, force) {
		Ok(result) => {
			if let Err(e) = mark_index_dirty(data_dir) {
				eprintln!("warning: failed to mark index stale: {}", e);
			}

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

use std::path::Path;

use crate::output::print;
use crate::validate::{validate_all, validate_file};

pub fn run(path: Option<&Path>, data_dir: &Path) {
	let errors = if let Some(p) = path {
		validate_file(p, data_dir)
	} else {
		print(&format!("Validating dataset in {:?}...", data_dir));
		validate_all(data_dir)
	};

	if errors.is_empty() {
		print("No validation errors found.");
	} else {
		eprintln!("Found {} validation error(s):", errors.len());
		for err in &errors {
			eprintln!("  {}", err);
		}
		std::process::exit(1);
	}
}

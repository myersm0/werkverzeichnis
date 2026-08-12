use std::path::Path;

use crate::output::print;
use crate::parse::{load_collection, load_composer, load_composition};

pub fn run_composition(path: &Path) {
	match load_composition(path) {
		Ok(comp) => print(&format!("{:#?}", comp)),
		Err(e) => {
			eprintln!("Error: {}", e);
			std::process::exit(1);
		}
	}
}

pub fn run_composer(path: &Path) {
	match load_composer(path) {
		Ok(comp) => print(&format!("{:#?}", comp)),
		Err(e) => {
			eprintln!("Error: {}", e);
			std::process::exit(1);
		}
	}
}

pub fn run_collection(path: &Path) {
	match load_collection(path) {
		Ok(coll) => print(&format!("{:#?}", coll)),
		Err(e) => {
			eprintln!("Error: {}", e);
			std::process::exit(1);
		}
	}
}

use std::io::{self, BufRead};
use std::path::Path;

use crate::catalog::{load_catalog_def, sort_key, sort_numbers};
use crate::output::print;

pub fn run_sort(scheme: &str, composer: Option<&str>, data_dir: &Path) {
	let defn = match load_catalog_def(data_dir, scheme, composer) {
		Ok(definition) => definition,
		Err(error) => {
			eprintln!("Error loading catalog metadata: {}", error);
			std::process::exit(1);
		}
	};

	let mut numbers = Vec::new();
	for line in io::stdin().lock().lines() {
		let line = match line {
			Ok(line) => line,
			Err(error) => {
				eprintln!("Error reading stdin: {}", error);
				std::process::exit(1);
			}
		};
		let number = line.trim();
		if !number.is_empty() {
			numbers.push(number.to_string());
		}
	}

	sort_numbers(&mut numbers, defn.as_ref());

	for n in numbers {
		print(&n);
	}
}

pub fn run_sort_key(scheme: &str, number: &str, composer: Option<&str>, data_dir: &Path) {
	let defn = match load_catalog_def(data_dir, scheme, composer) {
		Ok(Some(d)) => d,
		Ok(None) => {
			eprintln!("Unknown catalog: {}", scheme);
			std::process::exit(1);
		}
		Err(error) => {
			eprintln!("Error loading catalog metadata: {}", error);
			std::process::exit(1);
		}
	};

	let key = sort_key(number, &defn);
	print(&format!("{:?}", key));
}

use std::io::{self, BufRead};
use std::path::Path;

use crate::catalog::{load_catalog_def, sort_key, sort_numbers};

pub fn run_sort(scheme: &str, composer: Option<&str>, data_dir: &Path) {
	let defn = load_catalog_def(data_dir, scheme, composer);

	let mut numbers: Vec<String> = io::stdin()
		.lock()
		.lines()
		.map_while(Result::ok)
		.filter(|s| !s.trim().is_empty())
		.map(|s| s.trim().to_string())
		.collect();

	sort_numbers(&mut numbers, defn.as_ref());

	for n in numbers {
		println!("{}", n);
	}
}

pub fn run_sort_key(scheme: &str, number: &str, composer: Option<&str>, data_dir: &Path) {
	let defn = match load_catalog_def(data_dir, scheme, composer) {
		Some(d) => d,
		None => {
			eprintln!("Unknown catalog: {}", scheme);
			std::process::exit(1);
		}
	};

	let key = sort_key(number, &defn);
	println!("{:?}", key);
}

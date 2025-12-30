use std::io::{self, BufRead};
use std::path::Path;

use crate::catalog::load_catalog_def;
use crate::config::Config;
use crate::display::{expand_title, format_catalog, ExpansionContext};
use crate::types::Composition;

pub fn run(data_dir: &Path, config: &Config) {
	let input: String = io::stdin()
		.lock()
		.lines()
		.filter_map(|l| l.ok())
		.collect::<Vec<_>>()
		.join("\n");

	if input.trim().is_empty() {
		return;
	}

	let compositions: Vec<Composition> =
		if let Ok(arr) = serde_json::from_str::<Vec<Composition>>(&input) {
			arr
		} else if let Ok(comp) = serde_json::from_str::<Composition>(&input) {
			vec![comp]
		} else {
			eprintln!("Error: Invalid JSON input");
			std::process::exit(1);
		};

	for comp in &compositions {
		let ctx = ExpansionContext {
			composition: comp,
			collection: None,
			position_in_collection: None,
			config: &config.display,
		};
		let title = expand_title(&ctx);

		if let Some(attr) = comp.attribution.first() {
			if let Some(cat) = attr.catalog.as_ref().and_then(|c| c.first()) {
				let catalog_defn = load_catalog_def(data_dir, &cat.scheme, attr.composer.as_deref());
				let formatted = format_catalog(&cat.scheme, &cat.number, catalog_defn.as_ref());
				println!("{}, {}", title, formatted);
				continue;
			}
		}
		println!("{} [{}]", title, comp.id);
	}
}

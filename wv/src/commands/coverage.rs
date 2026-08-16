use std::collections::HashSet;
use std::path::Path;

use crate::catalog::{load_catalog_def, sort_numbers};
use crate::display::format_catalog;
use crate::index::{get_or_build_index, load_edition_index};
use crate::output::print;

pub fn run(composer: &str, scheme: Option<&str>, edition: Option<&str>, missing: bool, data_dir: &Path) {
	let index = get_or_build_index(data_dir);
	let schemes: Vec<String> = match scheme {
		Some(scheme) => vec![scheme.to_string()],
		None => {
			let Some(schemes) = index.inventory.catalogs.get(composer) else {
				eprintln!("No catalog inventories found for {}.", composer);
				return;
			};
			let mut names: Vec<String> = schemes.keys().cloned().collect();
			names.sort();
			names
		}
	};

	let mut found = false;
	for scheme in schemes {
		let defn = load_catalog_def(data_dir, &scheme, Some(composer));
		let Some(scheme_index) = index.inventory.catalogs.get(composer).and_then(|s| s.get(&scheme)) else {
			eprintln!("No catalog inventory found for {} / {}.", composer, scheme);
			continue;
		};

		let resolved_edition = if edition.is_some() {
			edition
		} else if scheme_index.default.is_none() {
			defn.as_ref().and_then(|d| d.current_edition.as_deref())
		} else {
			None
		};
		let Some(catalog) = index.inventory.catalog(composer, &scheme, resolved_edition, defn.as_ref()) else {
			eprintln!("No applicable catalog inventory found for {} / {}.", composer, scheme);
			continue;
		};
		found = true;

		let populated: HashSet<String> = if let Some(edition) = resolved_edition {
			load_edition_index(data_dir, composer, &scheme, edition)
				.unwrap_or_default()
				.into_keys()
				.collect()
		} else {
			index
				.catalog
				.get(composer)
				.and_then(|schemes| schemes.get(&scheme))
				.map(|scheme| scheme.current.keys().cloned().collect())
				.unwrap_or_default()
		};

		let total = catalog.entries.len();
		let populated_count = catalog.entries.iter().filter(|number| populated.contains(*number)).count();
		let missing_count = total.saturating_sub(populated_count);
		let percent = if total == 0 {
			0.0
		} else {
			100.0 * populated_count as f64 / total as f64
		};

		let edition_text = resolved_edition.map_or(String::new(), |edition| format!(" edition {}", edition));
		print(&format!("{} / {}{}", composer, scheme, edition_text));
		print(&format!("Inventory: {}", if catalog.complete { "complete" } else { "incomplete" }));
		print(&format!("Inventory entries: {}", total));
		print(&format!("Populated: {}", populated_count));
		print(&format!("Missing: {}", missing_count));
		print(&format!("Coverage: {:.1}%", percent));

		if missing {
			let mut numbers: Vec<String> = catalog
				.entries
				.iter()
				.filter(|number| !populated.contains(*number))
				.cloned()
				.collect();
			sort_numbers(&mut numbers, defn.as_ref());
			for number in numbers {
				print(&format_catalog(&scheme, &number, defn.as_ref()));
			}
		}
	}

	if !found && scheme.is_none() {
		eprintln!("No applicable catalog inventories found for {}.", composer);
	}
}

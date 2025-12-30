use std::path::Path;

use crate::config::Config;
use crate::display::{expand_title, format_catalog, ExpansionContext};
use crate::index::get_or_build_index;
use crate::merge::collection_path_from_id;
use crate::parse::{load_collection, load_composition};
use crate::catalog::load_catalog_def;

pub fn run_collection(
	id: &str,
	verify: bool,
	hydrate: bool,
	terse: bool,
	data_dir: &Path,
	config: &Config,
) {
	let collections_dir = data_dir.join("collections");
	let path = collection_path_from_id(&collections_dir, id);

	let collection = match load_collection(&path) {
		Ok(c) => c,
		Err(e) => {
			eprintln!("Error loading collection: {}", e);
			std::process::exit(1);
		}
	};

	let index = if verify || hydrate || !terse {
		Some(get_or_build_index(data_dir))
	} else {
		None
	};

	let composer = collection
		.composer
		.as_deref()
		.unwrap_or_else(|| id.split_once('-').map(|(c, _)| c).unwrap_or(id));

	let catalog_defn = if !terse {
		load_catalog_def(data_dir, &collection.scheme, Some(composer))
	} else {
		None
	};

	let mut missing = Vec::new();

	for (i, num) in collection.compositions.iter().enumerate() {
		let position = i + 1;

		if verify || hydrate {
			let idx = index.as_ref().unwrap();
			let found = idx
				.query()
				.composer(composer)
				.scheme(&collection.scheme)
				.number(num)
				.fetch_one();

			if let Some(comp_id) = found {
				let comp_path = data_dir
					.join("compositions")
					.join(&comp_id[..2])
					.join(format!("{}.json", &comp_id[2..]));

				if hydrate {
					if let Ok(comp) = load_composition(&comp_path) {
						println!("{}:{} [{}]", collection.scheme, num, comp_id);
						println!("  Form: {}", comp.form);
						if let Some(key) = &comp.key {
							println!("  Key: {}", key);
						}
					} else {
						println!("{}:{} [{}] (file not found)", collection.scheme, num, comp_id);
					}
				} else {
					println!("{}:{} ✓", collection.scheme, num);
				}
			} else {
				missing.push(num.clone());
				println!("{}:{} ✗ NOT FOUND", collection.scheme, num);
			}
		} else if terse {
			println!("{}:{}", collection.scheme, num);
		} else {
			let idx = index.as_ref().unwrap();
			let found = idx
				.query()
				.composer(composer)
				.scheme(&collection.scheme)
				.number(num)
				.fetch_one();

			if let Some(comp_id) = found {
				let comp_path = data_dir
					.join("compositions")
					.join(&comp_id[..2])
					.join(format!("{}.json", &comp_id[2..]));

				let formatted_cat = format_catalog(&collection.scheme, num, catalog_defn.as_ref());
				if let Ok(comp) = load_composition(&comp_path) {
					let ctx = ExpansionContext {
						composition: &comp,
						collection: Some(&collection),
						position_in_collection: Some(position),
						config: &config.display,
					};
					let title = expand_title(&ctx);
					println!("{}, {}", title, formatted_cat);
				} else {
					println!("{}", formatted_cat);
				}
			} else {
				let formatted_cat = format_catalog(&collection.scheme, num, catalog_defn.as_ref());
				println!("{} (not found)", formatted_cat);
			}
		}
	}

	if verify && !missing.is_empty() {
		eprintln!();
		eprintln!("Missing {} composition(s)", missing.len());
		std::process::exit(1);
	}
}

pub fn run_collections(query: &str, data_dir: &Path) {
	let collections_dir = data_dir.join("collections");

	let (scheme, number) = if let Some((s, n)) = query.split_once(':') {
		(Some(s), Some(n))
	} else {
		(None, None)
	};

	let mut found = Vec::new();

	if let Ok(composer_dirs) = std::fs::read_dir(&collections_dir) {
		for composer_entry in composer_dirs.flatten() {
			if !composer_entry.path().is_dir() {
				continue;
			}

			if let Ok(coll_files) = std::fs::read_dir(composer_entry.path()) {
				for file_entry in coll_files.flatten() {
					let path = file_entry.path();
					if path.extension().map_or(true, |e| e != "json") {
						continue;
					}

					if let Ok(coll) = load_collection(&path) {
						let matches = if let (Some(s), Some(n)) = (scheme, number) {
							coll.scheme == s && coll.compositions.contains(&n.to_string())
						} else {
							false
						};

						if matches {
							found.push(coll.id.clone());
						}
					}
				}
			}
		}
	}

	if found.is_empty() {
		println!("No collections found containing '{}'", query);
	} else {
		println!("Collections containing '{}':", query);
		for id in found {
			println!("  {}", id);
		}
	}
}

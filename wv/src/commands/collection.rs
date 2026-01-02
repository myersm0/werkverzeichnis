use std::path::Path;

use crate::catalog::load_catalog_def;
use crate::config::Config;
use crate::display::{expand_title, format_catalog, ExpansionContext};
use crate::index::get_or_build_index;
use crate::merge::collection_path_from_id;
use crate::parse::{load_collection, load_composition};

pub fn list(composer: Option<&str>, user: bool, data_dir: &Path) {
	let base_dir = if user {
		data_dir.join("user-collections")
	} else {
		data_dir.join("collections")
	};

	if !base_dir.exists() {
		return;
	}

	let dirs_to_scan: Vec<_> = if let Some(c) = composer {
		vec![base_dir.join(c)]
	} else {
		match std::fs::read_dir(&base_dir) {
			Ok(entries) => entries
				.flatten()
				.filter(|e| e.path().is_dir())
				.map(|e| e.path())
				.collect(),
			Err(_) => return,
		}
	};

	for dir in dirs_to_scan {
		if !dir.is_dir() {
			continue;
		}

		let files = match std::fs::read_dir(&dir) {
			Ok(f) => f,
			Err(_) => continue,
		};

		for entry in files.flatten() {
			let path = entry.path();
			if path.extension().map_or(true, |e| e != "json") {
				continue;
			}

			if let Ok(coll) = load_collection(&path) {
				let title = coll
					.title
					.get("en")
					.or_else(|| coll.title.get("de"))
					.map(|s| s.as_str())
					.unwrap_or("");
				let count = coll.compositions.len();
				println!("{}\t{}\t({})", coll.id, title, count);
			}
		}
	}
}

pub fn show(id: &str, data_dir: &Path, config: &Config) {
	let collections_dir = data_dir.join("collections");
	let user_collections_dir = data_dir.join("user-collections");

	let path = {
		let official = collection_path_from_id(&collections_dir, id);
		if official.exists() {
			official
		} else {
			let user_path = user_collections_dir.join(format!("{}.json", id));
			if user_path.exists() {
				user_path
			} else {
				eprintln!("Collection not found: {}", id);
				std::process::exit(1);
			}
		}
	};

	let collection = match load_collection(&path) {
		Ok(c) => c,
		Err(e) => {
			eprintln!("Error loading collection: {}", e);
			std::process::exit(1);
		}
	};

	let index = get_or_build_index(data_dir);

	let composer = collection
		.attribution
		.first()
		.and_then(|a| a.composer.as_deref())
		.or(collection.composer.as_deref())
		.unwrap_or_else(|| id.split_once('-').map(|(c, _)| c).unwrap_or(id));

	let catalog_defn = load_catalog_def(data_dir, &collection.scheme, Some(composer));

	if let Some(en) = collection.title.get("en") {
		println!("{}", en);
	} else if let Some(de) = collection.title.get("de") {
		println!("{}", de);
	}

	println!();

	for num in &collection.compositions {
		let found = index
			.query()
			.composer(composer)
			.scheme(&collection.scheme)
			.number(num)
			.fetch_one();

		let formatted_cat = format_catalog(&collection.scheme, num, catalog_defn.as_ref());

		if let Some(comp_id) = found {
			let comp_path = data_dir
				.join("compositions")
				.join(&comp_id[..2])
				.join(format!("{}.json", &comp_id[2..]));

			if let Ok(comp) = load_composition(&comp_path) {
				let ctx = ExpansionContext {
					composition: &comp,
					collection: None,
					position_in_collection: None,
					config: &config.display,
				};
				let title = expand_title(&ctx);
				println!("{}, {}", title, formatted_cat);
			} else {
				println!("{}", formatted_cat);
			}
		} else {
			println!("{} (not indexed)", formatted_cat);
		}
	}
}

pub fn find(query: &str, data_dir: &Path) {
	let collections_dir = data_dir.join("collections");

	let (scheme, number) = if let Some((s, n)) = query.split_once(':') {
		(s, n)
	} else {
		eprintln!("Usage: wv collection find <scheme>:<number>");
		eprintln!("Example: wv collection find bwv:846");
		std::process::exit(1);
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
						if coll.scheme == scheme && coll.compositions.contains(&number.to_string())
						{
							found.push(coll.id.clone());
						}
					}
				}
			}
		}
	}

	if found.is_empty() {
		println!("No collections contain {}:{}", scheme, number);
	} else {
		for id in found {
			println!("{}", id);
		}
	}
}

pub struct ExpandedRef {
	pub composer: String,
	pub scheme: String,
	pub number: String,
}

pub fn expand(ids: &[String], data_dir: &Path) -> Vec<ExpandedRef> {
	let collections_dir = data_dir.join("collections");
	let user_collections_dir = data_dir.join("user-collections");

	let mut result = Vec::new();

	for id in ids {
		let path = {
			let official = collection_path_from_id(&collections_dir, id);
			if official.exists() {
				official
			} else {
				let user_path = user_collections_dir.join(format!("{}.json", id));
				if user_path.exists() {
					user_path
				} else {
					eprintln!("Collection not found: {}", id);
					continue;
				}
			}
		};

		let collection = match load_collection(&path) {
			Ok(c) => c,
			Err(e) => {
				eprintln!("Error loading collection {}: {}", id, e);
				continue;
			}
		};

		let composer = collection
			.attribution
			.first()
			.and_then(|a| a.composer.clone())
			.or_else(|| collection.composer.clone())
			.unwrap_or_else(|| id.split_once('-').map(|(c, _)| c.to_string()).unwrap_or_default());

		for num in &collection.compositions {
			result.push(ExpandedRef {
				composer: composer.clone(),
				scheme: collection.scheme.clone(),
				number: num.clone(),
			});
		}
	}

	result
}

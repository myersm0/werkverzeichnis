use std::path::Path;

use crate::catalog::load_catalog_def;
use crate::config::Config;
use crate::display::{expand_title, format_catalog, ExpansionContext};
use crate::index::get_or_build_index;
use crate::merge::collection_path_from_id;
use crate::output::print;
use crate::parse::{load_collection, load_composition};

fn read_dir_or_exit(path: &Path) -> Vec<std::fs::DirEntry> {
	let entries = match std::fs::read_dir(path) {
		Ok(entries) => entries,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
		Err(error) => {
			eprintln!("Error reading directory {}: {}", path.display(), error);
			std::process::exit(1);
		}
	};
	entries
		.map(|entry| match entry {
			Ok(entry) => entry,
			Err(error) => {
				eprintln!("Error reading directory entry in {}: {}", path.display(), error);
				std::process::exit(1);
			}
		})
		.collect()
}

fn entry_is_dir_or_exit(entry: &std::fs::DirEntry) -> bool {
	match entry.file_type() {
		Ok(file_type) => file_type.is_dir(),
		Err(error) => {
			eprintln!("Error reading metadata for {}: {}", entry.path().display(), error);
			std::process::exit(1);
		}
	}
}

pub fn list(composer: Option<&str>, user: bool, data_dir: &Path) {
	let base_dir = if user {
		data_dir.join("user-collections")
	} else {
		data_dir.join("collections")
	};

	let dirs_to_scan: Vec<_> = if let Some(c) = composer {
		vec![base_dir.join(c)]
	} else {
		read_dir_or_exit(&base_dir)
			.into_iter()
			.filter(entry_is_dir_or_exit)
			.map(|entry| entry.path())
			.collect()
	};

	for dir in dirs_to_scan {
		for entry in read_dir_or_exit(&dir) {
			let path = entry.path();
			if path.extension().map_or(true, |e| e != "json") {
				continue;
			}

			let coll = match load_collection(&path) {
				Ok(coll) => coll,
				Err(error) => {
					eprintln!("Error loading collection {}: {}", path.display(), error);
					std::process::exit(1);
				}
			};
			let title = coll
				.title
				.get("en")
				.or_else(|| coll.title.get("de"))
				.map(|s| s.as_str())
				.unwrap_or("");
			let count = coll.compositions.len();
			print(&format!("{}\t{}\t({})", coll.id, title, count));
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

	let index = match get_or_build_index(data_dir) {
		Ok(index) => index,
		Err(error) => {
			eprintln!("Error loading dataset: {}", error);
			std::process::exit(1);
		}
	};

	let composer = collection
		.attribution
		.first()
		.and_then(|a| a.composer.as_deref())
		.or(collection.composer.as_deref())
		.unwrap_or_else(|| id.split_once('-').map(|(c, _)| c).unwrap_or(id));

	let catalog_defn = match load_catalog_def(data_dir, &collection.scheme, Some(composer)) {
		Ok(definition) => definition,
		Err(error) => {
			eprintln!("Error loading catalog metadata: {}", error);
			std::process::exit(1);
		}
	};

	if let Some(en) = collection.title.get("en") {
		print(en);
	} else if let Some(de) = collection.title.get("de") {
		print(de);
	}

	print("");

	for num in &collection.compositions {
		let found = match index
			.query()
			.composer(composer)
			.scheme(&collection.scheme)
			.number(num)
			.fetch_one()
		{
			Ok(found) => found,
			Err(error) => {
				eprintln!("Error querying dataset: {}", error);
				std::process::exit(1);
			}
		};

		let formatted_cat = format_catalog(&collection.scheme, num, catalog_defn.as_ref());

		if let Some(comp_id) = found {
			let comp_path = data_dir
				.join("compositions")
				.join(&comp_id[..2])
				.join(format!("{}.json", &comp_id[2..]));

			let comp = match load_composition(&comp_path) {
				Ok(comp) => comp,
				Err(error) => {
					eprintln!("Error loading composition {}: {}", comp_path.display(), error);
					std::process::exit(1);
				}
			};
			let ctx = ExpansionContext {
				composition: &comp,
				collection: None,
				position_in_collection: None,
				config: &config.display,
			};
			let title = expand_title(&ctx);
			print(&format!("{}, {}", title, formatted_cat));
		} else {
			print(&format!("{} (not indexed)", formatted_cat));
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

	for composer_entry in read_dir_or_exit(&collections_dir) {
		if !entry_is_dir_or_exit(&composer_entry) {
			continue;
		}

		for file_entry in read_dir_or_exit(&composer_entry.path()) {
			let path = file_entry.path();
			if path.extension().map_or(true, |e| e != "json") {
				continue;
			}

			let coll = match load_collection(&path) {
				Ok(coll) => coll,
				Err(error) => {
					eprintln!("Error loading collection {}: {}", path.display(), error);
					std::process::exit(1);
				}
			};
			if coll.scheme == scheme && coll.compositions.contains(&number.to_string()) {
				found.push(coll.id.clone());
			}
		}
	}

	if found.is_empty() {
		print(&format!("No collections contain {}:{}", scheme, number));
	} else {
		for id in found {
			print(&id);
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
				std::process::exit(1);
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

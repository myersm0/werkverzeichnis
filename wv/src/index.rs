use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::merge::merge_attribution_with_collections;
use crate::parse::load_composition;
use crate::types::CatalogEntry;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchemeIndex {
	pub current: HashMap<String, String>,
	pub superseded: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct Index {
	pub by_composer: HashMap<String, Vec<String>>,
	pub catalog: HashMap<String, HashMap<String, SchemeIndex>>,
	pub editions: HashMap<String, HashMap<String, HashMap<String, String>>>,
}

pub fn build_index<P: AsRef<Path>>(data_dir: P) -> Index {
	let data_dir = data_dir.as_ref();
	let compositions_dir = data_dir.join("compositions");
	let collections_dir = data_dir.join("collections");

	let mut index = Index::default();

	let entries = match fs::read_dir(&compositions_dir) {
		Ok(e) => e,
		Err(_) => return index,
	};

	for prefix_entry in entries.flatten() {
		if !prefix_entry.path().is_dir() {
			continue;
		}

		let sub_entries = match fs::read_dir(prefix_entry.path()) {
			Ok(e) => e,
			Err(_) => continue,
		};

		for file_entry in sub_entries.flatten() {
			let path = file_entry.path();
			if path.extension().map_or(true, |e| e != "json") {
				continue;
			}

			if let Ok(comp) = load_composition(&path) {
				let merged = merge_attribution_with_collections(&comp.attribution, &collections_dir);

				if let Some(composer) = &merged.composer {
					index
						.by_composer
						.entry(composer.clone())
						.or_default()
						.push(comp.id.clone());

					let mut seen_schemes: HashMap<String, bool> = HashMap::new();

					for cat in &merged.catalog {
						let is_current = !seen_schemes.contains_key(&cat.scheme);
						seen_schemes.insert(cat.scheme.clone(), true);

						add_catalog_entry(&mut index, composer, cat, &comp.id, is_current);
					}
				}
			}
		}
	}

	index
}

fn add_catalog_entry(
	index: &mut Index,
	composer: &str,
	cat: &CatalogEntry,
	id: &str,
	is_current: bool,
) {
	let scheme_index = index
		.catalog
		.entry(composer.to_string())
		.or_default()
		.entry(cat.scheme.clone())
		.or_default();

	if is_current {
		scheme_index.current.insert(cat.number.clone(), id.to_string());
	} else {
		if !scheme_index.current.contains_key(&cat.number) {
			scheme_index.superseded.insert(cat.number.clone(), id.to_string());
		}
	}

	if let Some(edition) = &cat.edition {
		let key = format!("{}-{}", composer, cat.scheme);
		index
			.editions
			.entry(key)
			.or_default()
			.entry(edition.clone())
			.or_default()
			.insert(cat.number.clone(), id.to_string());
	}
}

pub fn load_index<P: AsRef<Path>>(data_dir: P) -> Option<Index> {
	let data_dir = data_dir.as_ref();
	let index_path = data_dir.join(".indexes").join("index.json");
	let composer_path = data_dir.join(".indexes").join("composer-index.json");

	let catalog_content = fs::read_to_string(&index_path).ok()?;
	let composer_content = fs::read_to_string(&composer_path).ok()?;

	let catalog = serde_json::from_str(&catalog_content).ok()?;
	let by_composer = serde_json::from_str(&composer_content).ok()?;

	Some(Index {
		catalog,
		by_composer,
		editions: HashMap::new(),
	})
}

pub fn index_is_stale<P: AsRef<Path>>(data_dir: P) -> bool {
	let data_dir = data_dir.as_ref();
	let index_path = data_dir.join(".indexes").join("index.json");

	let index_mtime = match fs::metadata(&index_path).and_then(|m| m.modified()) {
		Ok(t) => t,
		Err(_) => return true,
	};

	let compositions_dir = data_dir.join("compositions");
	is_any_newer(&compositions_dir, index_mtime)
}

fn is_any_newer(dir: &Path, threshold: std::time::SystemTime) -> bool {
	let entries = match fs::read_dir(dir) {
		Ok(e) => e,
		Err(_) => return true,
	};

	for entry in entries.flatten() {
		let path = entry.path();
		if path.is_dir() {
			if is_any_newer(&path, threshold) {
				return true;
			}
		} else if path.extension().map_or(false, |e| e == "json") {
			if let Ok(meta) = fs::metadata(&path) {
				if let Ok(mtime) = meta.modified() {
					if mtime > threshold {
						return true;
					}
				}
			}
		}
	}
	false
}

pub fn get_or_build_index<P: AsRef<Path>>(data_dir: P) -> Index {
	let data_dir = data_dir.as_ref();

	if !index_is_stale(data_dir) {
		if let Some(index) = load_index(data_dir) {
			return index;
		}
	}

	build_index(data_dir)
}

pub fn write_index<P: AsRef<Path>>(index: &Index, output_path: P) -> std::io::Result<()> {
	let json = serde_json::to_string_pretty(&index.catalog)?;
	fs::write(output_path, json)?;
	Ok(())
}

pub fn write_composer_index<P: AsRef<Path>>(index: &Index, output_path: P) -> std::io::Result<()> {
	let json = serde_json::to_string_pretty(&index.by_composer)?;
	fs::write(output_path, json)?;
	Ok(())
}

pub fn write_edition_indexes<P: AsRef<Path>>(index: &Index, output_dir: P) -> std::io::Result<()> {
	let output_dir = output_dir.as_ref();
	fs::create_dir_all(output_dir)?;

	for (key, editions) in &index.editions {
		for (edition, numbers) in editions {
			let filename = format!("{}-{}.json", key, edition);
			let path = output_dir.join(filename);
			let json = serde_json::to_string_pretty(numbers)?;
			fs::write(path, json)?;
		}
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_add_catalog_entry_current() {
		let mut index = Index::default();
		let cat = CatalogEntry {
			scheme: "bwv".into(),
			number: "846".into(),
			edition: None,
			since: None,
		};

		add_catalog_entry(&mut index, "bach", &cat, "abc12345", true);

		assert!(index.catalog.contains_key("bach"));
		assert!(index.catalog["bach"].contains_key("bwv"));
		assert_eq!(
			index.catalog["bach"]["bwv"].current.get("846"),
			Some(&"abc12345".to_string())
		);
		assert!(index.catalog["bach"]["bwv"].superseded.is_empty());
	}

	#[test]
	fn test_add_catalog_entry_superseded() {
		let mut index = Index::default();
		let cat = CatalogEntry {
			scheme: "k".into(),
			number: "300i".into(),
			edition: Some("6".into()),
			since: None,
		};

		add_catalog_entry(&mut index, "mozart", &cat, "a7a495c0", false);

		assert_eq!(
			index.catalog["mozart"]["k"].superseded.get("300i"),
			Some(&"a7a495c0".to_string())
		);
		assert!(index.catalog["mozart"]["k"].current.is_empty());
	}

	#[test]
	fn test_add_catalog_entry_with_edition() {
		let mut index = Index::default();
		let cat = CatalogEntry {
			scheme: "k".into(),
			number: "332".into(),
			edition: Some("9".into()),
			since: None,
		};

		add_catalog_entry(&mut index, "mozart", &cat, "bdb3e9e8", true);

		assert!(index.catalog["mozart"]["k"].current.contains_key("332"));
		assert!(index.editions.contains_key("mozart-k"));
		assert!(index.editions["mozart-k"]["9"].contains_key("332"));
	}
}

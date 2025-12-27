use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::merge::merge_attribution_with_collections;
use crate::parse::load_composition;
use crate::types::CatalogEntry;

#[derive(Debug, Clone, Default)]
pub struct Index {
	pub by_composer: HashMap<String, Vec<String>>,
	pub catalog: HashMap<String, HashMap<String, HashMap<String, String>>>,
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

					for cat in &merged.catalog {
						add_catalog_entry(&mut index, composer, cat, &comp.id);
					}
				}
			}
		}
	}

	index
}

fn add_catalog_entry(index: &mut Index, composer: &str, cat: &CatalogEntry, id: &str) {
	index
		.catalog
		.entry(composer.to_string())
		.or_default()
		.entry(cat.scheme.clone())
		.or_default()
		.insert(cat.number.clone(), id.to_string());

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
	fn test_add_catalog_entry() {
		let mut index = Index::default();
		let cat = CatalogEntry {
			scheme: "bwv".into(),
			number: "846".into(),
			edition: None,
			since: None,
		};

		add_catalog_entry(&mut index, "bach", &cat, "abc12345");

		assert!(index.catalog.contains_key("bach"));
		assert!(index.catalog["bach"].contains_key("bwv"));
		assert_eq!(index.catalog["bach"]["bwv"]["846"], "abc12345");
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

		add_catalog_entry(&mut index, "mozart", &cat, "bdb3e9e8");

		assert!(index.catalog["mozart"]["k"].contains_key("332"));
		assert!(index.editions.contains_key("mozart-k"));
		assert!(index.editions["mozart-k"]["9"].contains_key("332"));
	}
}

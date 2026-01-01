use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::merge::collection_path_from_id;
use crate::parse::{load_collection, load_composition};
use crate::types::{AttributionEntry, CatalogEntry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
	pub id: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SchemeIndex {
	pub current: HashMap<String, IndexEntry>,
	pub superseded: HashMap<String, IndexEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct Index {
	pub by_composer: HashMap<String, Vec<String>>,
	pub catalog: HashMap<String, HashMap<String, SchemeIndex>>,
	pub editions: HashMap<String, HashMap<String, HashMap<String, String>>>,
}

struct EditionEntry {
	composer: String,
	scheme: String,
	edition: String,
	number: String,
	id: String,
}

fn resolve_composer(attr: &AttributionEntry, collections_dir: &Path) -> Option<String> {
	if let Some(composer) = &attr.composer {
		return Some(composer.clone());
	}
	if let Some(cf) = &attr.cf {
		let path = collection_path_from_id(collections_dir, cf);
		if let Ok(collection) = load_collection(&path) {
			return collection.attribution.first()?.composer.clone();
		}
	}
	None
}

pub fn build_index<P: AsRef<Path>>(data_dir: P) -> Index {
	let data_dir = data_dir.as_ref();
	let compositions_dir = data_dir.join("compositions");
	let collections_dir = data_dir.join("collections");

	let mut index = Index::default();
	let mut edition_entries: Vec<EditionEntry> = Vec::new();

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
				let mut composers_seen: std::collections::HashSet<String> = std::collections::HashSet::new();
				let mut scheme_first_seen: HashMap<(String, String), bool> = HashMap::new();

				for attr in comp.attribution.iter() {
					if let Some(composer) = resolve_composer(attr, &collections_dir) {
						if composers_seen.insert(composer.clone()) {
							index
								.by_composer
								.entry(composer.clone())
								.or_default()
								.push(comp.id.clone());
						}

						if let Some(catalog) = &attr.catalog {
							for cat in catalog {
								let key = (composer.clone(), cat.scheme.clone());
								let is_current = !scheme_first_seen.contains_key(&key);
								scheme_first_seen.insert(key, true);

								add_catalog_entry(&mut index, &composer, cat, &comp.id, is_current);

								if let Some(edition) = &cat.edition {
									edition_entries.push(EditionEntry {
										composer: composer.clone(),
										scheme: cat.scheme.clone(),
										edition: edition.clone(),
										number: cat.number.clone(),
										id: comp.id.clone(),
									});
								}
							}
						}
					}
				}
			}
		}
	}

	build_cumulative_editions(&mut index, &edition_entries);

	index
}

fn build_cumulative_editions(index: &mut Index, entries: &[EditionEntry]) {
	let mut by_scheme: HashMap<(String, String), Vec<&EditionEntry>> = HashMap::new();
	for entry in entries {
		by_scheme
			.entry((entry.composer.clone(), entry.scheme.clone()))
			.or_default()
			.push(entry);
	}

	for ((composer, scheme), scheme_entries) in by_scheme {
		let mut editions: Vec<String> = scheme_entries
			.iter()
			.map(|e| e.edition.clone())
			.collect::<std::collections::HashSet<_>>()
			.into_iter()
			.collect();
		editions.sort_by(|a, b| {
			a.parse::<i32>().unwrap_or(0).cmp(&b.parse::<i32>().unwrap_or(0))
		});

		let mut by_id: HashMap<String, Vec<&EditionEntry>> = HashMap::new();
		for entry in &scheme_entries {
			by_id.entry(entry.id.clone()).or_default().push(entry);
		}

		let key = format!("{}-{}", composer, scheme);

		for edition in &editions {
			let edition_num: i32 = edition.parse().unwrap_or(0);
			let mut edition_map: HashMap<String, String> = HashMap::new();

			for (id, id_entries) in &by_id {
				let best = id_entries
					.iter()
					.filter(|e| e.edition.parse::<i32>().unwrap_or(0) <= edition_num)
					.max_by_key(|e| e.edition.parse::<i32>().unwrap_or(0));

				if let Some(entry) = best {
					edition_map.insert(entry.number.clone(), id.clone());
				}
			}

			index
				.editions
				.entry(key.clone())
				.or_default()
				.insert(edition.clone(), edition_map);
		}
	}
}

fn add_catalog_entry(index: &mut Index, composer: &str, cat: &CatalogEntry, id: &str, is_current: bool) {
	let scheme_index = index
		.catalog
		.entry(composer.to_string())
		.or_default()
		.entry(cat.scheme.clone())
		.or_default();

	let entry = IndexEntry {
		id: id.to_string(),
		note: cat.note.clone(),
	};

	if is_current {
		scheme_index.current.insert(cat.number.clone(), entry);
	} else {
		if !scheme_index.current.contains_key(&cat.number) {
			scheme_index.superseded.insert(cat.number.clone(), entry);
		}
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

pub fn load_edition_index<P: AsRef<Path>>(
	data_dir: P,
	composer: &str,
	scheme: &str,
	edition: &str,
) -> Option<HashMap<String, String>> {
	let filename = format!("{}-{}-{}.json", composer, scheme, edition);
	let path = data_dir.as_ref().join(".indexes").join("editions").join(filename);
	let content = fs::read_to_string(&path).ok()?;
	serde_json::from_str(&content).ok()
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

pub fn write_edition_indexes<P: AsRef<Path>>(index: &Index, data_dir: P) -> std::io::Result<()> {
	let editions_dir = data_dir.as_ref().join(".indexes").join("editions");
	fs::create_dir_all(&editions_dir)?;

	for (key, editions) in &index.editions {
		for (edition, numbers) in editions {
			let filename = format!("{}-{}.json", key, edition);
			let path = editions_dir.join(filename);
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
			note: None,
		};

		add_catalog_entry(&mut index, "bach", &cat, "abc12345", true);

		assert!(index.catalog.contains_key("bach"));
		assert!(index.catalog["bach"].contains_key("bwv"));
		assert_eq!(index.catalog["bach"]["bwv"].current.get("846").map(|e| &e.id), Some(&"abc12345".to_string()));
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
			note: None,
		};

		add_catalog_entry(&mut index, "mozart", &cat, "a7a495c0", false);

		assert_eq!(index.catalog["mozart"]["k"].superseded.get("300i").map(|e| &e.id), Some(&"a7a495c0".to_string()));
		assert!(index.catalog["mozart"]["k"].current.is_empty());
	}

	#[test]
	fn test_cumulative_editions() {
		let mut index = Index::default();
		let entries = vec![
			EditionEntry {
				composer: "mozart".into(),
				scheme: "k".into(),
				edition: "1".into(),
				number: "300i".into(),
				id: "id1".into(),
			},
			EditionEntry {
				composer: "mozart".into(),
				scheme: "k".into(),
				edition: "9".into(),
				number: "331".into(),
				id: "id1".into(),
			},
			EditionEntry {
				composer: "mozart".into(),
				scheme: "k".into(),
				edition: "1".into(),
				number: "545".into(),
				id: "id2".into(),
			},
		];

		build_cumulative_editions(&mut index, &entries);

		// Edition 1: 300i and 545
		assert_eq!(index.editions["mozart-k"]["1"].get("300i"), Some(&"id1".to_string()));
		assert_eq!(index.editions["mozart-k"]["1"].get("545"), Some(&"id2".to_string()));
		assert!(!index.editions["mozart-k"]["1"].contains_key("331"));

		// Edition 9: 331 (supersedes 300i) and 545 (inherited from edition 1)
		assert_eq!(index.editions["mozart-k"]["9"].get("331"), Some(&"id1".to_string()));
		assert_eq!(index.editions["mozart-k"]["9"].get("545"), Some(&"id2".to_string()));
		assert!(!index.editions["mozart-k"]["9"].contains_key("300i"));
	}

	#[test]
	fn test_add_catalog_entry_with_note() {
		let mut index = Index::default();
		let cat = CatalogEntry {
			scheme: "bwv".into(),
			number: "anh. iii 141".into(),
			edition: None,
			since: Some("1990".into()),
			note: Some("spurious attribution".into()),
		};

		add_catalog_entry(&mut index, "bach", &cat, "78129abd", true);

		let entry = index.catalog["bach"]["bwv"].current.get("anh. iii 141").unwrap();
		assert_eq!(entry.id, "78129abd");
		assert_eq!(entry.note, Some("spurious attribution".to_string()));
	}
}

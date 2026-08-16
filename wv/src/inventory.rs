use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::catalog::{
	group_key, group_member_key, load_catalog_def, normalize_catalog_number, sort_numbers,
};
use crate::types::{CatalogDefinition, Inventory};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InventoryCatalogIndex {
	pub complete: bool,
	pub entries: HashSet<String>,
	pub groups: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InventorySchemeIndex {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub default: Option<InventoryCatalogIndex>,
	#[serde(default, skip_serializing_if = "HashMap::is_empty")]
	pub editions: HashMap<String, InventoryCatalogIndex>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InventoryIndex {
	pub catalogs: HashMap<String, HashMap<String, InventorySchemeIndex>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryLookup {
	Known,
	KnownGroup(Vec<String>),
	Absent,
	Unknown,
}

pub fn load_inventory<P: AsRef<Path>>(path: P) -> Result<Inventory, String> {
	let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
	toml::from_str(&content).map_err(|error| error.to_string())
}

pub fn build_inventory_index<P: AsRef<Path>>(data_dir: P) -> InventoryIndex {
	let data_dir = data_dir.as_ref();
	let mut index = InventoryIndex::default();
	let mut paths = Vec::new();
	collect_toml_files(&data_dir.join("inventories"), &mut paths);
	paths.sort();

	for path in paths {
		let Ok(inventory) = load_inventory(&path) else {
			continue;
		};
		let defn = load_catalog_def(data_dir, &inventory.scheme, Some(&inventory.composer));
		let Ok(catalog_index) = normalize_inventory(&inventory, defn.as_ref()) else {
			continue;
		};
		let scheme_index = index
			.catalogs
			.entry(inventory.composer.clone())
			.or_default()
			.entry(inventory.scheme.clone())
			.or_default();
		if let Some(edition) = &inventory.edition {
			scheme_index.editions.insert(edition.clone(), catalog_index);
		} else {
			scheme_index.default = Some(catalog_index);
		}
	}

	index
}

pub fn normalize_inventory(
	inventory: &Inventory,
	defn: Option<&CatalogDefinition>,
) -> Result<InventoryCatalogIndex, String> {
	let mut entries = HashSet::new();
	let mut groups: HashMap<String, Vec<String>> = HashMap::new();

	for raw_number in &inventory.entries {
		let number = normalize_catalog_number(raw_number);
		if !entries.insert(number.clone()) {
			return Err(format!("duplicate inventory entry '{}'", number));
		}

		if let Some(defn) = defn {
			if let Some(key) = group_member_key(&number, defn) {
				groups.entry(key).or_default().push(number);
			}
		}
	}

	if let Some(defn) = defn {
		for members in groups.values_mut() {
			sort_numbers(members, Some(defn));
		}
	}

	Ok(InventoryCatalogIndex {
		complete: inventory.complete,
		entries,
		groups,
	})
}

impl InventoryIndex {
	pub fn catalog<'a>(
		&'a self,
		composer: &str,
		scheme: &str,
		edition: Option<&str>,
		defn: Option<&CatalogDefinition>,
	) -> Option<&'a InventoryCatalogIndex> {
		let scheme_index = self.catalogs.get(composer)?.get(scheme)?;
		if let Some(edition) = edition {
			return scheme_index.editions.get(edition);
		}
		if let Some(default) = scheme_index.default.as_ref() {
			return Some(default);
		}
		defn
			.and_then(|d| d.current_edition.as_deref())
			.and_then(|edition| scheme_index.editions.get(edition))
	}

	pub fn lookup(
		&self,
		composer: &str,
		scheme: &str,
		edition: Option<&str>,
		number: &str,
		defn: Option<&CatalogDefinition>,
	) -> InventoryLookup {
		let Some(catalog) = self.catalog(composer, scheme, edition, defn) else {
			return InventoryLookup::Unknown;
		};
		let number = normalize_catalog_number(number);
		if catalog.entries.contains(&number) {
			return InventoryLookup::Known;
		}
		if let Some(defn) = defn {
			if group_member_key(&number, defn).is_none() {
				if let Some(key) = group_key(&number, defn) {
					if let Some(members) = catalog.groups.get(&key) {
						return InventoryLookup::KnownGroup(members.clone());
					}
				}
			}
		}
		if catalog.complete {
			InventoryLookup::Absent
		} else {
			InventoryLookup::Unknown
		}
	}
}

fn collect_toml_files(dir: &Path, paths: &mut Vec<PathBuf>) {
	let Ok(entries) = fs::read_dir(dir) else {
		return;
	};
	for entry in entries.flatten() {
		let path = entry.path();
		if path.is_dir() {
			collect_toml_files(&path, paths);
		} else if path.extension().map_or(false, |ext| ext == "toml") {
			paths.push(path);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::SortKey;

	fn op_defn() -> CatalogDefinition {
		CatalogDefinition {
			name: "Opus".into(),
			pattern: Some(r"^(\d+)(?:/(\d+))?$".into()),
			sort_keys: Some(vec![
				SortKey { group: 1, sort_type: "int".into(), display: None, none_last: None },
				SortKey { group: 2, sort_type: "int".into(), display: None, none_last: None },
			]),
			group_by: Some(vec![1]),
			..Default::default()
		}
	}

	#[test]
	fn flat_entries_derive_groups() {
		let inventory = Inventory {
			composer: "beethoven".into(),
			scheme: "op".into(),
			edition: None,
			complete: true,
			entries: vec!["2/1".into(), "2/2".into(), "2/3".into(), "4".into()],
		};
		let normalized = normalize_inventory(&inventory, Some(&op_defn())).unwrap();
		let key = group_key("2", &op_defn()).unwrap();
		assert_eq!(normalized.groups.get(&key).unwrap(), &vec!["2/1", "2/2", "2/3"]);
		assert_eq!(normalized.entries.len(), 4);
		assert!(normalized.entries.contains("4"));
		assert!(!normalized.entries.contains("2"));
	}

	#[test]
	fn incomplete_inventory_does_not_turn_missing_child_into_negative() {
		let inventory = Inventory {
			composer: "beethoven".into(),
			scheme: "op".into(),
			edition: None,
			complete: false,
			entries: vec!["2/1".into(), "2/2".into()],
		};
		let catalog = normalize_inventory(&inventory, Some(&op_defn())).unwrap();
		let mut index = InventoryIndex::default();
		index
			.catalogs
			.entry("beethoven".into())
			.or_default()
			.entry("op".into())
			.or_default()
			.default = Some(catalog);
		assert!(matches!(
			index.lookup("beethoven", "op", None, "2/3", Some(&op_defn())),
			InventoryLookup::Unknown
		));
	}

	#[test]
	fn complete_inventory_rejects_missing_child_but_resolves_group() {
		let inventory = Inventory {
			composer: "beethoven".into(),
			scheme: "op".into(),
			edition: None,
			complete: true,
			entries: vec!["2/1".into(), "2/2".into(), "2/3".into()],
		};
		let catalog = normalize_inventory(&inventory, Some(&op_defn())).unwrap();
		let mut index = InventoryIndex::default();
		index
			.catalogs
			.entry("beethoven".into())
			.or_default()
			.entry("op".into())
			.or_default()
			.default = Some(catalog);
		assert!(matches!(
			index.lookup("beethoven", "op", None, "2", Some(&op_defn())),
			InventoryLookup::KnownGroup(_)
		));
		assert!(matches!(
			index.lookup("beethoven", "op", None, "2/4", Some(&op_defn())),
			InventoryLookup::Absent
		));
	}

	#[test]
	fn letter_suffix_is_not_derived_as_a_parent() {
		let defn = CatalogDefinition {
			name: "WoO".into(),
			pattern: Some(r"^(\d+)([a-z])?(?:/(\d+))?$".into()),
			sort_keys: Some(vec![
				SortKey { group: 1, sort_type: "int".into(), display: None, none_last: None },
				SortKey { group: 2, sort_type: "str".into(), display: None, none_last: None },
				SortKey { group: 3, sort_type: "int".into(), display: None, none_last: None },
			]),
			group_by: Some(vec![1, 2]),
			..Default::default()
		};
		assert!(group_member_key("2a", &defn).is_none());
		assert_ne!(group_key("2", &defn), group_key("2a", &defn));
		assert_eq!(group_member_key("2a/1", &defn), group_key("2a", &defn));
	}

	#[test]
	fn exact_parent_and_children_can_coexist() {
		let inventory = Inventory {
			composer: "example".into(),
			scheme: "op".into(),
			edition: None,
			complete: true,
			entries: vec!["2".into(), "2/1".into(), "2/2".into()],
		};
		let normalized = normalize_inventory(&inventory, Some(&op_defn())).unwrap();
		assert!(normalized.entries.contains("2"));
		let key = group_key("2", &op_defn()).unwrap();
		assert_eq!(normalized.groups.get(&key).unwrap(), &vec!["2/1", "2/2"]);
	}

	#[test]
	fn toml_loader_accepts_comments_and_flat_string_entries() {
		let parsed: Inventory = toml::from_str(
			r#"
composer = "beethoven"
scheme = "op"
complete = false
entries = [
    # Piano sonatas
    "2/1", "2/2", "2/3",
    "4", # string quintet
]
"#,
		)
		.unwrap();
		assert_eq!(parsed.entries, vec!["2/1", "2/2", "2/3", "4"]);
	}
}

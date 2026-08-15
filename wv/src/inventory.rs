use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::catalog::{load_catalog_def, normalize_catalog_number};
use crate::types::{CatalogDefinition, Inventory};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryIndexEntry {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub label: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InventoryCatalogIndex {
	pub complete: bool,
	pub entries: HashMap<String, InventoryIndexEntry>,
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
	Known(InventoryIndexEntry),
	KnownGroup(Vec<String>),
	Absent,
	Unknown,
}

pub fn load_inventory<P: AsRef<Path>>(path: P) -> Result<Inventory, String> {
	let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
	serde_json::from_str(&content).map_err(|error| error.to_string())
}

pub fn build_inventory_index<P: AsRef<Path>>(data_dir: P) -> InventoryIndex {
	let data_dir = data_dir.as_ref();
	let mut index = InventoryIndex::default();
	let mut paths = Vec::new();
	collect_json_files(&data_dir.join("inventories"), &mut paths);
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
	let mut entries = HashMap::new();
	let mut groups = HashMap::new();

	for entry in &inventory.entries {
		let number = normalize_catalog_number(&entry.number);
		if let Some(max_member) = entry.member_range {
			if entries.contains_key(&number) || groups.contains_key(&number) {
				return Err(format!("inventory number '{}' is declared more than once", number));
			}
			let member_format = defn
				.and_then(|d| d.member_format.as_deref())
				.ok_or_else(|| format!("catalog '{}' does not define member_format", inventory.scheme))?;
			let mut members = Vec::new();
			for member in 1..=max_member {
				let expanded = normalize_catalog_number(
					&member_format
						.replace("{number}", &number)
						.replace("{member}", &member.to_string()),
				);
				if entries.contains_key(&expanded) {
					return Err(format!("duplicate inventory entry '{}'", expanded));
				}
				entries.insert(
					expanded.clone(),
					InventoryIndexEntry {
						label: entry.label.clone(),
					},
				);
				members.push(expanded);
			}
			groups.insert(number, members);
		} else {
			if entries.contains_key(&number) || groups.contains_key(&number) {
				return Err(format!("inventory number '{}' is declared more than once", number));
			}
			entries.insert(
				number,
				InventoryIndexEntry {
					label: entry.label.clone(),
				},
			);
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
		if let Some(entry) = catalog.entries.get(&number) {
			return InventoryLookup::Known(entry.clone());
		}
		if let Some(members) = catalog.groups.get(&number) {
			return InventoryLookup::KnownGroup(members.clone());
		}
		if catalog.complete {
			InventoryLookup::Absent
		} else {
			InventoryLookup::Unknown
		}
	}
}

fn collect_json_files(dir: &Path, paths: &mut Vec<PathBuf>) {
	let Ok(entries) = fs::read_dir(dir) else {
		return;
	};
	for entry in entries.flatten() {
		let path = entry.path();
		if path.is_dir() {
			collect_json_files(&path, paths);
		} else if path.extension().map_or(false, |ext| ext == "json") {
			paths.push(path);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::{InventoryEntry, SortKey};

	fn op_defn() -> CatalogDefinition {
		CatalogDefinition {
			name: "Opus".into(),
			pattern: Some(r"^(\d+)(?:/(\d+))?$".into()),
			sort_keys: Some(vec![
				SortKey { group: 1, sort_type: "int".into(), display: None, none_last: None },
				SortKey { group: 2, sort_type: "int".into(), display: None, none_last: None },
			]),
			member_format: Some("{number}/{member}".into()),
			..Default::default()
		}
	}

	#[test]
	fn member_range_expands_from_one_through_maximum() {
		let inventory = Inventory {
			composer: "beethoven".into(),
			scheme: "op".into(),
			edition: None,
			complete: true,
			sources: Vec::new(),
			entries: vec![InventoryEntry {
				number: "2".into(),
				member_range: Some(3),
				label: Some("piano sonatas".into()),
			}],
		};
		let normalized = normalize_inventory(&inventory, Some(&op_defn())).unwrap();
		assert_eq!(normalized.groups.get("2").unwrap(), &vec!["2/1", "2/2", "2/3"]);
		assert_eq!(normalized.entries.len(), 3);
		assert!(!normalized.entries.contains_key("2"));
	}

	#[test]
	fn member_range_requires_member_format() {
		let inventory = Inventory {
			composer: "beethoven".into(),
			scheme: "op".into(),
			edition: None,
			complete: false,
			sources: Vec::new(),
			entries: vec![InventoryEntry {
				number: "2".into(),
				member_range: Some(3),
				label: None,
			}],
		};
		assert!(normalize_inventory(&inventory, None).is_err());
	}
}

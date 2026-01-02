use std::path::Path;

use crate::types::{AttributionEntry, CatalogEntry, Dates, Status};

#[derive(Debug, Clone, Default)]
pub struct MergedAttribution {
	pub composer: Option<String>,
	pub dates: Dates,
	pub status: Option<Status>,
	pub catalog: Vec<CatalogEntry>,
	pub notes: Vec<String>,
}

fn merge_dates(base: &mut Dates, overlay: &Dates) {
	if base.composed.is_none() {
		base.composed = overlay.composed;
	}
	if base.published.is_none() {
		base.published = overlay.published;
	}
	if base.premiered.is_none() {
		base.premiered = overlay.premiered;
	}
	if base.revised.is_none() {
		base.revised = overlay.revised;
	}
}

pub fn merge_attribution(entries: &[AttributionEntry]) -> MergedAttribution {
	let mut result = MergedAttribution::default();

	// Status comes from first entry only (describes current attribution)
	if let Some(first) = entries.first() {
		result.status = first.status.clone();
	}

	for entry in entries {
		if result.composer.is_none() {
			result.composer = entry.composer.clone();
		}

		if let Some(dates) = &entry.dates {
			merge_dates(&mut result.dates, dates);
		}

		if let Some(catalog) = &entry.catalog {
			result.catalog.extend(catalog.iter().cloned());
		}

		if let Some(note) = &entry.note {
			result.notes.push(note.clone());
		}
	}

	result
}

pub fn collection_path_from_id<P: AsRef<Path>>(collections_dir: P, id: &str) -> std::path::PathBuf {
	let collections_dir = collections_dir.as_ref();

	// ID format: "composer-name" -> collections/composer/name.json
	if let Some((composer, name)) = id.split_once('-') {
		collections_dir.join(composer).join(format!("{}.json", name))
	} else {
		// Fallback: flat structure
		collections_dir.join(format!("{}.json", id))
	}
}

pub fn current_composer(entries: &[AttributionEntry]) -> Option<&str> {
	entries.iter().find_map(|e| e.composer.as_deref())
}

pub fn current_catalog_number<'a>(entries: &'a [AttributionEntry], scheme: &str) -> Option<&'a str> {
	entries
		.iter()
		.filter_map(|e| e.catalog.as_ref())
		.flatten()
		.find(|c| c.scheme == scheme)
		.map(|c| c.number.as_str())
}

pub fn current_catalog_number_for_edition<'a>(
	entries: &'a [AttributionEntry],
	scheme: &str,
	edition: &str,
) -> Option<&'a str> {
	entries
		.iter()
		.filter_map(|e| e.catalog.as_ref())
		.flatten()
		.find(|c| c.scheme == scheme && c.edition.as_deref() == Some(edition))
		.map(|c| c.number.as_str())
}

pub fn all_catalog_entries(entries: &[AttributionEntry]) -> impl Iterator<Item = &CatalogEntry> {
	entries.iter().filter_map(|e| e.catalog.as_ref()).flatten()
}

pub fn state_as_of(entries: &[AttributionEntry], date: &str) -> Vec<AttributionEntry> {
	entries
		.iter()
		.filter(|e| e.since.as_ref().map_or(true, |s| s.as_str() <= date))
		.cloned()
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_simple_merge() {
		let entries = vec![AttributionEntry {
			composer: Some("mozart".into()),
			cf: None,
			dates: Some(Dates {
				composed: Some(1783),
				published: None,
				premiered: None,
				revised: None,
			}),
			status: None,
			catalog: Some(vec![CatalogEntry {
				scheme: "k".into(),
				number: "332".into(),
				edition: Some("9".into()),
				since: None,
				note: None,
			}]),
			since: None,
			note: None,
		}];

		let merged = merge_attribution(&entries);
		assert_eq!(merged.composer, Some("mozart".into()));
		assert_eq!(merged.dates.composed, Some(1783));
		assert_eq!(merged.catalog.len(), 1);
		assert_eq!(merged.catalog[0].number, "332");
	}

	#[test]
	fn test_merge_multiple_entries() {
		let entries = vec![
			AttributionEntry {
				composer: Some("telemann".into()),
				cf: None,
				dates: Some(Dates {
					composed: Some(1725),
					published: None,
					premiered: None,
					revised: None,
				}),
				status: None,
				catalog: Some(vec![CatalogEntry {
					scheme: "twv".into(),
					number: "1:877".into(),
					edition: None,
					since: None,
					note: None,
				}]),
				since: Some("2020".into()),
				note: None,
			},
			AttributionEntry {
				composer: Some("bach".into()),
				cf: None,
				dates: None,
				status: Some(Status::Spurious),
				catalog: Some(vec![CatalogEntry {
					scheme: "bwv".into(),
					number: "160".into(),
					edition: None,
					since: None,
					note: None,
				}]),
				since: None,
				note: None,
			},
		];

		let merged = merge_attribution(&entries);
		assert_eq!(merged.composer, Some("telemann".into()));
		assert_eq!(merged.dates.composed, Some(1725));
		assert_eq!(merged.catalog.len(), 2);
	}

	#[test]
	fn test_current_composer() {
		let entries = vec![
			AttributionEntry {
				composer: None,
				cf: None,
				dates: Some(Dates {
					composed: Some(1725),
					..Default::default()
				}),
				status: None,
				catalog: None,
				since: None,
				note: None,
			},
			AttributionEntry {
				composer: Some("bach".into()),
				cf: None,
				dates: None,
				status: None,
				catalog: None,
				since: None,
				note: None,
			},
		];

		assert_eq!(current_composer(&entries), Some("bach"));
	}

	#[test]
	fn test_state_as_of() {
		let entries = vec![
			AttributionEntry {
				composer: Some("telemann".into()),
				cf: None,
				dates: None,
				status: None,
				catalog: None,
				since: Some("2020".into()),
				note: None,
			},
			AttributionEntry {
				composer: Some("bach".into()),
				cf: None,
				dates: None,
				status: None,
				catalog: None,
				since: Some("1950".into()),
				note: None,
			},
		];

		let as_of_2000 = state_as_of(&entries, "2000");
		assert_eq!(as_of_2000.len(), 1);
		assert_eq!(as_of_2000[0].composer, Some("bach".into()));

		let as_of_2025 = state_as_of(&entries, "2025");
		assert_eq!(as_of_2025.len(), 2);
	}
}

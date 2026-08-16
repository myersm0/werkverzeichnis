use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::catalog::{
	is_fallback_key, load_catalog_def, looks_like_group, matches_group,
	normalize_catalog_number, sort_key, sort_numbers, CatalogLoadError, SortValue,
};
use crate::index::{load_edition_index, EditionIndexError, Index};
use crate::parse::{load_composer, load_composition, path_for_id, ParseError};
use crate::types::Composition;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum QueryError {
	#[error(transparent)]
	Catalog(#[from] CatalogLoadError),
	#[error(transparent)]
	EditionIndex(#[from] EditionIndexError),
	#[error("failed to load composer metadata {path}: {source}")]
	Composer { path: PathBuf, #[source] source: ParseError },
	#[error("failed to load composition {path}: {source}")]
	Composition { path: PathBuf, #[source] source: ParseError },
	#[error("invalid range endpoint for catalog scheme '{scheme}'")]
	InvalidRangeEndpoint { scheme: String },
}

#[derive(Debug, Clone)]
pub struct QueryResult {
	pub id: String,
	pub number: Option<String>,
	pub superseded: bool,
	pub current_number: Option<String>,
	pub note: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Query {
	composer: Option<String>,
	scheme: Option<String>,
	edition: Option<String>,
	number: Option<String>,
	group: Option<String>,
	range_start: Option<String>,
	range_end: Option<String>,
	sorted: bool,
	strict: bool,
	data_dir: Option<PathBuf>,
}

impl Index {
	pub fn query(&self) -> QueryBuilder<'_> {
		QueryBuilder::new(self)
	}
}

pub struct QueryBuilder<'a> {
	index: &'a Index,
	query: Query,
}

impl<'a> QueryBuilder<'a> {
	pub fn new(index: &'a Index) -> Self {
		Self {
			index,
			query: Query::default(),
		}
	}

	pub fn composer(mut self, c: &str) -> Self {
		self.query.composer = Some(c.to_string());
		self
	}

	pub fn scheme(mut self, s: &str) -> Self {
		self.query.scheme = Some(s.to_string());
		self
	}

	pub fn edition(mut self, e: &str) -> Self {
		self.query.edition = Some(e.to_string());
		self
	}

	pub fn number(mut self, n: &str) -> Self {
		self.query.number = Some(n.to_string());
		self
	}

	pub fn group(mut self, g: &str) -> Self {
		self.query.group = Some(g.to_string());
		self
	}

	pub fn range(mut self, start: &str, end: &str) -> Self {
		self.query.range_start = Some(start.to_string());
		self.query.range_end = Some(end.to_string());
		self
	}

	pub fn sorted(mut self, data_dir: &Path) -> Self {
		self.query.sorted = true;
		self.query.data_dir = Some(data_dir.to_path_buf());
		self
	}

	pub fn strict(mut self, s: bool) -> Self {
		self.query.strict = s;
		self
	}

	pub fn data_dir(mut self, dir: &Path) -> Self {
		self.query.data_dir = Some(dir.to_path_buf());
		self
	}

	pub fn fetch_one(&self) -> Result<Option<String>, QueryError> {
		let Some(composer) = self.query.composer.as_ref() else {
			return Ok(None);
		};
		let Some(scheme) = self.query.scheme.as_ref() else {
			return Ok(None);
		};
		let Some(number) = self.query.number.as_ref() else {
			return Ok(None);
		};

		let normalized = normalize_catalog_number(number);

		if let Some(edition) = &self.query.edition {
			let Some(data_dir) = self.query.data_dir.as_ref() else {
				return Ok(None);
			};
			let Some(edition_index) = load_edition_index(data_dir, composer, scheme, edition)? else {
				return Ok(None);
			};
			return Ok(edition_index.get(&normalized).cloned());
		}

		let Some(scheme_index) = self.index.catalog.get(composer).and_then(|schemes| schemes.get(scheme)) else {
			return Ok(None);
		};

		if let Some(entry) = scheme_index.current.get(&normalized) {
			return Ok(Some(entry.id.clone()));
		}

		if !self.query.strict {
			if let Some(entry) = scheme_index.superseded.get(&normalized) {
				return Ok(Some(entry.id.clone()));
			}
		}

		Ok(None)
	}

	pub fn fetch(&self) -> Result<Vec<QueryResult>, QueryError> {
		match (&self.query.composer, &self.query.scheme, &self.query.number) {
			(Some(composer), Some(scheme), Some(number)) => {
				if let Some(result) = self.fetch_one_with_info()? {
					Ok(vec![result])
				} else {
					let dominated = if let Some(data_dir) = self.query.data_dir.as_ref() {
						load_catalog_def(data_dir, scheme, Some(composer))?
							.as_ref()
							.map(|defn| looks_like_group(number, defn))
							.unwrap_or(false)
					} else {
						false
					};

					if dominated {
						let mut query = self.query.clone();
						query.number = None;
						query.group = Some(number.clone());
						let builder = QueryBuilder {
							index: self.index,
							query,
						};
						builder.fetch_by_scheme(composer, scheme)
					} else {
						Ok(vec![])
					}
				}
			}

			(Some(composer), Some(scheme), None) => self.fetch_by_scheme(composer, scheme),

			(Some(composer), None, None) => self.fetch_by_composer(composer),

			_ => Ok(vec![]),
		}
	}

	fn fetch_one_with_info(&self) -> Result<Option<QueryResult>, QueryError> {
		let Some(composer) = self.query.composer.as_ref() else {
			return Ok(None);
		};
		let Some(scheme) = self.query.scheme.as_ref() else {
			return Ok(None);
		};
		let Some(number) = self.query.number.as_ref() else {
			return Ok(None);
		};

		let normalized = normalize_catalog_number(number);

		if let Some(edition) = &self.query.edition {
			let Some(data_dir) = self.query.data_dir.as_ref() else {
				return Ok(None);
			};
			let Some(edition_index) = load_edition_index(data_dir, composer, scheme, edition)? else {
				return Ok(None);
			};
			let Some(id) = edition_index.get(&normalized).cloned() else {
				return Ok(None);
			};
			return Ok(Some(QueryResult {
				id,
				number: Some(normalized),
				superseded: false,
				current_number: None,
				note: None,
			}));
		}

		let Some(scheme_index) = self.index.catalog.get(composer).and_then(|schemes| schemes.get(scheme)) else {
			return Ok(None);
		};

		if let Some(entry) = scheme_index.current.get(&normalized) {
			return Ok(Some(QueryResult {
				id: entry.id.clone(),
				number: Some(normalized),
				superseded: false,
				current_number: None,
				note: entry.note.clone(),
			}));
		}

		if !self.query.strict {
			if let Some(entry) = scheme_index.superseded.get(&normalized) {
				let current_num = scheme_index
					.current
					.iter()
					.find(|(_, v)| v.id == entry.id)
					.map(|(k, _)| k.clone());

				return Ok(Some(QueryResult {
					id: entry.id.clone(),
					number: Some(normalized),
					superseded: true,
					current_number: current_num,
					note: entry.note.clone(),
				}));
			}
		}

		Ok(None)
	}

	fn fetch_by_composer(&self, composer: &str) -> Result<Vec<QueryResult>, QueryError> {
		let Some(ids) = self.index.by_composer.get(composer) else {
			return Ok(vec![]);
		};

		let ordered_ids = if self.query.sorted {
			self.sort_composer_ids(composer, ids)?
		} else {
			ids.clone()
		};

		Ok(ordered_ids
			.into_iter()
			.map(|id| QueryResult {
				id,
				number: None,
				superseded: false,
				current_number: None,
				note: None,
			})
			.collect())
	}

	fn sort_composer_ids(&self, composer: &str, ids: &[String]) -> Result<Vec<String>, QueryError> {
		let Some(scheme_indexes) = self.index.catalog.get(composer) else {
			let mut sorted = ids.to_vec();
			sorted.sort();
			return Ok(sorted);
		};

		let data_dir = self.query.data_dir.as_deref();
		let default_scheme = if let Some(dir) = data_dir {
			let path = dir.join("composers").join(format!("{}.json", composer));
			match load_composer(&path) {
				Ok(composer) => composer.default_scheme,
				Err(ParseError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => None,
				Err(source) => return Err(QueryError::Composer { path, source }),
			}
		} else {
			None
		};

		let mut schemes: Vec<(String, Option<crate::types::CatalogDefinition>)> = Vec::new();
		for scheme in scheme_indexes.keys() {
			let defn = if let Some(dir) = data_dir {
				load_catalog_def(dir, scheme, Some(composer))?
			} else {
				None
			};
			schemes.push((scheme.clone(), defn));
		}

		schemes.sort_by(|(scheme_a, defn_a), (scheme_b, defn_b)| {
			let rank_a = (
				default_scheme.as_deref() != Some(scheme_a.as_str()),
				!defn_a.as_ref().and_then(|defn| defn.primary).unwrap_or(false),
				scheme_a.as_str(),
			);
			let rank_b = (
				default_scheme.as_deref() != Some(scheme_b.as_str()),
				!defn_b.as_ref().and_then(|defn| defn.primary).unwrap_or(false),
				scheme_b.as_str(),
			);
			rank_a.cmp(&rank_b)
		});

		let mut remaining: HashSet<String> = ids.iter().cloned().collect();
		let mut sorted = Vec::with_capacity(ids.len());

		for (scheme, defn) in schemes {
			let Some(scheme_index) = scheme_indexes.get(&scheme) else {
				continue;
			};
			let mut numbers: Vec<String> = scheme_index.current.keys().cloned().collect();
			sort_numbers(&mut numbers, defn.as_ref());

			for number in numbers {
				let Some(entry) = scheme_index.current.get(&number) else {
					continue;
				};
				if remaining.remove(&entry.id) {
					sorted.push(entry.id.clone());
				}
			}
		}

		let mut uncatalogued: Vec<String> = remaining.into_iter().collect();
		uncatalogued.sort();
		sorted.extend(uncatalogued);
		Ok(sorted)
	}

	fn fetch_by_scheme(&self, composer: &str, scheme: &str) -> Result<Vec<QueryResult>, QueryError> {
		let is_range_or_group = self.query.range_start.is_some() || self.query.group.is_some();

		let numbers: Vec<(String, String, bool, Option<String>)> = if let Some(edition) = &self.query.edition {
			let data_dir = match &self.query.data_dir {
				Some(d) => d,
				None => return Ok(vec![]),
			};
			match load_edition_index(data_dir, composer, scheme, edition)? {
				Some(n) => n.into_iter().map(|(k, v)| (k, v, false, None)).collect(),
				None => return Ok(vec![]),
			}
		} else {
			match self.index.catalog.get(composer).and_then(|s| s.get(scheme)) {
				Some(scheme_index) => {
					let mut entries: Vec<(String, String, bool, Option<String>)> = scheme_index
						.current
						.iter()
						.map(|(k, v)| (k.clone(), v.id.clone(), false, v.note.clone()))
						.collect();

					if !is_range_or_group && !self.query.strict {
						for (k, v) in &scheme_index.superseded {
							entries.push((k.clone(), v.id.clone(), true, v.note.clone()));
						}
					}

					entries
				}
				None => return Ok(vec![]),
			}
		};

		// One pass into a map, so the result stage is a lookup rather than a scan
		// per key. First entry wins, matching the previous `find` behaviour where
		// current entries precede superseded ones.
		let mut entries_by_number: HashMap<String, (String, bool, Option<String>)> =
			HashMap::with_capacity(numbers.len());
		let mut keys: Vec<String> = Vec::with_capacity(numbers.len());
		for (number, id, is_superseded, note) in numbers {
			if !entries_by_number.contains_key(&number) {
				keys.push(number.clone());
				entries_by_number.insert(number, (id, is_superseded, note));
			}
		}

		let defn = if let Some(data_dir) = self.query.data_dir.as_ref() {
			load_catalog_def(data_dir, scheme, Some(composer))?
		} else {
			None
		};

		if self.query.sorted || self.query.group.is_some() || self.query.range_start.is_some() {
			sort_numbers(&mut keys, defn.as_ref());
		}

		if let Some(group) = &self.query.group {
			let normalized_group = normalize_catalog_number(group);
			keys.retain(|k| matches_group(k, &normalized_group, defn.as_ref()));
		}

		if let (Some(start), Some(end)) = (&self.query.range_start, &self.query.range_end) {
			if let Some(ref d) = defn {
				let normalized_start = normalize_catalog_number(start);
				let normalized_end = normalize_catalog_number(end);

				let start_key = sort_key(&normalized_start, d);
				let end_key_raw = sort_key(&normalized_end, d);

				if is_fallback_key(&start_key) || is_fallback_key(&end_key_raw) {
					return Err(QueryError::InvalidRangeEndpoint {
						scheme: scheme.to_string(),
					});
				}

				let end_key = make_inclusive_ceiling(end_key_raw);

				keys.retain(|k| {
					let k_key = sort_key(k, d);
					k_key >= start_key && k_key <= end_key
				});
			} else {
				keys.retain(|k| k >= start && k <= end);
			}
		}

		let scheme_index = self.index.catalog.get(composer).and_then(|s| s.get(scheme));

		// Reverse index built once instead of scanning `current` per superseded hit.
		let current_number_by_id: HashMap<&str, &str> = scheme_index
			.map(|index| {
				index
					.current
					.iter()
					.map(|(number, entry)| (entry.id.as_str(), number.as_str()))
					.collect()
			})
			.unwrap_or_default();

		Ok(keys.into_iter()
			.filter_map(|k| {
				let (id, is_superseded, note) = entries_by_number.get(&k)?;

				let current_num = if *is_superseded {
					current_number_by_id
						.get(id.as_str())
						.map(|number| (*number).to_string())
				} else {
					None
				};

				Some(QueryResult {
					id: id.clone(),
					number: Some(k),
					superseded: *is_superseded,
					current_number: current_num,
					note: note.clone(),
				})
			})
			.collect())
	}

	pub fn fetch_compositions(&self) -> Result<Vec<Composition>, QueryError> {
		let data_dir = match &self.query.data_dir {
			Some(d) => d,
			None => return Ok(vec![]),
		};

		let results = self.fetch()?;
		let compositions_dir = data_dir.join("compositions");
		let mut compositions = Vec::with_capacity(results.len());
		for result in results {
			let path = path_for_id(&compositions_dir, &result.id).map_err(|source| {
				QueryError::Composition {
					path: compositions_dir.join(format!("{}.json", result.id)),
					source,
				}
			})?;
			let composition = load_composition(&path).map_err(|source| QueryError::Composition {
				path: path.clone(),
				source,
			})?;
			compositions.push(composition);
		}
		Ok(compositions)
	}

	pub fn count(&self) -> Result<usize, QueryError> {
		Ok(self.fetch()?.len())
	}

	pub fn exists(&self) -> Result<bool, QueryError> {
		Ok(self.fetch_one()?.is_some())
	}
}

fn make_inclusive_ceiling(key: Vec<SortValue>) -> Vec<SortValue> {
	let mut result = key;
	for i in (0..result.len()).rev() {
		if result[i] == SortValue::NoneFirst {
			result[i] = SortValue::NoneLast;
		} else {
			break;
		}
	}
	result
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::index::{IndexEntry, SchemeIndex};
	use std::collections::HashSet;

	fn make_test_index() -> Index {
		let mut index = Index::default();

		index
			.by_composer
			.insert("bach".into(), vec!["id1".into(), "id2".into()]);
		index
			.by_composer
			.insert("mozart".into(), vec!["id3".into()]);

		let mut bach_bwv = SchemeIndex::default();
		bach_bwv.current.insert("846".into(), IndexEntry { id: "id1".into(), note: None });
		bach_bwv.current.insert("847".into(), IndexEntry { id: "id2".into(), note: None });
		index
			.catalog
			.entry("bach".into())
			.or_default()
			.insert("bwv".into(), bach_bwv);

		let mut mozart_k = SchemeIndex::default();
		mozart_k.current.insert("332".into(), IndexEntry { id: "id3".into(), note: None });
		mozart_k.superseded.insert("300k".into(), IndexEntry { id: "id3".into(), note: None });
		index
			.catalog
			.entry("mozart".into())
			.or_default()
			.insert("k".into(), mozart_k);

		index
	}

	#[test]
	fn test_fetch_one_current() {
		let index = make_test_index();

		let id = index
			.query()
			.composer("bach")
			.scheme("bwv")
			.number("846")
			.fetch_one().unwrap();

		assert_eq!(id, Some("id1".into()));
	}

	#[test]
	fn test_fetch_one_superseded_fallback() {
		let index = make_test_index();

		let id = index
			.query()
			.composer("mozart")
			.scheme("k")
			.number("300k")
			.fetch_one().unwrap();

		assert_eq!(id, Some("id3".into()));
	}

	#[test]
	fn test_fetch_one_superseded_strict() {
		let index = make_test_index();

		let id = index
			.query()
			.composer("mozart")
			.scheme("k")
			.number("300k")
			.strict(true)
			.fetch_one().unwrap();

		assert_eq!(id, None);
	}

	#[test]
	fn test_fetch_one_not_found() {
		let index = make_test_index();

		let id = index
			.query()
			.composer("bach")
			.scheme("bwv")
			.number("999")
			.fetch_one().unwrap();

		assert_eq!(id, None);
	}

	#[test]
	fn test_fetch_by_composer() {
		let index = make_test_index();

		let results = index.query().composer("bach").fetch().unwrap();

		assert_eq!(results.len(), 2);
	}

	#[test]
	fn test_fetch_by_scheme_current_only() {
		let index = make_test_index();

		let results = index.query().composer("mozart").scheme("k").fetch().unwrap();

		assert!(results.iter().any(|r| r.number == Some("332".into())));
	}

	#[test]
	fn test_superseded_has_current_number() {
		let index = make_test_index();

		let results = index
			.query()
			.composer("mozart")
			.scheme("k")
			.number("300k")
			.fetch().unwrap();

		assert_eq!(results.len(), 1);
		assert!(results[0].superseded);
		assert_eq!(results[0].current_number, Some("332".into()));
	}

	#[test]
	fn fetch_by_scheme_resolves_each_number_once() {
		let index = make_test_index();

		let results = index.query().composer("bach").scheme("bwv").fetch().unwrap();

		assert_eq!(results.len(), 2);
		let numbers: HashSet<Option<String>> =
			results.iter().map(|result| result.number.clone()).collect();
		assert_eq!(numbers.len(), 2);
	}

	#[test]
	fn test_count() {
		let index = make_test_index();

		let count = index.query().composer("bach").scheme("bwv").count().unwrap();

		assert_eq!(count, 2);
	}

	#[test]
	fn test_exists() {
		let index = make_test_index();

		assert!(index
			.query()
			.composer("bach")
			.scheme("bwv")
			.number("846")
			.exists()
			.unwrap());

		assert!(!index
			.query()
			.composer("bach")
			.scheme("bwv")
			.number("999")
			.exists()
			.unwrap());
	}

	#[test]
	fn test_make_inclusive_ceiling() {
		assert_eq!(
			make_inclusive_ceiling(vec![
				SortValue::Int(10),
				SortValue::NoneFirst,
				SortValue::NoneFirst
			]),
			vec![
				SortValue::Int(10),
				SortValue::NoneLast,
				SortValue::NoneLast
			]
		);

		assert_eq!(
			make_inclusive_ceiling(vec![SortValue::Int(10), SortValue::Int(1), SortValue::NoneFirst]),
			vec![SortValue::Int(10), SortValue::Int(1), SortValue::NoneLast]
		);

		assert_eq!(
			make_inclusive_ceiling(vec![SortValue::Int(10), SortValue::Int(1), SortValue::Int(2)]),
			vec![SortValue::Int(10), SortValue::Int(1), SortValue::Int(2)]
		);
	}
}

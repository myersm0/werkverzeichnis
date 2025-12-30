use std::path::{Path, PathBuf};

use crate::catalog::{load_catalog_def, matches_group, sort_key, sort_numbers, SortValue};
use crate::index::Index;
use crate::parse::{load_composition, path_for_id};
use crate::types::Composition;

#[derive(Debug, Clone)]
pub struct QueryResult {
	pub id: String,
	pub number: Option<String>,
	pub superseded: bool,
	pub current_number: Option<String>,
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

	pub fn fetch_one(&self) -> Option<String> {
		let composer = self.query.composer.as_ref()?;
		let scheme = self.query.scheme.as_ref()?;
		let number = self.query.number.as_ref()?;

		if let Some(edition) = &self.query.edition {
			let key = format!("{}-{}", composer, scheme);
			self.index
				.editions
				.get(&key)?
				.get(edition)?
				.get(number)
				.cloned()
		} else {
			let scheme_index = self.index.catalog.get(composer)?.get(scheme)?;

			if let Some(id) = scheme_index.current.get(number) {
				return Some(id.clone());
			}

			if !self.query.strict {
				if let Some(id) = scheme_index.superseded.get(number) {
					return Some(id.clone());
				}
			}

			None
		}
	}

	pub fn fetch(&self) -> Vec<QueryResult> {
		match (&self.query.composer, &self.query.scheme, &self.query.number) {
			(Some(composer), Some(scheme), Some(number)) => {
				if let Some(result) = self.fetch_one_with_info() {
					vec![result]
				} else {
					let mut query = self.query.clone();
					query.number = None;
					query.group = Some(number.clone());
					let builder = QueryBuilder {
						index: self.index,
						query,
					};
					builder.fetch_by_scheme(composer, scheme)
				}
			}

			(Some(composer), Some(scheme), None) => self.fetch_by_scheme(composer, scheme),

			(Some(composer), None, None) => self.fetch_by_composer(composer),

			_ => vec![],
		}
	}

	fn fetch_one_with_info(&self) -> Option<QueryResult> {
		let composer = self.query.composer.as_ref()?;
		let scheme = self.query.scheme.as_ref()?;
		let number = self.query.number.as_ref()?;

		if let Some(edition) = &self.query.edition {
			let key = format!("{}-{}", composer, scheme);
			let id = self
				.index
				.editions
				.get(&key)?
				.get(edition)?
				.get(number)?
				.clone();
			return Some(QueryResult {
				id,
				number: Some(number.clone()),
				superseded: false,
				current_number: None,
			});
		}

		let scheme_index = self.index.catalog.get(composer)?.get(scheme)?;

		if let Some(id) = scheme_index.current.get(number) {
			return Some(QueryResult {
				id: id.clone(),
				number: Some(number.clone()),
				superseded: false,
				current_number: None,
			});
		}

		if !self.query.strict {
			if let Some(id) = scheme_index.superseded.get(number) {
				let current_num = scheme_index
					.current
					.iter()
					.find(|(_, v)| *v == id)
					.map(|(k, _)| k.clone());

				return Some(QueryResult {
					id: id.clone(),
					number: Some(number.clone()),
					superseded: true,
					current_number: current_num,
				});
			}
		}

		None
	}

	fn fetch_by_composer(&self, composer: &str) -> Vec<QueryResult> {
		self.index
			.by_composer
			.get(composer)
			.map(|ids| {
				ids.iter()
					.map(|id| QueryResult {
						id: id.clone(),
						number: None,
						superseded: false,
						current_number: None,
					})
					.collect()
			})
			.unwrap_or_default()
	}

	fn fetch_by_scheme(&self, composer: &str, scheme: &str) -> Vec<QueryResult> {
		let is_range_or_group = self.query.range_start.is_some() || self.query.group.is_some();

		let numbers: Vec<(String, String, bool)> = if let Some(edition) = &self.query.edition {
			let key = format!("{}-{}", composer, scheme);
			match self.index.editions.get(&key).and_then(|e| e.get(edition)) {
				Some(n) => n.iter().map(|(k, v)| (k.clone(), v.clone(), false)).collect(),
				None => return vec![],
			}
		} else {
			match self.index.catalog.get(composer).and_then(|s| s.get(scheme)) {
				Some(scheme_index) => {
					let mut entries: Vec<(String, String, bool)> = scheme_index
						.current
						.iter()
						.map(|(k, v)| (k.clone(), v.clone(), false))
						.collect();

					if !is_range_or_group && !self.query.strict {
						for (k, v) in &scheme_index.superseded {
							entries.push((k.clone(), v.clone(), true));
						}
					}

					entries
				}
				None => return vec![],
			}
		};

		let mut keys: Vec<String> = numbers.iter().map(|(k, _, _)| k.clone()).collect();

		let defn = self
			.query
			.data_dir
			.as_ref()
			.and_then(|d| load_catalog_def(d, scheme, Some(composer)));

		if self.query.sorted || self.query.group.is_some() || self.query.range_start.is_some() {
			sort_numbers(&mut keys, defn.as_ref());
		}

		if let Some(group) = &self.query.group {
			if let Some(ref d) = defn {
				keys.retain(|k| matches_group(k, group, Some(d)));
			}
		}

		if let (Some(start), Some(end)) = (&self.query.range_start, &self.query.range_end) {
			if let Some(ref d) = defn {
				let start_key = sort_key(start, d);
				let end_key = make_inclusive_ceiling(sort_key(end, d));

				keys.retain(|k| {
					let k_key = sort_key(k, d);
					k_key >= start_key && k_key <= end_key
				});
			} else {
				keys.retain(|k| k >= start && k <= end);
			}
		}

		let scheme_index = self.index.catalog.get(composer).and_then(|s| s.get(scheme));

		keys.into_iter()
			.filter_map(|k| {
				let entry = numbers.iter().find(|(num, _, _)| num == &k)?;
				let (_, id, is_superseded) = entry;

				let current_num = if *is_superseded {
					scheme_index.and_then(|si| {
						si.current
							.iter()
							.find(|(_, v)| *v == id)
							.map(|(k, _)| k.clone())
					})
				} else {
					None
				};

				Some(QueryResult {
					id: id.clone(),
					number: Some(k),
					superseded: *is_superseded,
					current_number: current_num,
				})
			})
			.collect()
	}

	pub fn fetch_compositions(&self) -> Vec<Composition> {
		let data_dir = match &self.query.data_dir {
			Some(d) => d,
			None => return vec![],
		};

		let results = self.fetch();
		let compositions_dir = data_dir.join("compositions");

		results
			.into_iter()
			.filter_map(|r| {
				let path = path_for_id(&compositions_dir, &r.id).ok()?;
				load_composition(&path).ok()
			})
			.collect()
	}

	pub fn count(&self) -> usize {
		self.fetch().len()
	}

	pub fn exists(&self) -> bool {
		self.fetch_one().is_some()
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
	use crate::index::SchemeIndex;

	fn make_test_index() -> Index {
		let mut index = Index::default();

		index
			.by_composer
			.insert("bach".into(), vec!["id1".into(), "id2".into()]);
		index
			.by_composer
			.insert("mozart".into(), vec!["id3".into()]);

		let mut bach_bwv = SchemeIndex::default();
		bach_bwv.current.insert("846".into(), "id1".into());
		bach_bwv.current.insert("847".into(), "id2".into());
		index
			.catalog
			.entry("bach".into())
			.or_default()
			.insert("bwv".into(), bach_bwv);

		let mut mozart_k = SchemeIndex::default();
		mozart_k.current.insert("332".into(), "id3".into());
		mozart_k.superseded.insert("300k".into(), "id3".into());
		index
			.catalog
			.entry("mozart".into())
			.or_default()
			.insert("k".into(), mozart_k);

		let key = "mozart-k".to_string();
		index
			.editions
			.entry(key.clone())
			.or_default()
			.entry("6".into())
			.or_default()
			.insert("300k".into(), "id3".into());

		index
			.editions
			.entry(key)
			.or_default()
			.entry("9".into())
			.or_default()
			.insert("332".into(), "id3".into());

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
			.fetch_one();

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
			.fetch_one();

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
			.fetch_one();

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
			.fetch_one();

		assert_eq!(id, None);
	}

	#[test]
	fn test_fetch_by_composer() {
		let index = make_test_index();

		let results = index.query().composer("bach").fetch();

		assert_eq!(results.len(), 2);
	}

	#[test]
	fn test_fetch_by_scheme_current_only() {
		let index = make_test_index();

		let results = index.query().composer("mozart").scheme("k").fetch();

		assert!(results.iter().any(|r| r.number == Some("332".into())));
	}

	#[test]
	fn test_fetch_by_edition() {
		let index = make_test_index();

		let id = index
			.query()
			.composer("mozart")
			.scheme("k")
			.edition("6")
			.number("300k")
			.fetch_one();

		assert_eq!(id, Some("id3".into()));
	}

	#[test]
	fn test_superseded_has_current_number() {
		let index = make_test_index();

		let results = index
			.query()
			.composer("mozart")
			.scheme("k")
			.number("300k")
			.fetch();

		assert_eq!(results.len(), 1);
		assert!(results[0].superseded);
		assert_eq!(results[0].current_number, Some("332".into()));
	}

	#[test]
	fn test_count() {
		let index = make_test_index();

		let count = index.query().composer("bach").scheme("bwv").count();

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
			.exists());

		assert!(!index
			.query()
			.composer("bach")
			.scheme("bwv")
			.number("999")
			.exists());
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

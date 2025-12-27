use std::path::{Path, PathBuf};

use crate::catalog::{load_catalog_def, matches_group, sort_numbers};
use crate::index::Index;
use crate::parse::{load_composition, path_for_id};
use crate::types::Composition;

#[derive(Debug, Clone)]
pub struct QueryResult {
	pub id: String,
	pub number: Option<String>,
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
			self.index
				.catalog
				.get(composer)?
				.get(scheme)?
				.get(number)
				.cloned()
		}
	}

	pub fn fetch(&self) -> Vec<QueryResult> {
		match (&self.query.composer, &self.query.scheme, &self.query.number) {
			(Some(composer), Some(scheme), Some(number)) => {
				if let Some(id) = self.fetch_one() {
					vec![QueryResult {
						id,
						number: Some(number.clone()),
					}]
				} else {
					// No exact match - try group matching
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

			(Some(composer), Some(scheme), None) => {
				self.fetch_by_scheme(composer, scheme)
			}

			(Some(composer), None, None) => {
				self.fetch_by_composer(composer)
			}

			_ => vec![],
		}
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
					})
					.collect()
			})
			.unwrap_or_default()
	}

	fn fetch_by_scheme(&self, composer: &str, scheme: &str) -> Vec<QueryResult> {
		let numbers = if let Some(edition) = &self.query.edition {
			let key = format!("{}-{}", composer, scheme);
			match self.index.editions.get(&key).and_then(|e| e.get(edition)) {
				Some(n) => n,
				None => return vec![],
			}
		} else {
			match self.index.catalog.get(composer).and_then(|s| s.get(scheme)) {
				Some(n) => n,
				None => return vec![],
			}
		};

		let mut keys: Vec<String> = numbers.keys().cloned().collect();

		let defn = self.query.data_dir.as_ref().and_then(|d| load_catalog_def(d, scheme, Some(composer)));

		if self.query.sorted || self.query.group.is_some() || self.query.range_start.is_some() {
			sort_numbers(&mut keys, defn.as_ref());
		}

		if let Some(group) = &self.query.group {
			if let Some(ref d) = defn {
				keys.retain(|k| matches_group(k, group, Some(d)));
			}
		}

		if let (Some(start), Some(end)) = (&self.query.range_start, &self.query.range_end) {
			let start_idx = keys.iter().position(|k| {
				if let Some(ref d) = defn {
					matches_group(k, start, Some(d)) || k == start
				} else {
					k == start
				}
			});
			let end_idx = keys.iter().rposition(|k| {
				if let Some(ref d) = defn {
					matches_group(k, end, Some(d)) || k == end
				} else {
					k == end
				}
			});

			if let (Some(s), Some(e)) = (start_idx, end_idx) {
				if s <= e {
					keys = keys[s..=e].to_vec();
				}
			}
		}

		keys.into_iter()
			.map(|k| {
				let id = numbers.get(&k).unwrap().clone();
				QueryResult { id, number: Some(k) }
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

#[cfg(test)]
mod tests {
	use super::*;

	fn make_test_index() -> Index {
		let mut index = Index::default();

		index.by_composer.insert("bach".into(), vec!["id1".into(), "id2".into()]);
		index.by_composer.insert("mozart".into(), vec!["id3".into()]);

		index
			.catalog
			.entry("bach".into())
			.or_default()
			.entry("bwv".into())
			.or_default()
			.insert("846".into(), "id1".into());

		index
			.catalog
			.entry("bach".into())
			.or_default()
			.entry("bwv".into())
			.or_default()
			.insert("847".into(), "id2".into());

		index
			.catalog
			.entry("mozart".into())
			.or_default()
			.entry("k".into())
			.or_default()
			.insert("332".into(), "id3".into());

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
	fn test_fetch_one() {
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
	fn test_fetch_by_scheme() {
		let index = make_test_index();

		let results = index.query().composer("bach").scheme("bwv").fetch();

		assert_eq!(results.len(), 2);
		assert!(results.iter().any(|r| r.number == Some("846".into())));
		assert!(results.iter().any(|r| r.number == Some("847".into())));
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
}

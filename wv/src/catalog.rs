use regex::{Regex, RegexBuilder};
use serde_json::Value;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::parse::load_composer;
use crate::types::{CatalogConstraint, CatalogDefinition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortValue {
	Int(i64),
	Str(String),
	NoneFirst,
	NoneLast,
}

impl PartialOrd for SortValue {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for SortValue {
	fn cmp(&self, other: &Self) -> Ordering {
		use SortValue::*;
		match (self, other) {
			(NoneFirst, NoneFirst) => Ordering::Equal,
			(NoneFirst, _) => Ordering::Less,
			(_, NoneFirst) => Ordering::Greater,
			(NoneLast, NoneLast) => Ordering::Equal,
			(NoneLast, _) => Ordering::Greater,
			(_, NoneLast) => Ordering::Less,
			(Int(a), Int(b)) => a.cmp(b),
			(Str(a), Str(b)) => a.cmp(b),
			(Int(_), Str(_)) => Ordering::Less,
			(Str(_), Int(_)) => Ordering::Greater,
		}
	}
}

fn parse_roman(s: &str) -> i64 {
	let s = s.to_uppercase();
	if !s.chars().all(|c| "IVXLCDM".contains(c)) {
		return 0;
	}

	let vals: HashMap<char, i64> =
		[('I', 1), ('V', 5), ('X', 10), ('L', 50), ('C', 100), ('D', 500), ('M', 1000)]
			.into_iter()
			.collect();

	let mut total: i64 = 0;
	let mut prev: i64 = 0;

	for c in s.chars().rev() {
		let val = *vals.get(&c).unwrap_or(&0);
		if val < prev {
			total -= val;
		} else {
			total += val;
		}
		prev = val;
	}
	total
}

type CatalogCacheKey = (PathBuf, String, Option<String>);

thread_local! {
	static CATALOG_CACHE: RefCell<HashMap<CatalogCacheKey, Option<CatalogDefinition>>> =
		RefCell::new(HashMap::new());
}

/// Catalog definitions are read from disk on nearly every result row and every
/// validated catalog reference. They cannot change during a single run, so they
/// are memoized for the life of the process (per thread).
///
/// Call [`clear_catalog_cache`] if you rewrite composer or catalog files and
/// need subsequent reads to see them.
pub fn load_catalog_def<P: AsRef<Path>>(
	data_dir: P,
	scheme: &str,
	composer: Option<&str>,
) -> Option<CatalogDefinition> {
	let data_dir = data_dir.as_ref();
	let key: CatalogCacheKey = (
		data_dir.to_path_buf(),
		scheme.to_string(),
		composer.map(str::to_string),
	);

	if let Some(cached) = CATALOG_CACHE.with(|cache| cache.borrow().get(&key).cloned()) {
		return cached;
	}

	let definition = read_catalog_def(data_dir, scheme, composer);
	CATALOG_CACHE.with(|cache| {
		cache.borrow_mut().insert(key, definition.clone());
	});
	definition
}

pub fn clear_catalog_cache() {
	CATALOG_CACHE.with(|cache| cache.borrow_mut().clear());
}

fn read_catalog_def(
	data_dir: &Path,
	scheme: &str,
	composer: Option<&str>,
) -> Option<CatalogDefinition> {
	let composer_def = composer.and_then(|composer_slug| {
		let composer_path = data_dir.join("composers").join(format!("{}.json", composer_slug));
		load_composer(&composer_path)
			.ok()
			.and_then(|composer_data| composer_data.catalogs)
			.and_then(|catalogs| catalogs.get(scheme).cloned())
	});

	let global_path = data_dir.join("catalogs").join(format!("{}.json", scheme));
	let global_def: Option<CatalogDefinition> = std::fs::read_to_string(&global_path)
		.ok()
		.and_then(|content| serde_json::from_str(&content).ok());

	match (composer_def, global_def) {
		(Some(composer_def), Some(global_def)) => {
			Some(merge_catalog_definitions(&global_def, &composer_def))
		}
		(Some(composer_def), None) => Some(composer_def),
		(None, Some(global_def)) => Some(global_def),
		(None, None) => None,
	}
}

/// Overlay a composer-specific definition onto the shared one.
///
/// Done through serde rather than field by field so that adding a field to
/// [`CatalogDefinition`] cannot silently stop it being inherited. Every field
/// present on the composer definition wins; everything else falls back to the
/// global definition. Constraints are the one exception: they accumulate, so a
/// shared scheme can state structural rules that a composer then narrows.
pub fn merge_catalog_definitions(
	global: &CatalogDefinition,
	composer: &CatalogDefinition,
) -> CatalogDefinition {
	let constraints = match (global.constraints.clone(), composer.constraints.clone()) {
		(Some(mut shared), Some(local)) => {
			shared.extend(local);
			Some(shared)
		}
		(Some(shared), None) => Some(shared),
		(None, local) => local,
	};

	let merged = match (serde_json::to_value(global), serde_json::to_value(composer)) {
		(Ok(Value::Object(mut base)), Ok(Value::Object(overlay))) => {
			for (field, value) in overlay {
				base.insert(field, value);
			}
			serde_json::from_value(Value::Object(base)).ok()
		}
		_ => None,
	};

	let mut definition = merged.unwrap_or_else(|| composer.clone());
	definition.constraints = constraints;
	definition
}

fn parse_number_with_regex(number: &str, re: &Regex, max_group: usize) -> Option<Vec<Option<String>>> {
	let caps = re.captures(number)?;

	let mut result = Vec::new();
	for i in 1..=max_group {
		result.push(caps.get(i).map(|m| m.as_str().to_string()));
	}
	Some(result)
}

fn sort_key_with_regex(number: &str, re: &Regex, defn: &CatalogDefinition, max_group: usize) -> Vec<SortValue> {
	let captures = match parse_number_with_regex(number, re, max_group) {
		Some(c) => c,
		None => return vec![SortValue::Int(999999999), SortValue::Str(number.to_string())],
	};

	let sort_keys = match &defn.sort_keys {
		Some(sks) => sks,
		None => return vec![SortValue::Str(number.to_string())],
	};

	let mut key = Vec::new();

	for sk in sort_keys {
		let idx = sk.group - 1;
		let raw = captures.get(idx).and_then(|o| o.clone());
		let typ = sk.sort_type.as_str();

		let missing = if sk.none_last.unwrap_or(false) {
			SortValue::NoneLast
		} else {
			SortValue::NoneFirst
		};

		match raw {
			None => {
				key.push(missing);
			}
			Some(s) if s.is_empty() => {
				key.push(missing);
			}
			Some(s) => match typ {
				"int" => {
					let val = s.parse::<i64>().unwrap_or(0);
					key.push(SortValue::Int(val));
				}
				"roman" => {
					let val = parse_roman(&s);
					key.push(SortValue::Int(val));
				}
				_ => {
					key.push(SortValue::Str(s));
				}
			},
		}
	}

	key
}

pub fn sort_key(number: &str, defn: &CatalogDefinition) -> Vec<SortValue> {
	let pattern = match &defn.pattern {
		Some(p) => p,
		None => return vec![SortValue::Str(number.to_string())],
	};
	let re = match RegexBuilder::new(pattern).case_insensitive(true).build() {
		Ok(r) => r,
		Err(_) => return vec![SortValue::Int(999999999), SortValue::Str(number.to_string())],
	};
	let max_group = defn
		.sort_keys
		.as_ref()
		.map(|sks| sks.iter().map(|sk| sk.group).max().unwrap_or(0))
		.unwrap_or(0);

	sort_key_with_regex(number, &re, defn, max_group)
}

pub fn sort_numbers(numbers: &mut [String], defn: Option<&CatalogDefinition>) {
	match defn {
		Some(d) => {
			let pattern = match &d.pattern {
				Some(p) => p,
				None => {
					numbers.sort();
					return;
				}
			};
			let re = match RegexBuilder::new(pattern).case_insensitive(true).build() {
				Ok(r) => r,
				Err(_) => {
					numbers.sort();
					return;
				}
			};
			let max_group = d
				.sort_keys
				.as_ref()
				.map(|sks| sks.iter().map(|sk| sk.group).max().unwrap_or(0))
				.unwrap_or(0);

			numbers.sort_by(|a, b| {
				sort_key_with_regex(a, &re, d, max_group)
					.cmp(&sort_key_with_regex(b, &re, d, max_group))
			});
		}
		None => numbers.sort(),
	}
}

pub fn sort_numbers_by_scheme<P: AsRef<Path>>(
	numbers: &mut [String],
	data_dir: P,
	scheme: &str,
	composer: Option<&str>,
) {
	let defn = load_catalog_def(data_dir, scheme, composer);
	sort_numbers(numbers, defn.as_ref());
}

pub fn matches_group(number: &str, group: &str, defn: Option<&CatalogDefinition>) -> bool {
	let defn = match defn {
		Some(d) => d,
		None => return number.starts_with(group),
	};

	let pattern = match &defn.pattern {
		Some(p) => p,
		None => return number.starts_with(group),
	};

	let re = match RegexBuilder::new(pattern).case_insensitive(true).build() {
		Ok(r) => r,
		Err(_) => return number.starts_with(group),
	};

	let max_group = defn
		.sort_keys
		.as_ref()
		.map(|sks| sks.iter().map(|sk| sk.group).max().unwrap_or(0))
		.unwrap_or(0);

	let num_captures = match parse_number_with_regex(number, &re, max_group) {
		Some(c) => c,
		None => return false,
	};

	let grp_captures = match parse_number_with_regex(group, &re, max_group) {
		Some(c) => c,
		None => return number.starts_with(group),
	};

	let groups_to_compare: Vec<usize> = match &defn.group_by {
		Some(gb) => gb.clone(),
		None => {
			defn.sort_keys
				.as_ref()
				.map(|sks| {
					let groups: Vec<usize> = sks.iter().map(|sk| sk.group).collect();
					if groups.len() > 1 {
						groups[..groups.len() - 1].to_vec()
					} else {
						groups
					}
				})
				.unwrap_or_default()
		}
	};

	for &grp_idx in &groups_to_compare {
		if grp_idx == 0 || grp_idx > max_group {
			continue;
		}
		let num_val = num_captures.get(grp_idx - 1).and_then(|v| v.as_ref());
		let grp_val = grp_captures.get(grp_idx - 1).and_then(|v| v.as_ref());

		match (num_val, grp_val) {
			(Some(n), Some(g)) => {
				if n != g {
					return false;
				}
			}
			(None, None) => {}
			_ => return false,
		}
	}

	true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogDomainError {
	BelowMinimum { group: usize, name: Option<String>, value: i64, min: i64 },
	AboveMaximum { group: usize, name: Option<String>, value: i64, max: i64 },
	OutsideRanges { group: usize, name: Option<String>, value: i64 },
}

impl std::fmt::Display for CatalogDomainError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::BelowMinimum { group, name, value, min } => {
				let name = name.clone().unwrap_or_else(|| format!("component {}", group));
				write!(f, "{} {} is below the minimum {}", name, value, min)
			}
			Self::AboveMaximum { group, name, value, max } => {
				let name = name.clone().unwrap_or_else(|| format!("component {}", group));
				write!(f, "{} {} is above the maximum {}", name, value, max)
			}
			Self::OutsideRanges { group, name, value } => {
				let name = name.clone().unwrap_or_else(|| format!("component {}", group));
				write!(f, "{} {} is outside the allowed ranges", name, value)
			}
		}
	}
}

fn constraint_accepts(value: i64, constraint: &CatalogConstraint) -> Result<(), CatalogDomainError> {
	if let Some(min) = constraint.min {
		if value < min {
			return Err(CatalogDomainError::BelowMinimum { group: constraint.group, name: constraint.name.clone(), value, min });
		}
	}
	if let Some(max) = constraint.max {
		if value > max {
			return Err(CatalogDomainError::AboveMaximum { group: constraint.group, name: constraint.name.clone(), value, max });
		}
	}
	if let Some(ranges) = &constraint.ranges {
		if !ranges.iter().any(|range| value >= range.min && value <= range.max) {
			return Err(CatalogDomainError::OutsideRanges { group: constraint.group, name: constraint.name.clone(), value });
		}
	}
	Ok(())
}

pub fn validate_catalog_domain(number: &str, defn: &CatalogDefinition) -> Result<(), CatalogDomainError> {
	let Some(pattern) = &defn.pattern else {
		return Ok(());
	};
	let Ok(re) = RegexBuilder::new(pattern).case_insensitive(true).build() else {
		return Ok(());
	};
	let Some(captures) = re.captures(number) else {
		return Ok(());
	};

	if let Some(constraints) = &defn.constraints {
		for constraint in constraints {
			let Some(value) = captures.get(constraint.group) else {
				continue;
			};
			let sort_type = defn
				.sort_keys
				.as_ref()
				.and_then(|keys| keys.iter().find(|key| key.group == constraint.group))
				.map(|key| key.sort_type.as_str())
				.unwrap_or("int");
			let value = match sort_type {
				"roman" => parse_roman(value.as_str()),
				_ => match value.as_str().parse::<i64>() {
					Ok(value) => value,
					Err(_) => continue,
				},
			};
			constraint_accepts(value, constraint)?;
		}
	}

	Ok(())
}

pub fn normalize_catalog_number(number: &str) -> String {
	number.to_lowercase()
}

pub(crate) fn group_key(number: &str, defn: &CatalogDefinition) -> Option<String> {
	group_key_inner(number, defn).map(|(key, _)| key)
}

pub(crate) fn group_member_key(number: &str, defn: &CatalogDefinition) -> Option<String> {
	let (key, has_detail) = group_key_inner(number, defn)?;
	if has_detail {
		Some(key)
	} else {
		None
	}
}

fn group_key_inner(number: &str, defn: &CatalogDefinition) -> Option<(String, bool)> {
	let group_by = defn.group_by.as_ref()?;
	if group_by.is_empty() {
		return None;
	}
	let pattern = defn.pattern.as_ref()?;
	let re = RegexBuilder::new(pattern).case_insensitive(true).build().ok()?;
	let captures = re.captures(number)?;
	let grouped: HashSet<usize> = group_by.iter().copied().collect();
	let mut parts = Vec::with_capacity(group_by.len());

	for &index in group_by {
		if index == 0 || index >= captures.len() {
			return None;
		}
		match captures.get(index) {
			Some(value) => {
				let value = normalize_catalog_number(value.as_str());
				parts.push(format!("{}:{}", value.len(), value));
			}
			None => parts.push("-".into()),
		}
	}

	let has_detail = (1..captures.len()).any(|index| {
		!grouped.contains(&index)
			&& captures
				.get(index)
				.map_or(false, |value| !value.as_str().is_empty())
	});

	Some((parts.join("|"), has_detail))
}

pub fn is_fallback_key(key: &[SortValue]) -> bool {
	matches!(key.first(), Some(SortValue::Int(999999999)))
}

pub fn looks_like_group(number: &str, defn: &CatalogDefinition) -> bool {
	let key = sort_key(number, defn);
	if is_fallback_key(&key) {
		return false;
	}
	key.iter().rev().take_while(|v| **v == SortValue::NoneFirst).count() > 0
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::SortKey;

	#[test]
	fn test_parse_roman() {
		assert_eq!(parse_roman("I"), 1);
		assert_eq!(parse_roman("IV"), 4);
		assert_eq!(parse_roman("IX"), 9);
		assert_eq!(parse_roman("XIV"), 14);
		assert_eq!(parse_roman("XLII"), 42);
		assert_eq!(parse_roman("MCMXCIV"), 1994);
	}

	#[test]
	fn test_sort_value_ordering() {
		use SortValue::*;
		assert!(NoneFirst < Int(0));
		assert!(Int(0) < Int(1));
		assert!(Int(100) < NoneLast);
		assert!(Str("a".into()) < Str("b".into()));
	}

	#[test]
	fn test_simple_numeric_sort() {
		let defn = CatalogDefinition {
			name: "Test".into(),
			description: None,
			canonical_format: None,
			pattern: Some(r"^(\d+)$".into()),
			sort_keys: Some(vec![SortKey {
				group: 1,
				sort_type: "int".into(),
				display: None,
				none_last: None,
			}]),
			group_by: None,
			aliases: None,
			editions: None,
			..Default::default()
		};

		let mut nums: Vec<String> = vec!["10", "2", "1", "20"]
			.into_iter()
			.map(String::from)
			.collect();

		sort_numbers(&mut nums, Some(&defn));
		assert_eq!(nums, vec!["1", "2", "10", "20"]);
	}

	#[test]
	fn test_opus_subnumber_sort() {
		let defn = CatalogDefinition {
			name: "Op".into(),
			description: None,
			canonical_format: None,
			pattern: Some(r"^(\d+)(?:/(\d+))?([a-z])?$".into()),
			sort_keys: Some(vec![
				SortKey { group: 1, sort_type: "int".into(), display: None, none_last: None },
				SortKey { group: 2, sort_type: "int".into(), display: None, none_last: None },
				SortKey { group: 3, sort_type: "str".into(), display: None, none_last: None },
			]),
			group_by: None,
			aliases: None,
			editions: None,
			..Default::default()
		};

		let mut nums: Vec<String> = vec!["2/1", "10", "2", "2/10", "2/2"]
			.into_iter()
			.map(String::from)
			.collect();

		sort_numbers(&mut nums, Some(&defn));
		assert_eq!(nums, vec!["2", "2/1", "2/2", "2/10", "10"]);
	}

	#[test]
	fn test_none_last_sort() {
		let defn = CatalogDefinition {
			name: "Test".into(),
			pattern: Some(r"^(\d+)(?:/(\d+))?$".into()),
			sort_keys: Some(vec![
				SortKey { group: 1, sort_type: "int".into(), display: None, none_last: None },
				SortKey { group: 2, sort_type: "int".into(), display: None, none_last: Some(true) },
			]),
			..Default::default()
		};

		let mut nums: Vec<String> = vec!["2", "2/2", "2/1"]
			.into_iter()
			.map(String::from)
			.collect();

		sort_numbers(&mut nums, Some(&defn));
		assert_eq!(nums, vec!["2/1", "2/2", "2"]);
	}

	#[test]
	fn test_normalize_catalog_number() {
		assert_eq!(normalize_catalog_number("300K"), "300k");
		assert_eq!(normalize_catalog_number("331A"), "331a");
		assert_eq!(normalize_catalog_number("I:13"), "i:13");
		assert_eq!(normalize_catalog_number("XVI:52"), "xvi:52");
		assert_eq!(normalize_catalog_number("BWV 846"), "bwv 846");
	}

	#[test]
	fn structural_constraints_reject_out_of_range_components() {
		let defn: CatalogDefinition = serde_json::from_str(r#"{
			"name":"Opus",
			"pattern":"^(\\d+)(?:/(\\d+))?$",
			"constraints":[
				{"group":1,"name":"opus number","min":1,"max":138},
				{"group":2,"name":"sub-number","min":1}
			]
		}"#).unwrap();

		assert!(validate_catalog_domain("2/1", &defn).is_ok());
		assert!(matches!(
			validate_catalog_domain("2/0", &defn),
			Err(CatalogDomainError::BelowMinimum { .. })
		));
		assert!(matches!(
			validate_catalog_domain("139", &defn),
			Err(CatalogDomainError::AboveMaximum { .. })
		));
	}

	#[test]
	fn structural_constraints_support_discontinuous_ranges() {
		let defn: CatalogDefinition = serde_json::from_str(r#"{
			"name":"TVWV",
			"pattern":"^(\\d+):(\\d+)$",
			"constraints":[{"group":1,"name":"genre","ranges":[{"min":1,"max":15},{"min":20,"max":25}]}]
		}"#).unwrap();

		assert!(validate_catalog_domain("15:1", &defn).is_ok());
		assert!(validate_catalog_domain("20:1", &defn).is_ok());
		assert!(matches!(
			validate_catalog_domain("16:1", &defn),
			Err(CatalogDomainError::OutsideRanges { .. })
		));
	}

	#[test]
	fn structural_constraints_support_roman_components() {
		let defn: CatalogDefinition = serde_json::from_str(r#"{
			"name":"Hoboken",
			"pattern":"^([ivxlcdm]+):(\\d+)$",
			"sort_keys":[{"group":1,"type":"roman"},{"group":2,"type":"int"}],
			"constraints":[{"group":1,"name":"category","min":1,"max":32}]
		}"#).unwrap();

		assert!(validate_catalog_domain("iii:32", &defn).is_ok());
		assert!(matches!(
			validate_catalog_domain("lvi:1", &defn),
			Err(CatalogDomainError::AboveMaximum { .. })
		));
	}

	fn fully_populated_global() -> CatalogDefinition {
		serde_json::from_str(r#"{
			"id": "op",
			"name": "Opus number",
			"description": "shared description",
			"canonical_format": "op. {number}",
			"pattern": "^(\\d+)(?:/(\\d+))?$",
			"sort_keys": [{"group": 1, "type": "int"}, {"group": 2, "type": "int"}],
			"group_by": [1],
			"examples": [{"number": "2/1", "display": "op. 2 no. 1"}],
			"aliases": ["opus"],
			"editions": {"1": {"year": 1850, "editor": "Shared"}},
			"current_edition": "1",
			"categories": {"posth": "Posthumous"},
			"constraints": [{"group": 2, "name": "sub-number", "min": 1}],
			"primary": true,
			"mb_format": "op. {number}",
			"mb_part_format": "no. {part}"
		}"#).unwrap()
	}

	#[test]
	fn merge_inherits_every_global_field() {
		let global = fully_populated_global();
		let composer = CatalogDefinition {
			name: "Opus number (Beethoven)".into(),
			..Default::default()
		};

		// Exhaustive destructuring on purpose: adding a field to CatalogDefinition
		// stops this compiling, which forces a decision about how it merges.
		let CatalogDefinition {
			id,
			name,
			description,
			canonical_format,
			pattern,
			sort_keys,
			group_by,
			examples,
			aliases,
			editions,
			current_edition,
			categories,
			constraints,
			primary,
			mb_format,
			mb_part_format,
		} = merge_catalog_definitions(&global, &composer);

		assert_eq!(name, "Opus number (Beethoven)");

		assert_eq!(id.as_deref(), Some("op"));
		assert_eq!(description.as_deref(), Some("shared description"));
		assert_eq!(canonical_format.as_deref(), Some("op. {number}"));
		assert_eq!(pattern.as_deref(), Some(r"^(\d+)(?:/(\d+))?$"));
		assert_eq!(sort_keys.map(|keys| keys.len()), Some(2));
		assert_eq!(group_by, Some(vec![1]));
		assert_eq!(examples.map(|e| e.len()), Some(1));
		assert_eq!(aliases, Some(vec!["opus".to_string()]));
		assert_eq!(editions.map(|e| e.len()), Some(1));
		assert_eq!(current_edition.as_deref(), Some("1"));
		assert_eq!(categories.map(|c| c.len()), Some(1));
		assert_eq!(constraints.map(|c| c.len()), Some(1));
		assert_eq!(primary, Some(true));
		assert_eq!(mb_format.as_deref(), Some("op. {number}"));
		assert_eq!(mb_part_format.as_deref(), Some("no. {part}"));
	}

	#[test]
	fn merge_prefers_composer_fields() {
		let global = fully_populated_global();
		let composer: CatalogDefinition = serde_json::from_str(r#"{
			"name": "Beethoven opus",
			"canonical_format": "Op. {number}",
			"primary": false
		}"#).unwrap();

		let merged = merge_catalog_definitions(&global, &composer);

		assert_eq!(merged.canonical_format.as_deref(), Some("Op. {number}"));
		assert_eq!(merged.primary, Some(false));
		assert_eq!(merged.description.as_deref(), Some("shared description"));
	}

	#[test]
	fn merge_accumulates_constraints_from_both_levels() {
		let global = fully_populated_global();
		let composer: CatalogDefinition = serde_json::from_str(r#"{
			"name": "Beethoven opus",
			"constraints": [{"group": 1, "name": "opus number", "min": 1, "max": 138}]
		}"#).unwrap();

		let merged = merge_catalog_definitions(&global, &composer);
		let constraints = merged.constraints.unwrap();

		assert_eq!(constraints.len(), 2);
		assert!(constraints.iter().any(|c| c.group == 2 && c.min == Some(1)));
		assert!(constraints.iter().any(|c| c.group == 1 && c.max == Some(138)));
	}

	#[test]
	fn catalog_definitions_are_cached_until_cleared() {
		let tmp = tempfile::tempdir().unwrap();
		std::fs::create_dir_all(tmp.path().join("catalogs")).unwrap();
		let path = tmp.path().join("catalogs").join("op.json");
		std::fs::write(&path, r#"{"id":"op","name":"Opus number"}"#).unwrap();

		clear_catalog_cache();
		let first = load_catalog_def(tmp.path(), "op", None).unwrap();
		assert_eq!(first.name, "Opus number");

		std::fs::write(&path, r#"{"id":"op","name":"Changed"}"#).unwrap();
		let cached = load_catalog_def(tmp.path(), "op", None).unwrap();
		assert_eq!(cached.name, "Opus number", "expected the memoized definition");

		clear_catalog_cache();
		let refreshed = load_catalog_def(tmp.path(), "op", None).unwrap();
		assert_eq!(refreshed.name, "Changed");
	}

	#[test]
	fn missing_definitions_are_cached_as_absent() {
		let tmp = tempfile::tempdir().unwrap();
		clear_catalog_cache();
		assert!(load_catalog_def(tmp.path(), "nonexistent", None).is_none());
		assert!(load_catalog_def(tmp.path(), "nonexistent", None).is_none());
	}

	#[test]
	fn test_is_fallback_key() {
		assert!(is_fallback_key(&vec![SortValue::Int(999999999), SortValue::Str("x".into())]));
		assert!(!is_fallback_key(&vec![SortValue::Int(1), SortValue::NoneFirst]));
	}
}

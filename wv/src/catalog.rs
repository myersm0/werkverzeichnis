use regex::{Regex, RegexBuilder};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;

use crate::parse::load_composer;
use crate::types::CatalogDefinition;

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

pub fn load_catalog_def<P: AsRef<Path>>(
	data_dir: P,
	scheme: &str,
	composer: Option<&str>,
) -> Option<CatalogDefinition> {
	let data_dir = data_dir.as_ref();

	let composer_def = if let Some(composer_slug) = composer {
		let composer_path = data_dir.join("composers").join(format!("{}.json", composer_slug));
		if let Ok(composer_data) = load_composer(&composer_path) {
			composer_data.catalogs.and_then(|c| c.get(scheme).cloned())
		} else {
			None
		}
	} else {
		None
	};

	let global_path = data_dir.join("catalogs").join(format!("{}.json", scheme));
	let global_def: Option<CatalogDefinition> = if global_path.exists() {
		std::fs::read_to_string(&global_path)
			.ok()
			.and_then(|content| serde_json::from_str(&content).ok())
	} else {
		None
	};

	match (composer_def, global_def) {
		(Some(mut c), Some(g)) => {
			if c.pattern.is_none() {
				c.pattern = g.pattern;
			}
			if c.sort_keys.is_none() {
				c.sort_keys = g.sort_keys;
			}
			if c.canonical_format.is_none() {
				c.canonical_format = g.canonical_format;
			}
			Some(c)
		}
		(Some(c), None) => Some(c),
		(None, Some(g)) => Some(g),
		(None, None) => None,
	}
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

		match raw {
			None => {
				key.push(SortValue::NoneFirst);
			}
			Some(s) if s.is_empty() => {
				key.push(SortValue::NoneFirst);
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

pub fn normalize_catalog_number(number: &str) -> String {
	number.to_lowercase()
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
			}]),
			group_by: None,
			aliases: None,
			editions: None,
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
				SortKey { group: 1, sort_type: "int".into(), display: None },
				SortKey { group: 2, sort_type: "int".into(), display: None },
				SortKey { group: 3, sort_type: "str".into(), display: None },
			]),
			group_by: None,
			aliases: None,
			editions: None,
		};

		let mut nums: Vec<String> = vec!["2/1", "10", "2", "2/10", "2/2"]
			.into_iter()
			.map(String::from)
			.collect();

		sort_numbers(&mut nums, Some(&defn));
		assert_eq!(nums, vec!["2", "2/1", "2/2", "2/10", "10"]);
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
	fn test_is_fallback_key() {
		assert!(is_fallback_key(&vec![SortValue::Int(999999999), SortValue::Str("x".into())]));
		assert!(!is_fallback_key(&vec![SortValue::Int(1), SortValue::NoneFirst]));
	}
}

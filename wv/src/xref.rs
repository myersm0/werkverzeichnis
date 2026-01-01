use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, Result as SqliteResult};

use crate::types::CatalogDefinition;

#[derive(Debug, Clone)]
pub struct MbLookupResult {
	pub catalog_number: String,
	pub mb_id: Option<String>,
	pub mb_title: Option<String>,
}

pub struct MbLookup {
	conn: Connection,
}

impl MbLookup {
	pub fn new<P: AsRef<Path>>(db_path: P) -> SqliteResult<Self> {
		let conn = Connection::open(db_path)?;
		Ok(Self { conn })
	}

	pub fn lookup(
		&self,
		composer: &str,
		scheme: &str,
		number: &str,
		catalog_def: Option<&CatalogDefinition>,
	) -> Option<MbLookupResult> {
		let composer_pattern = composer_to_pattern(composer);
		let (mb_number, part_filter) = format_for_mb(scheme, number, catalog_def);

		let result = if let Some(part) = part_filter {
			self.lookup_with_part(&mb_number, &composer_pattern, &part)
		} else {
			self.lookup_direct(&mb_number, &composer_pattern)
		};

		Some(MbLookupResult {
			catalog_number: number.to_string(),
			mb_id: result.as_ref().map(|(id, _)| id.clone()),
			mb_title: result.map(|(_, title)| title),
		})
	}

	fn lookup_direct(&self, catalog_number: &str, composer_pattern: &str) -> Option<(String, String)> {
		self.conn
			.query_row(
				"SELECT work_id, work_title FROM catalog_entries 
				 WHERE catalog_number = ? AND composer_name LIKE ?",
				[catalog_number, composer_pattern],
				|row| Ok((row.get(0)?, row.get(1)?)),
			)
			.ok()
	}

	fn lookup_with_part(
		&self,
		catalog_number: &str,
		composer_pattern: &str,
		part_filter: &str,
	) -> Option<(String, String)> {
		self.conn
			.query_row(
				"SELECT wp.child_id, wp.child_title 
				 FROM catalog_entries ce
				 JOIN work_parts wp ON ce.work_id = wp.parent_id
				 WHERE ce.catalog_number = ? 
				 AND ce.composer_name LIKE ?
				 AND wp.child_title LIKE ?",
				[catalog_number, composer_pattern, part_filter],
				|row| Ok((row.get(0)?, row.get(1)?)),
			)
			.ok()
	}

	pub fn lookup_batch(
		&self,
		composer: &str,
		scheme: &str,
		numbers: &[String],
		catalog_def: Option<&CatalogDefinition>,
	) -> Vec<MbLookupResult> {
		numbers
			.iter()
			.map(|n| {
				self.lookup(composer, scheme, n, catalog_def)
					.unwrap_or(MbLookupResult {
						catalog_number: n.clone(),
						mb_id: None,
						mb_title: None,
					})
			})
			.collect()
	}
}

fn composer_to_pattern(composer: &str) -> String {
	let patterns: HashMap<&str, &str> = [
		("mozart", "%Mozart%"),
		("beethoven", "%Beethoven%"),
		("bach", "%Bach%"),
		("schubert", "%Schubert%"),
		("haydn", "%Haydn%"),
		("telemann", "%Telemann%"),
		("handel", "%Handel%"),
		("brahms", "%Brahms%"),
		("chopin", "%Chopin%"),
		("liszt", "%Liszt%"),
	]
	.into_iter()
	.collect();

	patterns
		.get(composer)
		.map(|s| s.to_string())
		.unwrap_or_else(|| format!("%{}%", composer))
}

fn format_for_mb(
	scheme: &str,
	number: &str,
	catalog_def: Option<&CatalogDefinition>,
) -> (String, Option<String>) {
	if let Some(def) = catalog_def {
		if let Some(mb_format) = &def.mb_format {
			let (major, minor) = split_number(number);
			let formatted = mb_format
				.replace("{number}", number)
				.replace("{NUMBER}", &number.to_uppercase())
				.replace("{major}", &major)
				.replace("{minor}", minor.as_deref().unwrap_or_default());

			let part_filter = def.mb_part_format.as_ref().and_then(|pf| {
				minor.map(|m| pf.replace("{minor}", &m))
			});

			return (formatted, part_filter);
		}
	}

	default_format(scheme, number)
}

fn split_number(number: &str) -> (String, Option<String>) {
	if let Some((major, minor)) = number.split_once('.') {
		(major.to_string(), Some(minor.to_string()))
	} else if let Some((major, minor)) = number.split_once('/') {
		(major.to_string(), Some(minor.to_string()))
	} else {
		(number.to_string(), None)
	}
}

fn default_format(scheme: &str, number: &str) -> (String, Option<String>) {
	let (major, minor) = split_number(number);

	match scheme {
		"op" => {
			let formatted = format!("op. {}", major);
			let part = minor.map(|m| format!("%no. {}%", m));
			(formatted, part)
		}
		"k" => (number.to_string(), None),
		"bwv" => (format!("BWV {}", number.to_uppercase()), None),
		"d" => (format!("D. {}", number), None),
		"hob" => (format!("Hob. {}", number.to_uppercase()), None),
		"twv" => (format!("TWV {}", number.to_uppercase()), None),
		_ => (number.to_string(), None),
	}
}

#[derive(Debug, Clone)]
pub struct XrefStats {
	pub matched: usize,
	pub not_found: usize,
	pub duplicates: Vec<(String, Vec<String>)>,
}

pub fn check_duplicates(results: &[MbLookupResult]) -> Vec<(String, Vec<String>)> {
	let mut by_mbid: HashMap<String, Vec<String>> = HashMap::new();

	for r in results {
		if let Some(mb_id) = &r.mb_id {
			by_mbid
				.entry(mb_id.clone())
				.or_default()
				.push(r.catalog_number.clone());
		}
	}

	by_mbid
		.into_iter()
		.filter(|(_, nums)| nums.len() > 1)
		.collect()
}

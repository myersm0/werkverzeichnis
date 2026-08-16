use std::fs;
use std::path::Path;
use thiserror::Error;

use crate::types::{Collection, Composer, Composition};

#[derive(Error, Debug)]
pub enum ParseError {
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
	#[error("JSON error: {0}")]
	Json(#[from] serde_json::Error),
	#[error("Invalid composition path: {0}")]
	InvalidPath(String),
}

pub fn load_composition<P: AsRef<Path>>(path: P) -> Result<Composition, ParseError> {
	let content = fs::read_to_string(path)?;
	let composition: Composition = serde_json::from_str(&content)?;
	Ok(composition)
}

pub fn load_collection<P: AsRef<Path>>(path: P) -> Result<Collection, ParseError> {
	let content = fs::read_to_string(path)?;
	let collection: Collection = serde_json::from_str(&content)?;
	Ok(collection)
}

pub fn load_composer<P: AsRef<Path>>(path: P) -> Result<Composer, ParseError> {
	let content = fs::read_to_string(path)?;
	let composer: Composer = serde_json::from_str(&content)?;
	Ok(composer)
}

pub fn extract_id_from_path<P: AsRef<Path>>(path: P) -> Result<String, ParseError> {
	let path = path.as_ref();
	let invalid = || ParseError::InvalidPath(path.display().to_string());

	let file_stem = path.file_stem().and_then(|s| s.to_str()).ok_or_else(invalid)?;

	let suffix = match file_stem.rsplit_once('-') {
		Some((_, suffix)) => suffix,
		None => match file_stem.rsplit_once('_') {
			Some((_, suffix)) => suffix,
			None => file_stem,
		},
	};

	let prefix = path
		.parent()
		.and_then(|p| p.file_name())
		.and_then(|s| s.to_str())
		.ok_or_else(invalid)?;

	if prefix.len() != 2 || suffix.len() != 6 {
		return Err(invalid());
	}

	Ok(format!("{}{}", prefix, suffix))
}

pub fn path_for_id<P: AsRef<Path>>(base_dir: P, id: &str) -> Result<std::path::PathBuf, ParseError> {
	if id.len() != 8 || !id.is_ascii() {
		return Err(ParseError::InvalidPath(format!(
			"ID must be 8 ASCII characters: {}",
			id
		)));
	}
	let prefix = &id[..2];
	let suffix = &id[2..];
	Ok(base_dir.as_ref().join(prefix).join(format!("{}.json", suffix)))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_extract_id_from_path() {
		let path = Path::new("compositions/ab/cd1234.json");
		let id = extract_id_from_path(path).unwrap();
		assert_eq!(id, "abcd1234");
	}

	#[test]
	fn test_path_for_id() {
		let path = path_for_id("compositions", "abcd1234").unwrap();
		assert_eq!(path, Path::new("compositions/ab/cd1234.json"));
	}

	#[test]
	fn path_for_id_rejects_non_ascii_ids_without_panicking() {
		let id = "a\u{e9}aaaaa";
		assert_eq!(id.len(), 8);
		assert!(path_for_id("compositions", id).is_err());
	}

	#[test]
	fn path_for_id_rejects_short_ids() {
		assert!(path_for_id("compositions", "a").is_err());
		assert!(path_for_id("compositions", "").is_err());
	}

	#[test]
	fn extract_id_accepts_prefixed_filenames() {
		assert_eq!(
			extract_id_from_path(Path::new("compositions/ab/foo-bar-cd1234.json")).unwrap(),
			"abcd1234"
		);
		assert_eq!(
			extract_id_from_path(Path::new("compositions/ab/foo_cd1234.json")).unwrap(),
			"abcd1234"
		);
	}

	#[test]
	fn extract_id_rejects_wrong_component_lengths() {
		assert!(extract_id_from_path(Path::new("compositions/abc/cd1234.json")).is_err());
		assert!(extract_id_from_path(Path::new("compositions/ab/cd12345.json")).is_err());
	}

	#[test]
	fn test_composition_note_is_accepted() {
		let json = r#"{
			"id": "abcd1234",
			"form": "sonata",
			"note": "unfinished",
			"attribution": [{"composer": "schubert"}]
		}"#;
		let composition: Composition = serde_json::from_str(json).unwrap();
		assert_eq!(composition.note.as_deref(), Some("unfinished"));
	}

	#[test]
	fn test_composer_catalog_metadata_is_preserved() {
		let json = r#"{
			"id": "mozart",
			"name": {"full": "Wolfgang Amadeus Mozart", "sort": "Mozart, Wolfgang Amadeus"},
			"default_scheme": "k",
			"catalogs": {
				"k": {
					"name": "Köchel-Verzeichnis",
					"primary": true,
					"examples": [{"number": "331", "display": "K. 331"}],
					"categories": {"anh": "Appendix"},
					"editions": {"9": {"year": 2024, "editor": "Example"}}
				}
			}
		}"#;
		let composer: Composer = serde_json::from_str(json).unwrap();
		let catalog = composer.catalogs.as_ref().unwrap().get("k").unwrap();
		assert_eq!(composer.default_scheme.as_deref(), Some("k"));
		assert_eq!(catalog.primary, Some(true));
		assert_eq!(catalog.examples.as_ref().unwrap()[0].number, "331");
		assert_eq!(catalog.categories.as_ref().unwrap().get("anh").map(String::as_str), Some("Appendix"));
		assert_eq!(catalog.editions.as_ref().unwrap().get("9").unwrap().year, 2024);
	}

	#[test]
	fn test_unknown_composer_catalog_field_is_rejected() {
		let json = r#"{
			"id": "bach",
			"name": {"full": "Johann Sebastian Bach", "sort": "Bach, Johann Sebastian"},
			"catalogs": {"bwv": {"name": "Bach-Werke-Verzeichnis", "primray": true}}
		}"#;
		let error = serde_json::from_str::<Composer>(json).unwrap_err();
		assert!(error.to_string().contains("unknown field `primray`"));
	}

	#[test]
	fn test_unknown_composition_field_is_rejected() {
		let json = r#"{
			"id": "abcd1234",
			"form": "sonata",
			"notes": "unfinished",
			"attribution": [{"composer": "schubert"}]
		}"#;
		let error = serde_json::from_str::<Composition>(json).unwrap_err();
		assert!(error.to_string().contains("unknown field `notes`"));
	}
}

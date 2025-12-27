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
	let file_stem = path
		.file_stem()
		.and_then(|s| s.to_str())
		.ok_or_else(|| ParseError::InvalidPath(path.display().to_string()))?;

	let parent = path
		.parent()
		.and_then(|p| p.file_name())
		.and_then(|s| s.to_str())
		.ok_or_else(|| ParseError::InvalidPath(path.display().to_string()))?;

	Ok(format!("{}{}", parent, file_stem))
}

pub fn path_for_id<P: AsRef<Path>>(base_dir: P, id: &str) -> Result<std::path::PathBuf, ParseError> {
	if id.len() != 8 {
		return Err(ParseError::InvalidPath(format!(
			"ID must be 8 characters: {}",
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
}

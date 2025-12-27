use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::parse::load_composer;
use crate::types::Composition;

#[derive(Debug, Clone)]
pub struct ValidationError {
	pub path: String,
	pub message: String,
}

impl std::fmt::Display for ValidationError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}: {}", self.path, self.message)
	}
}

pub struct Validator {
	composers: HashSet<String>,
	catalog_schemes: HashSet<String>,
}

impl Validator {
	pub fn new<P: AsRef<Path>>(data_dir: P) -> Self {
		let data_dir = data_dir.as_ref();
		let mut composers = HashSet::new();
		let mut catalog_schemes = HashSet::new();

		let composers_dir = data_dir.join("composers");
		if let Ok(entries) = fs::read_dir(&composers_dir) {
			for entry in entries.flatten() {
				let path = entry.path();
				if path.extension().map_or(false, |e| e == "json") {
					if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
						composers.insert(stem.to_string());

						if let Ok(composer) = load_composer(&path) {
							if let Some(catalogs) = composer.catalogs {
								for scheme in catalogs.keys() {
									catalog_schemes.insert(scheme.clone());
								}
							}
						}
					}
				}
			}
		}

		let catalogs_dir = data_dir.join("catalogs");
		if let Ok(entries) = fs::read_dir(&catalogs_dir) {
			for entry in entries.flatten() {
				let path = entry.path();
				if path.extension().map_or(false, |e| e == "json") {
					if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
						catalog_schemes.insert(stem.to_string());
					}
				}
			}
		}

		Self {
			composers,
			catalog_schemes,
		}
	}

	pub fn validate_file<P: AsRef<Path>>(&self, path: P) -> Vec<ValidationError> {
		let path = path.as_ref();
		let path_str = path.display().to_string();
		let mut errors = Vec::new();

		let content = match fs::read_to_string(path) {
			Ok(c) => c,
			Err(e) => {
				errors.push(ValidationError {
					path: path_str,
					message: format!("Failed to read file: {}", e),
				});
				return errors;
			}
		};

		if let Err(e) = std::str::from_utf8(content.as_bytes()) {
			errors.push(ValidationError {
				path: path_str.clone(),
				message: format!("Invalid UTF-8: {}", e),
			});
		}

		if content.contains("  ") {
			errors.push(ValidationError {
				path: path_str.clone(),
				message: "Contains multiple consecutive spaces".into(),
			});
		}

		let comp: Composition = match serde_json::from_str(&content) {
			Ok(c) => c,
			Err(e) => {
				errors.push(ValidationError {
					path: path_str,
					message: format!("Invalid JSON: {}", e),
				});
				return errors;
			}
		};

		errors.extend(self.validate_id(&comp.id, path, &path_str));
		errors.extend(self.validate_key(&comp.key, &path_str));
		errors.extend(self.validate_attribution(&comp, &path_str));

		errors
	}

	fn validate_id(&self, id: &str, path: &Path, path_str: &str) -> Vec<ValidationError> {
		let mut errors = Vec::new();

		let hex_pattern = regex::Regex::new(r"^[a-f0-9]{8}$").unwrap();
		if !hex_pattern.is_match(id) {
			errors.push(ValidationError {
				path: path_str.to_string(),
				message: format!("ID '{}' is not 8 lowercase hex characters", id),
			});
			return errors;
		}

		let expected_id = extract_id_from_path(path);
		if let Some(expected) = expected_id {
			if expected != id {
				errors.push(ValidationError {
					path: path_str.to_string(),
					message: format!("ID '{}' doesn't match path (expected '{}')", id, expected),
				});
			}
		}

		errors
	}

	fn validate_key(&self, key: &Option<String>, path_str: &str) -> Vec<ValidationError> {
		let mut errors = Vec::new();

		if let Some(k) = key {
			let key_pattern =
				regex::Regex::new(r"^[A-Ga-g][#b]?(\.(dor|phr|lyd|mix|loc))?$").unwrap();
			if !key_pattern.is_match(k) {
				errors.push(ValidationError {
					path: path_str.to_string(),
					message: format!("Invalid key format: '{}'", k),
				});
			}
		}

		errors
	}

	fn validate_attribution(&self, comp: &Composition, path_str: &str) -> Vec<ValidationError> {
		let mut errors = Vec::new();

		if comp.attribution.is_empty() {
			errors.push(ValidationError {
				path: path_str.to_string(),
				message: "Attribution array is empty".into(),
			});
			return errors;
		}

		for (i, entry) in comp.attribution.iter().enumerate() {
			if let Some(composer) = &entry.composer {
				if !self.composers.contains(composer) {
					errors.push(ValidationError {
						path: path_str.to_string(),
						message: format!(
							"attribution[{}]: composer '{}' not found in composers/",
							i, composer
						),
					});
				}
			}

			if let Some(catalog) = &entry.catalog {
				for cat in catalog {
					if !self.catalog_schemes.contains(&cat.scheme) {
						errors.push(ValidationError {
							path: path_str.to_string(),
							message: format!(
								"attribution[{}]: catalog scheme '{}' not defined",
								i, cat.scheme
							),
						});
					}
				}
			}
		}

		errors
	}

	pub fn validate_all<P: AsRef<Path>>(&self, compositions_dir: P) -> Vec<ValidationError> {
		let compositions_dir = compositions_dir.as_ref();
		let mut errors = Vec::new();

		let entries = match fs::read_dir(compositions_dir) {
			Ok(e) => e,
			Err(_) => return errors,
		};

		for prefix_entry in entries.flatten() {
			if !prefix_entry.path().is_dir() {
				continue;
			}

			let sub_entries = match fs::read_dir(prefix_entry.path()) {
				Ok(e) => e,
				Err(_) => continue,
			};

			for file_entry in sub_entries.flatten() {
				let path = file_entry.path();
				if path.extension().map_or(true, |e| e != "json") {
					continue;
				}

				errors.extend(self.validate_file(&path));
			}
		}

		errors
	}
}

fn extract_id_from_path(path: &Path) -> Option<String> {
	let file_stem = path.file_stem()?.to_str()?;

	let id_part = if let Some(pos) = file_stem.rfind('-') {
		&file_stem[pos + 1..]
	} else if let Some(pos) = file_stem.rfind('_') {
		&file_stem[pos + 1..]
	} else {
		file_stem
	};

	let parent = path.parent()?.file_name()?.to_str()?;

	if parent.len() == 2 && id_part.len() == 6 {
		Some(format!("{}{}", parent, id_part))
	} else {
		None
	}
}

pub fn validate_file<P: AsRef<Path>>(path: P, data_dir: &Path) -> Vec<ValidationError> {
	let validator = Validator::new(data_dir);
	validator.validate_file(path)
}

pub fn validate_all<P: AsRef<Path>>(data_dir: P) -> Vec<ValidationError> {
	let data_dir = data_dir.as_ref();
	let validator = Validator::new(data_dir);
	validator.validate_all(data_dir.join("compositions"))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_extract_id_from_path() {
		let path = Path::new("compositions/ab/cd1234.json");
		assert_eq!(extract_id_from_path(path), Some("abcd1234".into()));

		let path = Path::new("compositions/ab/foo_bar_cd1234.json");
		assert_eq!(extract_id_from_path(path), Some("abcd1234".into()));

		let path = Path::new("compositions/ab/foo-bar-cd1234.json");
		assert_eq!(extract_id_from_path(path), Some("abcd1234".into()));
	}

	#[test]
	fn test_id_validation() {
		let validator = Validator {
			composers: HashSet::new(),
			catalog_schemes: HashSet::new(),
		};

		// Valid: matches path
		let path = Path::new("compositions/ab/cd1234.json");
		let errors = validator.validate_id("abcd1234", path, "test");
		assert!(errors.is_empty());

		// Invalid: uppercase hex
		let errors = validator.validate_id("ABCD1234", path, "test");
		assert!(!errors.is_empty());

		// Invalid: too short
		let errors = validator.validate_id("abc1234", path, "test");
		assert!(!errors.is_empty());

		// Invalid: contains non-hex chars
		let path = Path::new("compositions/wx/yz5678.json");
		let errors = validator.validate_id("wxyz5678", path, "test");
		assert!(!errors.is_empty());

		// Valid: all digits (valid hex), matches path
		let path = Path::new("compositions/12/345678.json");
		let errors = validator.validate_id("12345678", path, "test");
		assert!(errors.is_empty());
	}
}

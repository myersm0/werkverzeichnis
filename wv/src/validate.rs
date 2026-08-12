use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use jsonschema::Validator as JsonSchemaValidator;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::types::{AttributionEntry, CatalogDefinition, Collection, Composer, Composition};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataKind {
	Composition,
	Composer,
	Catalog,
	Collection,
}

struct SchemaCheck {
	path: PathBuf,
	validator: Option<JsonSchemaValidator>,
	error: Option<String>,
}

impl SchemaCheck {
	fn load(path: PathBuf) -> Self {
		match load_schema_validator(&path) {
			Ok(validator) => Self {
				path,
				validator: Some(validator),
				error: None,
			},
			Err(error) => Self {
				path,
				validator: None,
				error: Some(error),
			},
		}
	}

	fn schema_error(&self) -> Option<ValidationError> {
		self.error.as_ref().map(|message| ValidationError {
			path: self.path.display().to_string(),
			message: message.clone(),
		})
	}

	fn validate(&self, value: &Value, path_str: &str) -> Vec<ValidationError> {
		let Some(validator) = &self.validator else {
			return self.schema_error().into_iter().collect();
		};

		validator
			.iter_errors(value)
			.map(|error| {
				let instance_path = error.instance_path().to_string();
				let location = if instance_path.is_empty() {
					"$".to_string()
				} else {
					format!("${}", instance_path)
				};
				ValidationError {
					path: path_str.to_string(),
					message: format!("schema {}: {}", location, error),
				}
			})
			.collect()
	}
}

pub struct Validator {
	composers: HashSet<String>,
	catalog_schemes: HashSet<String>,
	global_catalog_schemes: HashSet<String>,
	composition_schema: SchemaCheck,
	composer_schema: SchemaCheck,
	catalog_schema: SchemaCheck,
	collection_schema: SchemaCheck,
}

impl Validator {
	pub fn new<P: AsRef<Path>>(data_dir: P) -> Self {
		let data_dir = data_dir.as_ref();
		let mut composers = HashSet::new();
		let mut catalog_schemes = HashSet::new();
		let mut global_catalog_schemes = HashSet::new();

		let composers_dir = data_dir.join("composers");
		if let Ok(entries) = fs::read_dir(&composers_dir) {
			for entry in entries.flatten() {
				let path = entry.path();
				if path.extension().map_or(false, |e| e == "json") {
					if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
						composers.insert(stem.to_string());
					}

					if let Ok(content) = fs::read_to_string(&path) {
						if let Ok(value) = serde_json::from_str::<Value>(&content) {
							if let Some(catalogs) = value.get("catalogs").and_then(Value::as_object) {
								catalog_schemes.extend(catalogs.keys().cloned());
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
						global_catalog_schemes.insert(stem.to_string());
					}
				}
			}
		}

		let schemas_dir = data_dir.join("schemas");

		Self {
			composers,
			catalog_schemes,
			global_catalog_schemes,
			composition_schema: SchemaCheck::load(schemas_dir.join("composition.schema.json")),
			composer_schema: SchemaCheck::load(schemas_dir.join("composer.schema.json")),
			catalog_schema: SchemaCheck::load(schemas_dir.join("catalog.schema.json")),
			collection_schema: SchemaCheck::load(schemas_dir.join("collection.schema.json")),
		}
	}

	pub fn validate_file<P: AsRef<Path>>(&self, path: P) -> Vec<ValidationError> {
		let path = path.as_ref();
		match data_kind(path) {
			Some(DataKind::Composition) => self.validate_composition_file(path),
			Some(DataKind::Composer) => self.validate_composer_file(path),
			Some(DataKind::Catalog) => self.validate_catalog_file(path),
			Some(DataKind::Collection) => self.validate_collection_file(path),
			None => vec![ValidationError {
				path: path.display().to_string(),
				message: "Cannot determine data type; path must be under compositions/, composers/, catalogs/, or collections/".into(),
			}],
		}
	}

	fn validate_composition_file(&self, path: &Path) -> Vec<ValidationError> {
		let (value, mut errors) = match self.read_and_validate(path, &self.composition_schema, true) {
			Ok(result) => result,
			Err(errors) => return errors,
		};
		let path_str = path.display().to_string();
		let Some(comp) = deserialize_model::<Composition>(&value, &path_str, &mut errors) else {
			return errors;
		};

		errors.extend(self.validate_id(&comp.id, path, &path_str));
		errors.extend(self.validate_key(&comp.key, &path_str));
		errors.extend(self.validate_attribution(&comp.attribution, &path_str, true));
		errors
	}

	fn validate_composer_file(&self, path: &Path) -> Vec<ValidationError> {
		let (value, mut errors) = match self.read_and_validate(path, &self.composer_schema, false) {
			Ok(result) => result,
			Err(errors) => return errors,
		};
		let path_str = path.display().to_string();
		let Some(composer) = deserialize_model::<Composer>(&value, &path_str, &mut errors) else {
			return errors;
		};

		if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
			if composer.id != stem {
				errors.push(ValidationError {
					path: path_str.clone(),
					message: format!("Composer ID '{}' doesn't match filename '{}'", composer.id, stem),
				});
			}
		}

		if let Some(default_scheme) = &composer.default_scheme {
			let defined_locally = composer
				.catalogs
				.as_ref()
				.map_or(false, |catalogs| catalogs.contains_key(default_scheme));
			if !defined_locally && !self.global_catalog_schemes.contains(default_scheme) {
				errors.push(ValidationError {
					path: path_str,
					message: format!("default_scheme '{}' is not defined for this composer or globally", default_scheme),
				});
			}
		}

		errors
	}

	fn validate_catalog_file(&self, path: &Path) -> Vec<ValidationError> {
		let (value, mut errors) = match self.read_and_validate(path, &self.catalog_schema, false) {
			Ok(result) => result,
			Err(errors) => return errors,
		};
		let path_str = path.display().to_string();
		let Some(catalog) = deserialize_model::<CatalogDefinition>(&value, &path_str, &mut errors) else {
			return errors;
		};

		if let (Some(id), Some(stem)) = (
			catalog.id.as_deref(),
			path.file_stem().and_then(|s| s.to_str()),
		) {
			if id != stem {
				errors.push(ValidationError {
					path: path_str,
					message: format!("Catalog ID '{}' doesn't match filename '{}'", id, stem),
				});
			}
		}

		errors
	}

	fn validate_collection_file(&self, path: &Path) -> Vec<ValidationError> {
		let (value, mut errors) = match self.read_and_validate(path, &self.collection_schema, false) {
			Ok(result) => result,
			Err(errors) => return errors,
		};
		let path_str = path.display().to_string();
		let Some(collection) = deserialize_model::<Collection>(&value, &path_str, &mut errors) else {
			return errors;
		};

		if let Some(expected) = collection_id_from_path(path) {
			if collection.id != expected {
				errors.push(ValidationError {
					path: path_str.clone(),
					message: format!("Collection ID '{}' doesn't match path (expected '{}')", collection.id, expected),
				});
			}
		}

		if let Some(composer) = &collection.composer {
			if !self.composers.contains(composer) {
				errors.push(ValidationError {
					path: path_str.clone(),
					message: format!("composer '{}' not found in composers/", composer),
				});
			}
		}

		if !self.catalog_schemes.contains(&collection.scheme) {
			errors.push(ValidationError {
				path: path_str.clone(),
				message: format!("catalog scheme '{}' not defined", collection.scheme),
			});
		}

		errors.extend(self.validate_attribution(&collection.attribution, &path_str, false));
		errors
	}

	fn read_and_validate(
		&self,
		path: &Path,
		schema: &SchemaCheck,
		check_spacing: bool,
	) -> Result<(Value, Vec<ValidationError>), Vec<ValidationError>> {
		let path_str = path.display().to_string();
		let content = match fs::read_to_string(path) {
			Ok(content) => content,
			Err(error) => {
				return Err(vec![ValidationError {
					path: path_str,
					message: format!("Failed to read file: {}", error),
				}]);
			}
		};
		let mut errors = Vec::new();

		if check_spacing && content.contains("  ") {
			errors.push(ValidationError {
				path: path_str.clone(),
				message: "Contains multiple consecutive spaces".into(),
			});
		}

		let value: Value = match serde_json::from_str(&content) {
			Ok(value) => value,
			Err(error) => {
				errors.push(ValidationError {
					path: path_str,
					message: format!("Invalid JSON: {}", error),
				});
				return Err(errors);
			}
		};

		let schema_errors = schema.validate(&value, &path_str);
		if !schema_errors.is_empty() {
			errors.extend(schema_errors);
			return Err(errors);
		}

		Ok((value, errors))
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

	fn validate_attribution(
		&self,
		attribution: &[AttributionEntry],
		path_str: &str,
		require_nonempty: bool,
	) -> Vec<ValidationError> {
		let mut errors = Vec::new();

		if require_nonempty && attribution.is_empty() {
			errors.push(ValidationError {
				path: path_str.to_string(),
				message: "Attribution array is empty".into(),
			});
			return errors;
		}

		for (i, entry) in attribution.iter().enumerate() {
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

					if cat.scheme != cat.scheme.to_lowercase() {
						errors.push(ValidationError {
							path: path_str.to_string(),
							message: format!(
								"attribution[{}]: catalog scheme '{}' must be lowercase",
								i, cat.scheme
							),
						});
					}

					if !is_valid_catalog_number_case(&cat.scheme, &cat.number) {
						errors.push(ValidationError {
							path: path_str.to_string(),
							message: format!(
								"attribution[{}]: catalog number '{}' must be lowercase{}",
								i,
								cat.number,
								if cat.scheme == "bwv" { " (R suffix allowed)" } else { "" }
							),
						});
					}
				}
			}
		}

		errors
	}

	pub fn validate_all<P: AsRef<Path>>(&self, data_dir: P) -> Vec<ValidationError> {
		let data_dir = data_dir.as_ref();
		let schema_errors: Vec<_> = [
			&self.composition_schema,
			&self.composer_schema,
			&self.catalog_schema,
			&self.collection_schema,
		]
		.into_iter()
		.filter_map(|schema| schema.schema_error())
		.collect();
		if !schema_errors.is_empty() {
			return schema_errors;
		}

		let mut paths = Vec::new();
		for directory in ["composers", "catalogs", "collections", "compositions"] {
			collect_json_files(&data_dir.join(directory), &mut paths);
		}
		paths.sort();

		let mut errors = Vec::new();
		for path in paths {
			errors.extend(self.validate_file(path));
		}
		errors
	}
}

fn deserialize_model<T: DeserializeOwned>(
	value: &Value,
	path_str: &str,
	errors: &mut Vec<ValidationError>,
) -> Option<T> {
	match serde_json::from_value(value.clone()) {
		Ok(model) => Some(model),
		Err(error) => {
			errors.push(ValidationError {
				path: path_str.to_string(),
				message: format!("Schema/model mismatch: {}", error),
			});
			None
		}
	}
}

fn load_schema_validator(path: &Path) -> Result<JsonSchemaValidator, String> {
	let content = fs::read_to_string(path)
		.map_err(|error| format!("Failed to read schema: {}", error))?;
	let schema: Value = serde_json::from_str(&content)
		.map_err(|error| format!("Invalid schema JSON: {}", error))?;
	jsonschema::meta::validate(&schema)
		.map_err(|error| format!("Invalid JSON Schema: {}", error))?;
	jsonschema::validator_for(&schema)
		.map_err(|error| format!("Failed to compile JSON Schema: {}", error))
}

fn data_kind(path: &Path) -> Option<DataKind> {
	for ancestor in path.ancestors() {
		match ancestor.file_name().and_then(|name| name.to_str()) {
			Some("compositions") => return Some(DataKind::Composition),
			Some("composers") => return Some(DataKind::Composer),
			Some("catalogs") => return Some(DataKind::Catalog),
			Some("collections") => return Some(DataKind::Collection),
			_ => {}
		}
	}
	None
}

fn collect_json_files(dir: &Path, paths: &mut Vec<PathBuf>) {
	let Ok(entries) = fs::read_dir(dir) else {
		return;
	};

	for entry in entries.flatten() {
		let path = entry.path();
		if path.is_dir() {
			collect_json_files(&path, paths);
		} else if path.extension().map_or(false, |ext| ext == "json") {
			paths.push(path);
		}
	}
}

fn is_valid_catalog_number_case(scheme: &str, number: &str) -> bool {
	if scheme == "bwv" {
		if number.ends_with('R') {
			let prefix = &number[..number.len() - 1];
			return prefix == prefix.to_lowercase();
		}
	}
	number == number.to_lowercase()
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

fn collection_id_from_path(path: &Path) -> Option<String> {
	let stem = path.file_stem()?.to_str()?;
	let parent = path.parent()?.file_name()?.to_str()?;
	let collections = path.parent()?.parent()?.file_name()?.to_str()?;
	if collections == "collections" {
		Some(format!("{}-{}", parent, stem))
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
	validator.validate_all(data_dir)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn empty_schema() -> SchemaCheck {
		SchemaCheck {
			path: PathBuf::new(),
			validator: None,
			error: None,
		}
	}

	fn test_validator() -> Validator {
		Validator {
			composers: HashSet::new(),
			catalog_schemes: HashSet::new(),
			global_catalog_schemes: HashSet::new(),
			composition_schema: empty_schema(),
			composer_schema: empty_schema(),
			catalog_schema: empty_schema(),
			collection_schema: empty_schema(),
		}
	}

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
	fn test_collection_id_from_path() {
		let path = Path::new("collections/bach/french-suites.json");
		assert_eq!(collection_id_from_path(path), Some("bach-french-suites".into()));
	}

	#[test]
	fn test_data_kind() {
		assert_eq!(data_kind(Path::new("compositions/ab/cd1234.json")), Some(DataKind::Composition));
		assert_eq!(data_kind(Path::new("composers/bach.json")), Some(DataKind::Composer));
		assert_eq!(data_kind(Path::new("catalogs/op.json")), Some(DataKind::Catalog));
		assert_eq!(data_kind(Path::new("collections/bach/wtc-1.json")), Some(DataKind::Collection));
	}

	#[test]
	fn test_id_validation() {
		let validator = test_validator();

		let path = Path::new("compositions/ab/cd1234.json");
		let errors = validator.validate_id("abcd1234", path, "test");
		assert!(errors.is_empty());

		let errors = validator.validate_id("ABCD1234", path, "test");
		assert!(!errors.is_empty());

		let errors = validator.validate_id("abc1234", path, "test");
		assert!(!errors.is_empty());

		let path = Path::new("compositions/wx/yz5678.json");
		let errors = validator.validate_id("wxyz5678", path, "test");
		assert!(!errors.is_empty());

		let path = Path::new("compositions/12/345678.json");
		let errors = validator.validate_id("12345678", path, "test");
		assert!(errors.is_empty());
	}
}

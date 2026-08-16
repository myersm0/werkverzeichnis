use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use jsonschema::Validator as JsonSchemaValidator;
use regex::Regex;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::OnceLock;

use crate::catalog::{
	cached_regex, load_catalog_def, normalize_catalog_number, validate_catalog_domain,
};
use crate::inventory::{build_inventory_index, normalize_inventory, InventoryIndex};
use crate::parse::extract_id_from_path;
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
	Inventory,
}

struct SchemaCheck {
	path: PathBuf,
	validator: Option<JsonSchemaValidator>,
	error: Option<String>,
}

#[derive(Debug, Clone)]
enum CachedComposition {
	Parsed {
		value: Value,
		has_multiple_spaces: bool,
	},
	ReadError(String),
	InvalidJson {
		message: String,
		has_multiple_spaces: bool,
	},
}

impl CachedComposition {
	fn load(path: &Path) -> Self {
		let content = match fs::read_to_string(path) {
			Ok(content) => content,
			Err(error) => return Self::ReadError(error.to_string()),
		};
		let has_multiple_spaces = content.contains("  ");

		match serde_json::from_str(&content) {
			Ok(value) => Self::Parsed {
				value,
				has_multiple_spaces,
			},
			Err(error) => Self::InvalidJson {
				message: error.to_string(),
				has_multiple_spaces,
			},
		}
	}
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
	data_dir: PathBuf,
	composers: HashSet<String>,
	catalog_schemes: HashSet<String>,
	global_catalog_schemes: HashSet<String>,
	composer_catalog_schemes: HashMap<String, HashSet<String>>,
	current_catalog_targets: HashMap<(String, String, String), Vec<String>>,
	composition_cache: HashMap<PathBuf, CachedComposition>,
	composition_schema: SchemaCheck,
	composer_schema: SchemaCheck,
	catalog_schema: SchemaCheck,
	collection_schema: SchemaCheck,
	inventory_index: InventoryIndex,
}

impl Validator {
	pub fn new<P: AsRef<Path>>(data_dir: P) -> Self {
		let data_dir = data_dir.as_ref();
		let mut composers = HashSet::new();
		let mut catalog_schemes = HashSet::new();
		let mut global_catalog_schemes = HashSet::new();
		let mut composer_catalog_schemes = HashMap::new();

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
								let schemes: HashSet<String> = catalogs.keys().cloned().collect();
								catalog_schemes.extend(schemes.iter().cloned());
								if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
									composer_catalog_schemes.insert(stem.to_string(), schemes);
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
						global_catalog_schemes.insert(stem.to_string());
					}
				}
			}
		}

		let mut current_catalog_targets = HashMap::new();
		let mut composition_cache = HashMap::new();
		let mut composition_paths = Vec::new();
		collect_json_files(&data_dir.join("compositions"), &mut composition_paths);
		for path in composition_paths {
			let cached = CachedComposition::load(&path);
			if let CachedComposition::Parsed { value, .. } = &cached {
				if let Ok(composition) = serde_json::from_value::<Composition>(value.clone()) {
					let mut schemes_seen = HashSet::new();
					for attribution in &composition.attribution {
						let (Some(composer), Some(catalog)) = (&attribution.composer, &attribution.catalog) else {
							continue;
						};
						for entry in catalog {
							if schemes_seen.insert((composer.clone(), entry.scheme.clone())) {
								current_catalog_targets
									.entry((composer.clone(), entry.scheme.clone(), entry.number.clone()))
									.or_insert_with(Vec::new)
									.push(composition.id.clone());
							}
						}
					}
				}
			}
			composition_cache.insert(path, cached);
		}

		let schemas_dir = data_dir.join("schemas");

		Self {
			data_dir: data_dir.to_path_buf(),
			composers,
			catalog_schemes,
			global_catalog_schemes,
			composer_catalog_schemes,
			current_catalog_targets,
			composition_cache,
			composition_schema: SchemaCheck::load(schemas_dir.join("composition.schema.json")),
			composer_schema: SchemaCheck::load(schemas_dir.join("composer.schema.json")),
			catalog_schema: SchemaCheck::load(schemas_dir.join("catalog.schema.json")),
			collection_schema: SchemaCheck::load(schemas_dir.join("collection.schema.json")),
			inventory_index: build_inventory_index(data_dir),
		}
	}

	pub fn validate_file<P: AsRef<Path>>(&self, path: P) -> Vec<ValidationError> {
		let path = path.as_ref();
		match data_kind(path) {
			Some(DataKind::Composition) => self.validate_composition_file(path),
			Some(DataKind::Composer) => self.validate_composer_file(path),
			Some(DataKind::Catalog) => self.validate_catalog_file(path),
			Some(DataKind::Collection) => self.validate_collection_file(path),
			Some(DataKind::Inventory) => self.validate_inventory_file(path),
			None => vec![ValidationError {
				path: path.display().to_string(),
				message: "Cannot determine data type; path must be under compositions/, composers/, catalogs/, collections/, or inventories/".into(),
			}],
		}
	}

	fn catalog_is_allowed_for_composer(&self, composer: &str, scheme: &str) -> bool {
		self.global_catalog_schemes.contains(scheme)
			|| self
				.composer_catalog_schemes
				.get(composer)
				.map_or(false, |schemes| schemes.contains(scheme))
	}

	fn validate_catalog_reference(
		&self,
		composer: &str,
		scheme: &str,
		number: &str,
		edition: Option<&str>,
		path_str: &str,
		location: &str,
	) -> Vec<ValidationError> {
		let mut errors = Vec::new();
		let Some(definition) = load_catalog_def(&self.data_dir, scheme, Some(composer)) else {
			return errors;
		};

		let mut pattern_valid = true;
		if let Some(pattern) = &definition.pattern {
			match cached_regex(pattern) {
				Some(regex) if !regex.is_match(number) => {
					pattern_valid = false;
					errors.push(ValidationError {
						path: path_str.to_string(),
						message: format!(
							"{}: catalog number '{}' does not match '{}' pattern",
							location, number, scheme
						),
					});
				}
				None => {
					pattern_valid = false;
					errors.push(ValidationError {
						path: path_str.to_string(),
						message: format!("{}: invalid '{}' catalog pattern", location, scheme),
					});
				}
				_ => {}
			}
		}

		if let Some(edition) = edition {
			let defined = definition
				.editions
				.as_ref()
				.map_or(false, |editions| editions.contains_key(edition));
			if !defined {
				errors.push(ValidationError {
					path: path_str.to_string(),
					message: format!(
						"{}: edition '{}' is not defined for {}:{}",
						location, edition, composer, scheme
					),
				});
			}
		}

		let mut domain_valid = pattern_valid;
		if pattern_valid {
			if let Err(error) = validate_catalog_domain(number, &definition) {
				domain_valid = false;
				errors.push(ValidationError {
					path: path_str.to_string(),
					message: format!(
						"{}: catalog number '{}' is outside the structural domain for '{}': {}",
						location, number, scheme, error
					),
				});
			}
		}

		if domain_valid {
			if let Some(catalog) = self
				.inventory_index
				.catalog(composer, scheme, edition, Some(&definition))
			{
				let normalized = normalize_catalog_number(number);
				if catalog.complete && !catalog.entries.contains(&normalized) {
					errors.push(ValidationError {
						path: path_str.to_string(),
						message: format!(
							"{}: {}:{}:{} is not present in the complete catalog inventory",
							location, composer, scheme, number
						),
					});
				}
			}
		}

		errors
	}

	fn validate_current_catalog_uniqueness(
		&self,
		composition: &Composition,
		path_str: &str,
	) -> Vec<ValidationError> {
		let mut errors = Vec::new();
		let mut schemes_seen = HashSet::new();

		for attribution in &composition.attribution {
			let (Some(composer), Some(catalog)) = (&attribution.composer, &attribution.catalog) else {
				continue;
			};
			for entry in catalog {
				if !schemes_seen.insert((composer.clone(), entry.scheme.clone())) {
					continue;
				}
				let key = (composer.clone(), entry.scheme.clone(), entry.number.clone());
				if let Some(ids) = self.current_catalog_targets.get(&key) {
					let mut unique: Vec<_> = ids.iter().cloned().collect::<HashSet<_>>().into_iter().collect();
					if unique.len() > 1 {
						unique.sort();
						errors.push(ValidationError {
							path: path_str.to_string(),
							message: format!(
								"current catalog identifier {}:{}:{} is shared by compositions {}",
								composer, entry.scheme, entry.number, unique.join(", ")
							),
						});
					}
				}
			}
		}

		errors
	}

	pub fn validate_composition_file(&self, path: &Path) -> Vec<ValidationError> {
		if let Some(cached) = self.composition_cache.get(path) {
			return self.validate_cached_composition(path, cached);
		}

		let (value, errors) = match self.read_and_validate(path, &self.composition_schema, true) {
			Ok(result) => result,
			Err(errors) => return errors,
		};
		self.validate_composition_value(path, &value, errors)
	}

	fn validate_cached_composition(
		&self,
		path: &Path,
		cached: &CachedComposition,
	) -> Vec<ValidationError> {
		let path_str = path.display().to_string();
		match cached {
			CachedComposition::ReadError(message) => vec![ValidationError {
				path: path_str,
				message: format!("Failed to read file: {}", message),
			}],
			CachedComposition::InvalidJson {
				message,
				has_multiple_spaces,
			} => {
				let mut errors = Vec::new();
				if *has_multiple_spaces {
					errors.push(ValidationError {
						path: path_str.clone(),
						message: "Contains multiple consecutive spaces".into(),
					});
				}
				errors.push(ValidationError {
					path: path_str,
					message: format!("Invalid JSON: {}", message),
				});
				errors
			}
			CachedComposition::Parsed {
				value,
				has_multiple_spaces,
			} => {
				let mut errors = Vec::new();
				if *has_multiple_spaces {
					errors.push(ValidationError {
						path: path_str.clone(),
						message: "Contains multiple consecutive spaces".into(),
					});
				}
				let schema_errors = self.composition_schema.validate(value, &path_str);
				if !schema_errors.is_empty() {
					errors.extend(schema_errors);
					return errors;
				}
				self.validate_composition_value(path, value, errors)
			}
		}
	}

	fn validate_composition_value(
		&self,
		path: &Path,
		value: &Value,
		mut errors: Vec<ValidationError>,
	) -> Vec<ValidationError> {
		let path_str = path.display().to_string();
		let Some(comp) = deserialize_model::<Composition>(value, &path_str, &mut errors) else {
			return errors;
		};

		errors.extend(self.validate_id(&comp.id, path, &path_str));
		errors.extend(self.validate_key(&comp.key, &path_str));
		errors.extend(self.validate_attribution(&comp.attribution, &path_str, true));
		errors.extend(self.validate_current_catalog_uniqueness(&comp, &path_str));
		errors
	}

	fn validate_catalog_definition_domain(
		&self,
		definition: &CatalogDefinition,
		path_str: &str,
		location: &str,
	) -> Vec<ValidationError> {
		let mut errors = Vec::new();
		let capture_count = definition
			.pattern
			.as_ref()
			.and_then(|pattern| cached_regex(pattern))
			.map(|regex| regex.captures_len().saturating_sub(1));

		if let Some(constraints) = &definition.constraints {
			for constraint in constraints {
				if let Some(count) = capture_count {
					if constraint.group > count {
						errors.push(ValidationError {
							path: path_str.to_string(),
							message: format!(
								"{}: constraint group {} exceeds pattern capture count {}",
								location, constraint.group, count
							),
						});
					}
				}
				if let (Some(min), Some(max)) = (constraint.min, constraint.max) {
					if min > max {
						errors.push(ValidationError {
							path: path_str.to_string(),
							message: format!("{}: constraint group {} has min {} greater than max {}", location, constraint.group, min, max),
						});
					}
				}
				if let Some(ranges) = &constraint.ranges {
					for range in ranges {
						if range.min > range.max {
							errors.push(ValidationError {
								path: path_str.to_string(),
								message: format!("{}: constraint group {} has range {}..{}", location, constraint.group, range.min, range.max),
							});
						}
					}
				}
			}
		}

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
					path: path_str.clone(),
					message: format!("default_scheme '{}' is not defined for this composer or globally", default_scheme),
				});
			}
		}

		if let Some(catalogs) = &composer.catalogs {
			for (scheme, definition) in catalogs {
				if let Some(effective) = load_catalog_def(&self.data_dir, scheme, Some(&composer.id)) {
					errors.extend(self.validate_catalog_definition_domain(
						&effective,
						&path_str,
						&format!("catalog '{}'", scheme),
					));
				}
				if let Some(current_edition) = &definition.current_edition {
					let defined = definition
						.editions
						.as_ref()
						.map_or(false, |editions| editions.contains_key(current_edition));
					if !defined {
						errors.push(ValidationError {
							path: path_str.clone(),
							message: format!(
								"catalog '{}': current_edition '{}' is not defined in editions",
								scheme, current_edition
							),
						});
					}
				}
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
					path: path_str.clone(),
					message: format!("Catalog ID '{}' doesn't match filename '{}'", id, stem),
				});
			}
		}

		errors.extend(self.validate_catalog_definition_domain(&catalog, &path_str, "catalog"));
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

		errors.extend(self.validate_attribution(&collection.attribution, &path_str, false));

		let composer = collection
			.attribution
			.first()
			.and_then(|entry| entry.composer.as_deref())
			.or(collection.composer.as_deref())
			.or_else(|| collection.id.split_once('-').map(|(composer, _)| composer));

		let Some(composer) = composer else {
			errors.push(ValidationError {
				path: path_str,
				message: "cannot determine collection composer".into(),
			});
			return errors;
		};

		if !self.composers.contains(composer) {
			errors.push(ValidationError {
				path: path_str.clone(),
				message: format!("collection composer '{}' not found in composers/", composer),
			});
		}

		if !self.catalog_is_allowed_for_composer(composer, &collection.scheme) {
			errors.push(ValidationError {
				path: path_str.clone(),
				message: format!(
					"catalog scheme '{}' is not defined for composer '{}' or globally",
					collection.scheme, composer
				),
			});
			return errors;
		}

		let mut seen = HashSet::new();
		for (i, number) in collection.compositions.iter().enumerate() {
			let location = format!("compositions[{}]", i);
			if !seen.insert(number) {
				errors.push(ValidationError {
					path: path_str.clone(),
					message: format!("{}: duplicate collection member '{}'", location, number),
				});
			}

			errors.extend(self.validate_catalog_reference(
				composer,
				&collection.scheme,
				number,
				None,
				&path_str,
				&location,
			));

			let key = (composer.to_string(), collection.scheme.clone(), number.clone());
			match self.current_catalog_targets.get(&key) {
				None => errors.push(ValidationError {
					path: path_str.clone(),
					message: format!(
						"{}: {}:{}:{} does not resolve to a current composition",
						location, composer, collection.scheme, number
					),
				}),
				Some(ids) => {
					let unique: HashSet<_> = ids.iter().collect();
					if unique.len() > 1 {
						errors.push(ValidationError {
							path: path_str.clone(),
							message: format!("{}: catalog identifier resolves ambiguously", location),
						});
					}
				}
			}
		}

		errors
	}

	fn validate_inventory_file(&self, path: &Path) -> Vec<ValidationError> {
		let path_str = path.display().to_string();
		let inventory = match crate::inventory::load_inventory(path) {
			Ok(inventory) => inventory,
			Err(error) => {
				return vec![ValidationError {
					path: path_str,
					message: error.to_string(),
				}];
			}
		};
		let mut errors = Vec::new();

		if !self.composers.contains(&inventory.composer) {
			errors.push(ValidationError {
				path: path_str.clone(),
				message: format!("composer '{}' not found in composers/", inventory.composer),
			});
			return errors;
		}
		if !self.catalog_is_allowed_for_composer(&inventory.composer, &inventory.scheme) {
			errors.push(ValidationError {
				path: path_str.clone(),
				message: format!(
					"catalog scheme '{}' is not defined for composer '{}' or globally",
					inventory.scheme, inventory.composer
				),
			});
			return errors;
		}

		if let Some(path_composer) = inventory_composer_from_path(path) {
			if path_composer != inventory.composer {
				errors.push(ValidationError {
					path: path_str.clone(),
					message: format!(
						"inventory composer '{}' does not match path composer '{}'",
						inventory.composer, path_composer
					),
				});
			}
		}

		let definition = load_catalog_def(&self.data_dir, &inventory.scheme, Some(&inventory.composer));
		if let Some(edition) = inventory.edition.as_deref() {
			let defined = definition
				.as_ref()
				.and_then(|definition| definition.editions.as_ref())
				.map_or(false, |editions| editions.contains_key(edition));
			if !defined {
				errors.push(ValidationError {
					path: path_str.clone(),
					message: format!(
						"edition '{}' is not defined for {}:{}",
						edition, inventory.composer, inventory.scheme
					),
				});
			}
		}

		match normalize_inventory(&inventory, definition.as_ref()) {
			Ok(normalized) => {
				for number in &normalized.entries {
					errors.extend(self.validate_catalog_reference(
						&inventory.composer,
						&inventory.scheme,
						number,
						inventory.edition.as_deref(),
						&path_str,
						"entries",
					));
				}
			}
			Err(message) => errors.push(ValidationError {
				path: path_str,
				message,
			}),
		}

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

		if !id_pattern().is_match(id) {
			errors.push(ValidationError {
				path: path_str.to_string(),
				message: format!("ID '{}' is not 8 lowercase hex characters", id),
			});
			return errors;
		}

		let expected_id = extract_id_from_path(path).ok();
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
			if !key_pattern().is_match(k) {
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
				for (j, cat) in catalog.iter().enumerate() {
					let location = format!("attribution[{}].catalog[{}]", i, j);
					let scheme_defined = if let Some(composer) = &entry.composer {
						self.catalog_is_allowed_for_composer(composer, &cat.scheme)
					} else {
						self.catalog_schemes.contains(&cat.scheme)
					};

					if !scheme_defined {
						errors.push(ValidationError {
							path: path_str.to_string(),
							message: if let Some(composer) = &entry.composer {
								format!(
									"{}: catalog scheme '{}' is not defined for composer '{}' or globally",
									location, cat.scheme, composer
								)
							} else {
								format!("{}: catalog scheme '{}' not defined", location, cat.scheme)
							},
						});
					}

					if cat.scheme != cat.scheme.to_lowercase() {
						errors.push(ValidationError {
							path: path_str.to_string(),
							message: format!("{}: catalog scheme '{}' must be lowercase", location, cat.scheme),
						});
					}

					if !is_valid_catalog_number_case(&cat.scheme, &cat.number) {
						errors.push(ValidationError {
							path: path_str.to_string(),
							message: format!(
								"{}: catalog number '{}' must be lowercase{}",
								location,
								cat.number,
								if cat.scheme == "bwv" { " (R suffix allowed)" } else { "" }
							),
						});
					}

					if scheme_defined {
						if let Some(composer) = &entry.composer {
							errors.extend(self.validate_catalog_reference(
								composer,
								&cat.scheme,
								&cat.number,
								cat.edition.as_deref(),
								path_str,
								&location,
							));
						}
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
		collect_inventory_files(&data_dir.join("inventories"), &mut paths);
		paths.sort();

		let mut errors = Vec::new();
		for path in paths {
			errors.extend(self.validate_file(path));
		}

		let mut inventory_identities: HashMap<(String, String, Option<String>), Vec<String>> = HashMap::new();
		let mut inventory_paths = Vec::new();
		collect_inventory_files(&data_dir.join("inventories"), &mut inventory_paths);
		for path in inventory_paths {
			let Ok(inventory) = crate::inventory::load_inventory(&path) else {
				continue;
			};
			inventory_identities
				.entry((inventory.composer, inventory.scheme, inventory.edition))
				.or_default()
				.push(path.display().to_string());
		}
		for ((composer, scheme, edition), mut duplicate_paths) in inventory_identities {
			if duplicate_paths.len() < 2 {
				continue;
			}
			duplicate_paths.sort();
			let identity = edition.map_or_else(
				|| format!("{}:{}", composer, scheme),
				|edition| format!("{}:{} edition {}", composer, scheme, edition),
			);
			errors.push(ValidationError {
				path: duplicate_paths.join(", "),
				message: format!("duplicate inventory identity {}", identity),
			});
		}

		errors
	}
}

/// Case-sensitive, so these stay outside the shared case-insensitive regex cache.
fn id_pattern() -> &'static Regex {
	static PATTERN: OnceLock<Regex> = OnceLock::new();
	PATTERN.get_or_init(|| Regex::new(r"^[a-f0-9]{8}$").expect("valid composition id pattern"))
}

fn key_pattern() -> &'static Regex {
	static PATTERN: OnceLock<Regex> = OnceLock::new();
	PATTERN.get_or_init(|| {
		Regex::new(r"^[A-Ga-g][#b]?(\.(dor|phr|lyd|mix|loc))?$").expect("valid key pattern")
	})
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
			Some("inventories") => return Some(DataKind::Inventory),
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

fn collect_inventory_files(dir: &Path, paths: &mut Vec<PathBuf>) {
	let Ok(entries) = fs::read_dir(dir) else {
		return;
	};

	for entry in entries.flatten() {
		let path = entry.path();
		if path.is_dir() {
			collect_inventory_files(&path, paths);
		} else if path
			.extension()
			.map_or(false, |ext| ext == "toml" || ext == "json")
		{
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

fn inventory_composer_from_path(path: &Path) -> Option<&str> {
	let mut components = path.components();
	while let Some(component) = components.next() {
		if component.as_os_str().to_str() == Some("inventories") {
			return components.next()?.as_os_str().to_str();
		}
	}
	None
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
			data_dir: PathBuf::new(),
			composers: HashSet::new(),
			catalog_schemes: HashSet::new(),
			global_catalog_schemes: HashSet::new(),
			composer_catalog_schemes: HashMap::new(),
			current_catalog_targets: HashMap::new(),
			composition_cache: HashMap::new(),
			composition_schema: empty_schema(),
			composer_schema: empty_schema(),
			catalog_schema: empty_schema(),
			collection_schema: empty_schema(),
			inventory_index: InventoryIndex::default(),
		}
	}

	#[test]
	fn cached_dataset_composition_is_not_reread() {
		let temp = tempfile::tempdir().unwrap();
		let dir = temp.path().join("compositions").join("ab");
		fs::create_dir_all(&dir).unwrap();
		let path = dir.join("cd1234.json");
		fs::write(
			&path,
			r#"{"id":"abcd1234","form":"sonata","attribution":[{"composer":"bach"}]}"#,
		)
		.unwrap();

		let mut validator = test_validator();
		validator.composers.insert("bach".into());
		validator
			.composition_cache
			.insert(path.clone(), CachedComposition::load(&path));

		fs::write(&path, "{").unwrap();
		let errors = validator.validate_composition_file(&path);

		assert!(!errors.iter().any(|error| error.message.contains("Invalid JSON")));
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
		assert_eq!(data_kind(Path::new("inventories/beethoven/op.toml")), Some(DataKind::Inventory));
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

	#[test]
	fn test_catalog_scheme_scope() {
		let mut validator = test_validator();
		validator.global_catalog_schemes.insert("op".into());
		validator
			.composer_catalog_schemes
			.insert("bach".into(), HashSet::from(["bwv".into()]));

		assert!(validator.catalog_is_allowed_for_composer("bach", "bwv"));
		assert!(validator.catalog_is_allowed_for_composer("bach", "op"));
		assert!(!validator.catalog_is_allowed_for_composer("mozart", "bwv"));
	}

	#[test]
	fn test_current_catalog_collision() {
		let mut validator = test_validator();
		validator.current_catalog_targets.insert(
			("bach".into(), "bwv".into(), "812".into()),
			vec!["11111111".into(), "22222222".into()],
		);
		let composition: Composition = serde_json::from_str(r#"{
			"id": "11111111",
			"form": "suite",
			"attribution": [{
				"composer": "bach",
				"catalog": [{"scheme": "bwv", "number": "812"}]
			}]
		}"#).unwrap();

		let errors = validator.validate_current_catalog_uniqueness(&composition, "test");
		assert_eq!(errors.len(), 1);
	}

	#[test]
	fn test_catalog_pattern_edition_and_collection_resolution() {
		let tmp = tempfile::tempdir().unwrap();
		fs::create_dir_all(tmp.path().join("composers")).unwrap();
		fs::create_dir_all(tmp.path().join("collections/mozart")).unwrap();
		fs::write(tmp.path().join("composers/mozart.json"), r#"{
			"id": "mozart",
			"name": {"full": "Wolfgang Amadeus Mozart", "sort": "Mozart, Wolfgang Amadeus"},
			"catalogs": {
				"k": {
					"name": "Köchel-Verzeichnis",
					"pattern": "^\\d+$",
					"editions": {"9": {"year": 2024, "editor": "Neal Zaslaw"}}
				}
			}
		}"#).unwrap();

		let mut validator = test_validator();
		validator.data_dir = tmp.path().to_path_buf();
		validator.composers.insert("mozart".into());
		validator
			.composer_catalog_schemes
			.insert("mozart".into(), HashSet::from(["k".into()]));
		validator.current_catalog_targets.insert(
			("mozart".into(), "k".into(), "331".into()),
			vec!["11111111".into()],
		);

		assert!(validator
			.validate_catalog_reference("mozart", "k", "331", Some("9"), "test", "catalog")
			.is_empty());
		assert_eq!(
			validator
				.validate_catalog_reference("mozart", "k", "331a", Some("6"), "test", "catalog")
				.len(),
			2
		);

		let path = tmp.path().join("collections/mozart/test.json");
		fs::write(&path, r#"{
			"id": "mozart-test",
			"title": {"en": "Test"},
			"attribution": [{"composer": "mozart"}],
			"scheme": "k",
			"compositions": ["331"]
		}"#).unwrap();
		assert!(validator.validate_collection_file(&path).is_empty());
	}
}

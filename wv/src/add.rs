use std::fs;
use std::path::{Path, PathBuf};

use crate::parse::{load_composition, path_for_id};
use crate::types::Composition;
use crate::validate::Validator;

#[derive(Debug)]
pub enum AddError {
	ReadError(String),
	ParseError(String),
	ValidationError(Vec<String>),
	WriteError(String),
	AlreadyExists(String),
}

impl std::fmt::Display for AddError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			AddError::ReadError(e) => write!(f, "Failed to read file: {}", e),
			AddError::ParseError(e) => write!(f, "Failed to parse: {}", e),
			AddError::ValidationError(errs) => {
				writeln!(f, "Validation errors:")?;
				for e in errs {
					writeln!(f, "  {}", e)?;
				}
				Ok(())
			}
			AddError::WriteError(e) => write!(f, "Failed to write: {}", e),
			AddError::AlreadyExists(p) => write!(f, "File already exists: {}", p),
		}
	}
}

pub struct AddResult {
	pub id: String,
	pub source: PathBuf,
	pub destination: PathBuf,
}

#[derive(Debug)]
pub struct PreparedAdd {
	pub id: String,
	pub source: PathBuf,
	pub destination: PathBuf,
	pub composition: Composition,
	pub overwrites: bool,
	content: String,
}

pub fn prepare_composition<P: AsRef<Path>, Q: AsRef<Path>>(
	source: P,
	data_dir: Q,
	force: bool,
) -> Result<PreparedAdd, AddError> {
	let data_dir = data_dir.as_ref();
	let validator = Validator::new(data_dir);
	prepare_composition_with(source, data_dir, force, &validator)
}

/// As [`prepare_composition`], but reusing a validator the caller already built.
///
/// `Validator::new` reads every composition in the dataset, so constructing one
/// per incoming file makes a batch add quadratic.
pub fn prepare_composition_with<P: AsRef<Path>, Q: AsRef<Path>>(
	source: P,
	data_dir: Q,
	force: bool,
	validator: &Validator,
) -> Result<PreparedAdd, AddError> {
	let source = source.as_ref();
	let data_dir = data_dir.as_ref();

	let comp = load_composition(source).map_err(|e| AddError::ParseError(e.to_string()))?;

	let errors = validator.validate_composition_file(source);
	let non_path_errors: Vec<_> = errors
		.iter()
		.filter(|e| !e.message.contains("doesn't match path"))
		.collect();

	if !non_path_errors.is_empty() {
		return Err(AddError::ValidationError(
			non_path_errors.iter().map(|e| e.message.clone()).collect(),
		));
	}

	let id = &comp.id;
	let dest_path = path_for_id(data_dir.join("compositions"), id)
		.map_err(|error| AddError::ParseError(error.to_string()))?;
	let overwrites = dest_path.exists();

	if overwrites && !force {
		return Err(AddError::AlreadyExists(dest_path.display().to_string()));
	}

	let content = fs::read_to_string(source).map_err(|e| AddError::ReadError(e.to_string()))?;

	Ok(PreparedAdd {
		id: id.clone(),
		source: source.to_path_buf(),
		destination: dest_path,
		composition: comp,
		overwrites,
		content,
	})
}

pub fn commit_composition(prepared: PreparedAdd) -> Result<AddResult, AddError> {
	let dest_dir = prepared
		.destination
		.parent()
		.ok_or_else(|| AddError::WriteError("Destination has no parent directory".into()))?;
	fs::create_dir_all(dest_dir).map_err(|e| AddError::WriteError(e.to_string()))?;
	fs::write(&prepared.destination, prepared.content).map_err(|e| AddError::WriteError(e.to_string()))?;

	Ok(AddResult {
		id: prepared.id,
		source: prepared.source,
		destination: prepared.destination,
	})
}

pub fn add_composition<P: AsRef<Path>, Q: AsRef<Path>>(
	source: P,
	data_dir: Q,
	force: bool,
) -> Result<AddResult, AddError> {
	let prepared = prepare_composition(source, data_dir, force)?;
	commit_composition(prepared)
}

pub fn generate_id() -> String {
	let mut bytes = [0u8; 4];
	if getrandom::fill(&mut bytes).is_err() {
		use std::time::{SystemTime, UNIX_EPOCH};
		let nanos = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_default()
			.as_nanos();
		bytes = (nanos as u32).to_ne_bytes();
	}
	format!("{:08x}", u32::from_ne_bytes(bytes))
}

/// An ID with no existing composition file, so callers cannot silently overwrite.
/// The 32-bit ID space makes collisions plausible well before exhaustion, so
/// every generation path should route through here when a dataset is available.
pub fn generate_unique_id<P: AsRef<Path>>(data_dir: P) -> String {
	let compositions = data_dir.as_ref().join("compositions");
	for _ in 0..64 {
		let id = generate_id();
		match path_for_id(&compositions, &id) {
			Ok(path) if path.exists() => continue,
			_ => return id,
		}
	}
	generate_id()
}

pub fn scaffold_composition(id: &str, form: &str, composer: &str) -> String {
	format!(
		r#"{{
	"id": "{}",
	"form": "{}",
	"attribution": [
		{{
			"composer": "{}"
		}}
	]
}}"#,
		id, form, composer
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::HashSet;
	use tempfile::TempDir;

	fn setup_data_dir() -> TempDir {
		let tmp = TempDir::new().unwrap();
		fs::create_dir_all(tmp.path().join("schemas")).unwrap();
		fs::create_dir_all(tmp.path().join("composers")).unwrap();
		fs::write(tmp.path().join("schemas/composition.schema.json"), "{}").unwrap();
		fs::write(tmp.path().join("composers/mozart.json"), "{}").unwrap();
		tmp
	}

	fn write_source(root: &Path, name: &str, id: &str) -> PathBuf {
		let path = root.join(name);
		fs::write(
			&path,
			format!(
				r#"{{"id":"{}","form":"sonata","attribution":[{{"composer":"mozart"}}]}}"#,
				id
			),
		)
		.unwrap();
		path
	}

	#[test]
	fn test_generate_id() {
		let id1 = generate_id();

		assert_eq!(id1.len(), 8);
		assert!(id1.chars().all(|c| c.is_ascii_hexdigit()));

		let id2 = generate_id();
		assert_ne!(id1, id2);
	}

	#[test]
	fn generate_id_is_lowercase_hex() {
		for _ in 0..1000 {
			let id = generate_id();
			assert_eq!(id.len(), 8);
			assert!(id.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
		}
	}

	#[test]
	fn generate_id_does_not_repeat_in_a_tight_loop() {
		// the previous time-derived implementation returned duplicates here
		let ids: HashSet<String> = (0..5000).map(|_| generate_id()).collect();
		assert!(ids.len() > 4990, "only {} distinct ids from 5000", ids.len());
	}

	#[test]
	fn generate_unique_id_avoids_existing_files() {
		let tmp = TempDir::new().unwrap();
		let taken = generate_id();
		let path = path_for_id(tmp.path().join("compositions"), &taken).unwrap();
		fs::create_dir_all(path.parent().unwrap()).unwrap();
		fs::write(&path, "{}").unwrap();

		for _ in 0..50 {
			assert_ne!(generate_unique_id(tmp.path()), taken);
		}
	}

	#[test]
	fn test_scaffold_composition() {
		let json = scaffold_composition("abcd1234", "sonata", "beethoven");
		assert!(json.contains("\"id\": \"abcd1234\""));
		assert!(json.contains("\"form\": \"sonata\""));
		assert!(json.contains("\"composer\": \"beethoven\""));
	}

	#[test]
	fn prepare_rejects_malformed_ids_without_panicking() {
		let tmp = setup_data_dir();
		let source = write_source(tmp.path(), "incoming.json", "a\u{e9}aaaaa");
		// rejected (by validation, before the path is ever built) rather than panicking
		assert!(prepare_composition(&source, tmp.path(), false).is_err());
	}

	#[test]
	fn prepare_does_not_write_destination() {
		let tmp = setup_data_dir();
		let source = write_source(tmp.path(), "incoming.json", "ab123456");
		let prepared = prepare_composition(&source, tmp.path(), false).unwrap();
		assert_eq!(prepared.id, "ab123456");
		assert!(!prepared.destination.exists());
	}

	#[test]
	fn add_composition_still_writes_single_file() {
		let tmp = setup_data_dir();
		let source = write_source(tmp.path(), "incoming.json", "ab123456");
		let result = add_composition(&source, tmp.path(), false).unwrap();
		assert!(result.destination.exists());
		assert_eq!(fs::read_to_string(result.destination).unwrap(), fs::read_to_string(source).unwrap());
	}
}

use std::fs;
use std::path::{Path, PathBuf};

use crate::parse::load_composition;
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
	let source = source.as_ref();
	let data_dir = data_dir.as_ref();

	let comp = load_composition(source).map_err(|e| AddError::ParseError(e.to_string()))?;

	let validator = Validator::new(data_dir);
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
	if id.len() != 8 {
		return Err(AddError::ParseError(format!(
			"ID must be 8 characters, got {}",
			id.len()
		)));
	}
	let prefix = &id[..2];
	let suffix = &id[2..];
	let dest_dir = data_dir.join("compositions").join(prefix);
	let dest_path = dest_dir.join(format!("{}.json", suffix));
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
	use std::time::{SystemTime, UNIX_EPOCH};

	let duration = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default();

	let nanos = duration.as_nanos();
	let hash = (nanos ^ (nanos >> 32) ^ (nanos >> 64) ^ (nanos >> 96)) as u64;

	format!("{:08x}", hash as u32)
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

		std::thread::sleep(std::time::Duration::from_millis(1));
		let id2 = generate_id();
		assert_ne!(id1, id2);
	}

	#[test]
	fn test_scaffold_composition() {
		let json = scaffold_composition("abcd1234", "sonata", "beethoven");
		assert!(json.contains("\"id\": \"abcd1234\""));
		assert!(json.contains("\"form\": \"sonata\""));
		assert!(json.contains("\"composer\": \"beethoven\""));
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

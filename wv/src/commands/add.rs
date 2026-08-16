use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use crate::add::{commit_composition, prepare_composition_with, AddError, PreparedAdd};
use crate::validate::Validator;
use crate::config::Config;
use crate::display::{expand_title, ExpansionContext};
use crate::index::mark_index_dirty;
use crate::output::{format_id_header, print};

struct PreflightFailure {
	source: PathBuf,
	message: String,
	already_exists: bool,
}

struct BatchPlan {
	prepared: Vec<PreparedAdd>,
	failures: Vec<PreflightFailure>,
	total: usize,
}

fn discover_sources(path: &Path) -> Result<Vec<PathBuf>, String> {
	if path.is_file() {
		return Ok(vec![path.to_path_buf()]);
	}
	if !path.exists() {
		return Err(format!("Path does not exist: {}", path.display()));
	}
	if !path.is_dir() {
		return Err(format!("Path is not a file or directory: {}", path.display()));
	}

	let entries = fs::read_dir(path).map_err(|e| format!("Failed to read directory: {}", e))?;
	let mut sources = Vec::new();
	for entry in entries {
		let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
		let entry_path = entry.path();
		if entry_path.is_file()
			&& entry_path
				.extension()
				.and_then(|ext| ext.to_str())
				.is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
		{
			sources.push(entry_path);
		}
	}
	sources.sort();
	if sources.is_empty() {
		return Err(format!("No JSON files found in {}", path.display()));
	}
	Ok(sources)
}

fn current_catalog_keys(prepared: &PreparedAdd) -> Vec<(String, String, String)> {
	let mut seen = HashSet::new();
	let mut keys = Vec::new();
	for attribution in &prepared.composition.attribution {
		let (Some(composer), Some(catalog)) = (&attribution.composer, &attribution.catalog) else {
			continue;
		};
		for entry in catalog {
			let scheme_key = (composer.clone(), entry.scheme.clone());
			if seen.insert(scheme_key) {
				keys.push((composer.clone(), entry.scheme.clone(), entry.number.clone()));
			}
		}
	}
	keys
}

fn preflight(sources: &[PathBuf], force: bool, data_dir: &Path) -> BatchPlan {
	// Nothing is committed until the whole plan is approved, so the dataset does
	// not change underneath a single shared validator.
	let validator = Validator::new(data_dir);
	let mut prepared = Vec::new();
	let mut failures = Vec::new();
	let mut destinations: HashMap<PathBuf, PathBuf> = HashMap::new();
	let mut catalog_keys: HashMap<(String, String, String), (String, PathBuf)> = HashMap::new();

	for source in sources {
		match prepare_composition_with(source, data_dir, force, &validator) {
			Ok(plan) => {
				if let Some(first_source) = destinations.get(&plan.destination) {
					failures.push(PreflightFailure {
						source: source.clone(),
						message: format!(
							"duplicate composition ID {}; also provided by {}",
							plan.id,
							first_source.display()
						),
						already_exists: false,
					});
					continue;
				}

				let keys = current_catalog_keys(&plan);
				let catalog_conflict = keys.iter().find_map(|key| {
					catalog_keys.get(key).map(|(id, first_source)| {
						(key.clone(), id.clone(), first_source.clone())
					})
				});
				if let Some(((composer, scheme, number), id, first_source)) = catalog_conflict {
					failures.push(PreflightFailure {
						source: source.clone(),
						message: format!(
							"current catalog identifier {}:{}:{} is also used by {} in {}",
							composer,
							scheme,
							number,
							id,
							first_source.display()
						),
						already_exists: false,
					});
					continue;
				}

				destinations.insert(plan.destination.clone(), source.clone());
				for key in keys {
					catalog_keys.insert(key, (plan.id.clone(), source.clone()));
				}
				prepared.push(plan);
			}
			Err(error) => {
				let already_exists = matches!(&error, AddError::AlreadyExists(_));
				failures.push(PreflightFailure {
					source: source.clone(),
					message: error.to_string().trim_end().to_string(),
					already_exists,
				});
			}
		}
	}

	BatchPlan {
		prepared,
		failures,
		total: sources.len(),
	}
}

fn summary_rows(prepared: &[PreparedAdd], data_dir: &Path, config: &Config) -> Vec<String> {
	let rows: Vec<_> = prepared
		.iter()
		.map(|plan| {
			let ctx = ExpansionContext {
				composition: &plan.composition,
				collection: None,
				position_in_collection: None,
				config: &config.display,
			};
			let title = expand_title(&ctx);
			let catalog = format_id_header(&plan.composition, &plan.id, data_dir);
			(catalog, title, plan.id.clone(), plan.overwrites)
		})
		.collect();
	let width = rows.iter().map(|(catalog, _, _, _)| catalog.chars().count()).max().unwrap_or(0);
	rows.into_iter()
		.map(|(catalog, title, id, overwrites)| {
			let overwrite = if overwrites { "  [overwrite]" } else { "" };
			if catalog == id {
				format!("  {:width$}  {}{}", id, title, overwrite, width = width)
			} else {
				format!("  {:width$}  {}  {}{}", catalog, title, id, overwrite, width = width)
			}
		})
		.collect()
}

fn print_review(plan: &BatchPlan, data_dir: &Path, config: &Config) {
	if !plan.prepared.is_empty() {
		print(&format!(
			"{} composition{} ready to add:",
			plan.prepared.len(),
			if plan.prepared.len() == 1 { "" } else { "s" }
		));
		for row in summary_rows(&plan.prepared, data_dir, config) {
			print(&row);
		}
		print("");
	}

	let already_present = plan.failures.iter().filter(|failure| failure.already_exists).count();
	let invalid = plan.failures.len() - already_present;
	let overwrites = plan.prepared.iter().filter(|prepared| prepared.overwrites).count();
	print(&format!("{} ready", plan.prepared.len()));
	print(&format!("{} invalid", invalid));
	if overwrites > 0 {
		print(&format!("{} will be overwritten", overwrites));
	} else {
		print(&format!("{} already present", already_present));
	}
	if plan.total != plan.prepared.len() + plan.failures.len() {
		print(&format!("{} files examined", plan.total));
	}
}

fn confirm_with<R: BufRead, W: Write>(reader: &mut R, writer: &mut W, count: usize) -> io::Result<bool> {
	write!(
		writer,
		"Add {} composition{}? [y/N] ",
		count,
		if count == 1 { "" } else { "s" }
	)?;
	writer.flush()?;
	let mut response = String::new();
	reader.read_line(&mut response)?;
	Ok(matches!(response.trim().to_ascii_lowercase().as_str(), "y" | "yes"))
}

fn confirm(count: usize) -> io::Result<bool> {
	let stdin = io::stdin();
	let stdout = io::stdout();
	let mut reader = stdin.lock();
	let mut writer = stdout.lock();
	confirm_with(&mut reader, &mut writer, count)
}

pub fn run(
	path: &Path,
	force: bool,
	interactive: bool,
	dry_run: bool,
	data_dir: &Path,
	config: &Config,
) {
	let directory_input = path.is_dir();
	let sources = match discover_sources(path) {
		Ok(sources) => sources,
		Err(error) => {
			eprintln!("Error: {}", error);
			std::process::exit(1);
		}
	};
	let plan = preflight(&sources, force, data_dir);

	if !plan.failures.is_empty() {
		if directory_input || interactive || dry_run {
			print_review(&plan, data_dir, config);
			print("");
			print("Errors:");
			for failure in &plan.failures {
				eprintln!("  {}: {}", failure.source.display(), failure.message);
			}
			eprintln!("No files were added.");
		} else {
			eprintln!("Error: {}", plan.failures[0].message);
		}
		std::process::exit(1);
	}

	if directory_input || interactive || dry_run {
		print_review(&plan, data_dir, config);
	}

	if dry_run {
		print("Dry run: no files added.");
		return;
	}

	if interactive {
		match confirm(plan.prepared.len()) {
			Ok(true) => {}
			Ok(false) => {
				print("No files added.");
				return;
			}
			Err(error) => {
				eprintln!("Error reading confirmation: {}", error);
				std::process::exit(1);
			}
		}
	}

	let mut results = Vec::with_capacity(plan.prepared.len());
	for prepared in plan.prepared {
		match commit_composition(prepared) {
			Ok(result) => results.push(result),
			Err(error) => {
				eprintln!("Error: {}", error);
				if !results.is_empty() {
					eprintln!("{} composition(s) had already been written.", results.len());
				}
				std::process::exit(1);
			}
		}
	}

	if let Err(error) = mark_index_dirty(data_dir) {
		eprintln!("warning: failed to mark index stale: {}", error);
	}

	if !directory_input && !interactive && results.len() == 1 {
		let result = &results[0];
		print(&format!(
			"Added {} -> {}",
			result.source.display(),
			result.destination.display()
		));
		print(&format!("ID: {}", result.id));
	} else {
		print(&format!(
			"Added {} composition{}.",
			results.len(),
			if results.len() == 1 { "" } else { "s" }
		));
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::io::Cursor;
	use tempfile::TempDir;

	fn setup_data_dir() -> TempDir {
		let tmp = TempDir::new().unwrap();
		fs::create_dir_all(tmp.path().join("schemas")).unwrap();
		fs::create_dir_all(tmp.path().join("composers")).unwrap();
		fs::write(tmp.path().join("schemas/composition.schema.json"), "{}").unwrap();
		fs::write(tmp.path().join("composers/mozart.json"), "{}").unwrap();
		tmp
	}

	fn composition(id: &str) -> String {
		format!(
			r#"{{"id":"{}","form":"sonata","attribution":[{{"composer":"mozart"}}]}}"#,
			id
		)
	}

	#[test]
	fn discovers_immediate_json_files_in_sorted_order() {
		let tmp = TempDir::new().unwrap();
		fs::write(tmp.path().join("b.json"), "{}").unwrap();
		fs::write(tmp.path().join("a.json"), "{}").unwrap();
		fs::write(tmp.path().join("ignore.txt"), "{}").unwrap();
		fs::create_dir(tmp.path().join("nested")).unwrap();
		fs::write(tmp.path().join("nested/c.json"), "{}").unwrap();
		let sources = discover_sources(tmp.path()).unwrap();
		let names: Vec<_> = sources
			.iter()
			.map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
			.collect();
		assert_eq!(names, vec!["a.json", "b.json"]);
	}

	#[test]
	fn preflight_failure_writes_nothing() {
		let tmp = setup_data_dir();
		let input = tmp.path().join("incoming");
		fs::create_dir(&input).unwrap();
		fs::write(input.join("a.json"), composition("ab123456")).unwrap();
		fs::write(input.join("b.json"), "not json").unwrap();
		let sources = discover_sources(&input).unwrap();
		let plan = preflight(&sources, false, tmp.path());
		assert_eq!(plan.prepared.len(), 1);
		assert_eq!(plan.failures.len(), 1);
		assert!(!tmp.path().join("compositions/ab/123456.json").exists());
	}

	#[test]
	fn preflight_rejects_duplicate_ids_in_batch() {
		let tmp = setup_data_dir();
		let input = tmp.path().join("incoming");
		fs::create_dir(&input).unwrap();
		fs::write(input.join("a.json"), composition("ab123456")).unwrap();
		fs::write(input.join("b.json"), composition("ab123456")).unwrap();
		let sources = discover_sources(&input).unwrap();
		let plan = preflight(&sources, false, tmp.path());
		assert_eq!(plan.prepared.len(), 1);
		assert_eq!(plan.failures.len(), 1);
		assert!(plan.failures[0].message.contains("duplicate composition ID"));
	}

	#[test]
	fn confirmation_defaults_to_no() {
		for input in ["\n", "n\n", "no\n"] {
			let mut reader = Cursor::new(input.as_bytes());
			let mut writer = Vec::new();
			assert!(!confirm_with(&mut reader, &mut writer, 2).unwrap());
		}
	}

	#[test]
	fn confirmation_accepts_yes() {
		for input in ["y\n", "yes\n", "YES\n"] {
			let mut reader = Cursor::new(input.as_bytes());
			let mut writer = Vec::new();
			assert!(confirm_with(&mut reader, &mut writer, 2).unwrap());
		}
	}
}

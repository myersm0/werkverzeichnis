use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::catalog::{load_catalog_def, CatalogLoadError};
use crate::config::Config;
use crate::display::{expand_title, format_catalog, ExpansionContext};
use crate::parse::{load_composition, path_for_id, ParseError};
use crate::query::QueryResult;
use crate::types::{CatalogDefinition, Composition};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OutputError {
	#[error("failed to load composition {path}: {source}")]
	Composition { path: PathBuf, #[source] source: ParseError },
	#[error(transparent)]
	Catalog(#[from] CatalogLoadError),
	#[error(transparent)]
	Json(#[from] serde_json::Error),
}

fn load_required_composition(path: &Path) -> Result<Composition, OutputError> {
	load_composition(path).map_err(|source| OutputError::Composition {
		path: path.to_path_buf(),
		source,
	})
}

fn load_optional_composition(path: &Path) -> Result<Option<Composition>, OutputError> {
	match load_composition(path) {
		Ok(composition) => Ok(Some(composition)),
		Err(ParseError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
		Err(source) => Err(OutputError::Composition {
			path: path.to_path_buf(),
			source,
		}),
	}
}

pub fn print(s: &str) {
	if let Err(error) = writeln!(io::stdout(), "{}", s) {
		if error.kind() == io::ErrorKind::BrokenPipe {
			std::process::exit(0);
		}

		eprintln!("Error writing to stdout: {}", error);
		std::process::exit(1);
	}
}

pub fn id_to_path(data_dir: &Path, id: &str) -> std::path::PathBuf {
	let compositions = data_dir.join("compositions");
	path_for_id(&compositions, id).unwrap_or_else(|_| {
		let sanitized = id.replace(|c: char| c == '/' || c == '\\', "_");
		compositions.join(format!("{}.json", sanitized))
	})
}

fn first_catalog(comp: &Composition) -> Option<(&str, &str)> {
	comp.attribution
		.first()
		.and_then(|attr| attr.catalog.as_ref())
		.and_then(|cats| cats.first())
		.map(|c| (c.scheme.as_str(), c.number.as_str()))
}

pub fn format_id_header(
	comp: &Composition,
	id: &str,
	data_dir: &Path,
) -> Result<String, OutputError> {
	if let Some(attr) = comp.attribution.first() {
		if let Some(cat) = attr.catalog.as_ref().and_then(|c| c.first()) {
			let catalog_defn = load_catalog_def(data_dir, &cat.scheme, attr.composer.as_deref())?;
			return Ok(format_catalog(&cat.scheme, &cat.number, catalog_defn.as_ref()));
		}
	}
	Ok(id.to_string())
}

pub struct OutputContext<'a> {
	pub data_dir: &'a Path,
	pub config: &'a Config,
	pub scheme: Option<&'a str>,
	pub catalog_defn: Option<&'a CatalogDefinition>,
}

pub fn output_json(results: &[QueryResult], ctx: &OutputContext) -> Result<(), OutputError> {
	let mut output: Vec<serde_json::Value> = Vec::with_capacity(results.len());

	for result in results {
		let comp_path = id_to_path(ctx.data_dir, &result.id);
		let comp = load_required_composition(&comp_path)?;
		output.push(serde_json::to_value(&comp)?);
	}

	let json_str = serde_json::to_string_pretty(&output)?;
	print(&json_str);
	Ok(())
}

pub fn output_movements(results: &[QueryResult], ctx: &OutputContext) -> Result<(), OutputError> {
	let multi = results.len() > 1;

	for result in results {
		let comp_path = id_to_path(ctx.data_dir, &result.id);
		let comp = load_required_composition(&comp_path)?;

		if multi {
			let header = match (&result.number, ctx.scheme) {
				(Some(n), Some(s)) => format_catalog(s, n, ctx.catalog_defn),
				_ => {
					if let Some((scheme, number)) = first_catalog(&comp) {
						let defn = load_catalog_def(
							ctx.data_dir,
							scheme,
							comp.attribution.first().and_then(|a| a.composer.as_deref()),
						)?;
						format_catalog(scheme, number, defn.as_ref())
					} else {
						result.id.clone()
					}
				}
			};
			print(&format!("{}:", header));
		}

		let prefix = if multi { "  " } else { "" };

		if let Some(movements) = &comp.movements {
			for (i, movement) in movements.iter().enumerate() {
				let title = movement
					.title
					.as_deref()
					.or(movement.form.as_deref())
					.unwrap_or("?");
				print(&format!("{}{}. {}", prefix, i + 1, title));
			}
		} else if let Some(sections) = &comp.sections {
			for (i, section) in sections.iter().enumerate() {
				let title = section
					.title
					.as_deref()
					.or(section.form.as_deref())
					.unwrap_or("?");
				print(&format!("{}{}. {}", prefix, i + 1, title));
			}
		}

		if multi {
			print("");
		}
	}
	Ok(())
}

pub fn output_terse(results: &[QueryResult]) {
	for result in results {
		print(&result.id);
	}
}

pub fn output_pretty(results: &[QueryResult], ctx: &OutputContext) -> Result<(), OutputError> {
	for result in results {
		let comp_path = id_to_path(ctx.data_dir, &result.id);
		let comp = load_required_composition(&comp_path)?;
		let expansion_ctx = ExpansionContext {
			composition: &comp,
			collection: None,
			position_in_collection: None,
			config: &ctx.config.display,
		};
		let title = expand_title(&expansion_ctx);

		let catalog_str = match (&result.number, ctx.scheme) {
			(Some(n), Some(s)) => format_catalog(s, n, ctx.catalog_defn),
			_ => {
				if let Some((scheme, number)) = first_catalog(&comp) {
					let defn = load_catalog_def(
						ctx.data_dir,
						scheme,
						comp.attribution.first().and_then(|a| a.composer.as_deref()),
					)?;
					format_catalog(scheme, number, defn.as_ref())
				} else {
					result.id.clone()
				}
			}
		};
		print(&format!("{}, {}", title, catalog_str));
	}
	Ok(())
}

pub fn output_by_ids(
	ids: &[String],
	data_dir: &Path,
	config: &Config,
	terse: bool,
	movements: bool,
	json: bool,
) -> Result<(), OutputError> {
	if terse {
		for id in ids {
			print(id);
		}
		return Ok(());
	}

	if json {
		let mut output = Vec::new();
		for id in ids {
			let path = id_to_path(data_dir, id);
			if let Some(comp) = load_optional_composition(&path)? {
				output.push(serde_json::to_value(&comp)?);
			}
		}
		print(&serde_json::to_string_pretty(&output)?);
		return Ok(());
	}

	if movements {
		for id in ids {
			let path = id_to_path(data_dir, id);
			let Some(comp) = load_optional_composition(&path)? else {
				continue;
			};
			if let Some(movements) = &comp.movements {
				for (i, movement) in movements.iter().enumerate() {
					let title = movement.title.as_deref().or(movement.form.as_deref()).unwrap_or("?");
					print(&format!("{}. {}", i + 1, title));
				}
			} else if let Some(sections) = &comp.sections {
				for (i, section) in sections.iter().enumerate() {
					let title = section.title.as_deref().or(section.form.as_deref()).unwrap_or("?");
					print(&format!("{}. {}", i + 1, title));
				}
			}
		}
		return Ok(());
	}

	for id in ids {
		let comp_path = id_to_path(data_dir, id);
		if let Some(comp) = load_optional_composition(&comp_path)? {
			let expansion_ctx = ExpansionContext {
				composition: &comp,
				collection: None,
				position_in_collection: None,
				config: &config.display,
			};
			let title = expand_title(&expansion_ctx);
			let header = format_id_header(&comp, id, data_dir)?;
			print(&format!("{}, {}", title, header));
		} else {
			print(id);
		}
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn id_to_path_builds_sharded_path() {
		let path = id_to_path(Path::new("/data"), "abcd1234");
		assert_eq!(path, Path::new("/data/compositions/ab/cd1234.json"));
	}

	#[test]
	fn id_to_path_does_not_panic_on_malformed_ids() {
		for id in ["", "a", "ab", "a\u{e9}aaaaa", "much-too-long-to-be-an-id"] {
			let path = id_to_path(Path::new("/data"), id);
			assert!(path.starts_with("/data/compositions"));
		}
	}

	#[test]
	fn id_to_path_does_not_escape_the_compositions_directory() {
		let path = id_to_path(Path::new("/data"), "../../etc/passwd");
		assert_eq!(path.parent(), Some(Path::new("/data/compositions")));
	}
}

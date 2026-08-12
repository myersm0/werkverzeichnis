use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
	pub data_dir: Option<PathBuf>,
	pub editor: Option<String>,
	pub display: DisplayConfig,
	pub xref: XrefConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct XrefConfig {
	pub mb_database: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DisplayConfig {
	pub language: String,
	pub key_symbols: KeySymbols,
	pub patterns: PatternConfig,
	pub keys: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum KeySymbols {
	Unicode,
	Ascii,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PatternConfig {
	pub generic: String,
	pub with_number: String,
	pub instrumentation_max_chars: usize,
}

impl Default for Config {
	fn default() -> Self {
		Self {
			data_dir: None,
			editor: None,
			display: DisplayConfig::default(),
			xref: XrefConfig::default(),
		}
	}
}

impl Default for XrefConfig {
	fn default() -> Self {
		Self {
			mb_database: None,
		}
	}
}

impl Default for DisplayConfig {
	fn default() -> Self {
		Self {
			language: "en".into(),
			key_symbols: KeySymbols::Unicode,
			patterns: PatternConfig::default(),
			keys: HashMap::new(),
		}
	}
}

impl Default for KeySymbols {
	fn default() -> Self {
		Self::Unicode
	}
}

impl Default for PatternConfig {
	fn default() -> Self {
		Self {
			generic: "{form} in {key}".into(),
			with_number: "{form} no. {num} in {key}".into(),
			instrumentation_max_chars: 40,
		}
	}
}

impl Config {
	pub fn load() -> Self {
		let path = config_path();
		if path.exists() {
			match fs::read_to_string(&path) {
				Ok(content) => match toml::from_str(&content) {
					Ok(config) => return config,
					Err(e) => {
						eprintln!("Warning: Failed to parse config: {}", e);
					}
				},
				Err(e) => {
					eprintln!("Warning: Failed to read config: {}", e);
				}
			}
		}
		Config::default()
	}
}

fn config_path() -> PathBuf {
	if let Some(home) = dirs::home_dir() {
		let unix_path = home.join(".config").join("wv").join("config.toml");
		if unix_path.exists() {
			return unix_path;
		}
	}
	if let Some(config_dir) = dirs::config_dir() {
		config_dir.join("wv").join("config.toml")
	} else {
		PathBuf::from(".wv.toml")
	}
}

#[derive(Debug, thiserror::Error)]
pub enum DataDirError {
	#[error("{origin} data directory is not a werkverzeichnis dataset: {path}")]
	Invalid {
		origin: &'static str,
		path: PathBuf,
	},
	#[error(
		"could not locate werkverzeichnis data (searched ancestors of {current}, bundled data beside the executable, and {installed}); use --data-dir, WV_DATA_DIR, or data_dir in config.toml"
	)]
	NotFound {
		current: PathBuf,
		installed: String,
	},
}

const DATA_SUBDIRS: [&str; 5] = [
	"catalogs",
	"collections",
	"composers",
	"compositions",
	"schemas",
];

pub fn is_data_dir(path: &Path) -> bool {
	DATA_SUBDIRS.iter().all(|name| path.join(name).is_dir())
}

fn require_data_dir(path: PathBuf, origin: &'static str) -> Result<PathBuf, DataDirError> {
	if is_data_dir(&path) {
		Ok(path)
	} else {
		Err(DataDirError::Invalid { origin, path })
	}
}

fn find_data_dir_from(start: &Path) -> Option<PathBuf> {
	start
		.ancestors()
		.find(|path| is_data_dir(path))
		.map(Path::to_path_buf)
}

fn bundled_data_dir() -> Option<PathBuf> {
	std::env::current_exe()
		.ok()
		.and_then(|path| path.parent().map(|parent| parent.join("data")))
}

fn installed_data_dir() -> Option<PathBuf> {
	dirs::data_dir().map(|path| path.join("werkverzeichnis"))
}

pub fn resolve_data_dir(
	cli_arg: Option<&PathBuf>,
	config: &Config,
) -> Result<PathBuf, DataDirError> {
	if let Some(dir) = cli_arg {
		return require_data_dir(dir.clone(), "--data-dir");
	}

	if let Ok(dir) = std::env::var("WV_DATA_DIR") {
		return require_data_dir(PathBuf::from(dir), "WV_DATA_DIR");
	}

	if let Some(dir) = &config.data_dir {
		return require_data_dir(dir.clone(), "config.toml");
	}

	let current = std::env::current_dir().unwrap_or_default();
	if let Some(dir) = find_data_dir_from(&current) {
		return Ok(dir);
	}

	if let Some(dir) = bundled_data_dir().filter(|path| is_data_dir(path)) {
		return Ok(dir);
	}

	let installed = installed_data_dir();
	if let Some(dir) = installed.as_ref().filter(|path| is_data_dir(path)) {
		return Ok(dir.clone());
	}

	Err(DataDirError::NotFound {
		current,
		installed: installed
			.map(|path| path.display().to_string())
			.unwrap_or_else(|| "<platform data directory unavailable>".into()),
	})
}

pub fn resolve_editor(config: &Config) -> String {
	// 1. Config file
	if let Some(editor) = &config.editor {
		return editor.clone();
	}

	// 2. Environment variable
	if let Ok(editor) = std::env::var("EDITOR") {
		return editor;
	}

	// 3. Fallback
	"vi".into()
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::tempdir;

	fn create_data_dir(path: &Path) {
		for name in DATA_SUBDIRS {
			std::fs::create_dir_all(path.join(name)).unwrap();
		}
	}

	#[test]
	fn data_dir_requires_all_canonical_subdirectories() {
		let temp = tempdir().unwrap();
		create_data_dir(temp.path());
		assert!(is_data_dir(temp.path()));

		std::fs::remove_dir(temp.path().join("schemas")).unwrap();
		assert!(!is_data_dir(temp.path()));
	}

	#[test]
	fn finds_checkout_from_nested_directory() {
		let temp = tempdir().unwrap();
		create_data_dir(temp.path());
		let nested = temp.path().join("wv").join("src").join("bin");
		std::fs::create_dir_all(&nested).unwrap();

		assert_eq!(find_data_dir_from(&nested), Some(temp.path().to_path_buf()));
	}
}

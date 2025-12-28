use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
	pub data_dir: Option<PathBuf>,
	pub editor: Option<String>,
	pub display: DisplayConfig,
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
	if let Some(config_dir) = dirs::config_dir() {
		config_dir.join("wv").join("config.toml")
	} else {
		PathBuf::from(".wv.toml")
	}
}

pub fn resolve_data_dir(
	cli_arg: Option<&PathBuf>,
	config: &Config,
) -> PathBuf {
	// 1. CLI flag
	if let Some(dir) = cli_arg {
		return dir.clone();
	}

	// 2. Environment variable
	if let Ok(dir) = std::env::var("WV_DATA_DIR") {
		return PathBuf::from(dir);
	}

	// 3. Config file
	if let Some(dir) = &config.data_dir {
		return dir.clone();
	}

	// 4. Current/parent directory
	let current = std::env::current_dir().unwrap_or_default();
	if current.join("composers").exists() {
		return current;
	}

	let parent = current.parent().map(|p| p.to_path_buf()).unwrap_or(current.clone());
	if parent.join("composers").exists() {
		return parent;
	}

	current
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

use std::collections::HashMap;

use crate::config::{DisplayConfig, KeySymbols};
use crate::types::{CatalogDefinition, Collection, Composition};

pub fn expand_key(code: &str, config: &DisplayConfig) -> String {
	// Check user overrides first
	if let Some(expanded) = config.keys.get(code) {
		return expanded.clone();
	}

	// If already expanded (contains major/minor/Dur/Moll), return as-is
	let lower = code.to_lowercase();
	if lower.contains("major") || lower.contains("minor") 
		|| lower.contains("dur") || lower.contains("moll")
		|| lower.contains("dorian") || lower.contains("phrygian")
		|| lower.contains("lydian") || lower.contains("mixolydian")
		|| lower.contains("locrian")
	{
		return code.to_string();
	}

	// Check translation table - returns already-correct format
	let translations = key_translations(&config.language);
	if let Some(expanded) = translations.get(code) {
		return expanded.to_string();
	}

	// Parse and expand dynamically (only this path needs symbol conversion)
	expand_key_dynamic(code, config)
}

fn expand_key_dynamic(code: &str, config: &DisplayConfig) -> String {
	let is_minor = code.chars().next().map_or(false, |c| c.is_lowercase());
	let base = code.to_uppercase();

	let (note, accidental, mode_suffix) = parse_key_code(&base);

	let note_str = match config.key_symbols {
		KeySymbols::Unicode => format_note_unicode(&note, &accidental),
		KeySymbols::Ascii => format_note_ascii(&note, &accidental),
	};

	let quality = if is_minor { "minor" } else { "major" };

	let mode = match mode_suffix.as_deref() {
		Some("dor") => "Dorian",
		Some("phr") => "Phrygian",
		Some("lyd") => "Lydian",
		Some("mix") => "Mixolydian",
		Some("loc") => "Locrian",
		_ => quality,
	};

	// For minor keys, lowercase the note in output
	let final_note = if is_minor && mode_suffix.is_none() {
		note_str.to_lowercase()
	} else {
		note_str
	};

	format!("{} {}", final_note, mode)
}

fn parse_key_code(code: &str) -> (String, String, Option<String>) {
	let code = code.trim();

	// Check for mode suffix
	let (main, mode) = if let Some(idx) = code.find('.') {
		(code[..idx].to_string(), Some(code[idx + 1..].to_lowercase()))
	} else {
		(code.to_string(), None)
	};

	let note = main.chars().next().unwrap_or('C').to_string();
	let accidental = main.chars().skip(1).collect::<String>();

	(note, accidental, mode)
}

fn format_note_unicode(note: &str, accidental: &str) -> String {
	let acc = match accidental.to_uppercase().as_str() {
		"#" => "♯",
		"B" => "♭",
		"BB" => "𝄫",
		"##" | "X" => "𝄪",
		_ => "",
	};
	format!("{}{}", note, acc)
}

fn format_note_ascii(note: &str, accidental: &str) -> String {
	let acc = match accidental.to_uppercase().as_str() {
		"#" => "#",
		"B" => "b",
		"BB" => "bb",
		"##" | "X" => "##",
		_ => "",
	};
	format!("{}{}", note, acc)
}

fn key_translations(language: &str) -> HashMap<&'static str, &'static str> {
	match language {
		"de" => german_keys(),
		_ => english_keys(),
	}
}

fn english_keys() -> HashMap<&'static str, &'static str> {
	let mut m = HashMap::new();
	// Major keys
	m.insert("C", "C major");
	m.insert("D", "D major");
	m.insert("E", "E major");
	m.insert("F", "F major");
	m.insert("G", "G major");
	m.insert("A", "A major");
	m.insert("B", "B major");
	m.insert("F#", "F♯ major");
	m.insert("C#", "C♯ major");
	m.insert("Bb", "B♭ major");
	m.insert("Eb", "E♭ major");
	m.insert("Ab", "A♭ major");
	m.insert("Db", "D♭ major");
	m.insert("Gb", "G♭ major");
	m.insert("Cb", "C♭ major");
	// Minor keys
	m.insert("c", "c minor");
	m.insert("d", "d minor");
	m.insert("e", "e minor");
	m.insert("f", "f minor");
	m.insert("g", "g minor");
	m.insert("a", "a minor");
	m.insert("b", "b minor");
	m.insert("f#", "f♯ minor");
	m.insert("c#", "c♯ minor");
	m.insert("g#", "g♯ minor");
	m.insert("bb", "b♭ minor");
	m.insert("eb", "e♭ minor");
	m
}

fn german_keys() -> HashMap<&'static str, &'static str> {
	let mut m = HashMap::new();
	// Major keys (Dur)
	m.insert("C", "C-Dur");
	m.insert("D", "D-Dur");
	m.insert("E", "E-Dur");
	m.insert("F", "F-Dur");
	m.insert("G", "G-Dur");
	m.insert("A", "A-Dur");
	m.insert("B", "H-Dur"); // German B = H
	m.insert("F#", "Fis-Dur");
	m.insert("C#", "Cis-Dur");
	m.insert("Bb", "B-Dur"); // German Bb = B
	m.insert("Eb", "Es-Dur");
	m.insert("Ab", "As-Dur");
	m.insert("Db", "Des-Dur");
	m.insert("Gb", "Ges-Dur");
	// Minor keys (Moll)
	m.insert("c", "c-Moll");
	m.insert("d", "d-Moll");
	m.insert("e", "e-Moll");
	m.insert("f", "f-Moll");
	m.insert("g", "g-Moll");
	m.insert("a", "a-Moll");
	m.insert("b", "h-Moll");
	m.insert("f#", "fis-Moll");
	m.insert("c#", "cis-Moll");
	m.insert("g#", "gis-Moll");
	m.insert("bb", "b-Moll");
	m.insert("eb", "es-Moll");
	m
}

pub fn format_form(form: &str) -> String {
	form.split_whitespace()
		.map(|word| {
			let mut chars = word.chars();
			match chars.next() {
				Some(c) => {
					let rest: String = chars.collect();
					format!("{}{}", c.to_uppercase(), rest.to_lowercase())
				}
				None => String::new(),
			}
		})
		.collect::<Vec<_>>()
		.join(" ")
}

pub fn format_catalog(scheme: &str, number: &str, defn: Option<&CatalogDefinition>) -> String {
	// Use canonical_format if available, otherwise default format
	let base_format = defn
		.and_then(|d| d.canonical_format.as_ref())
		.map(|f| f.replace("{number}", "{}"))
		.unwrap_or_else(|| {
			// Default: style per OpenOpus guidelines
			match scheme.to_lowercase().as_str() {
				"op" => "op. {}".to_string(),
				"bwv" => "BWV {}".to_string(),
				"k" | "kv" => "K. {}".to_string(),
				"hob" => "Hob. {}".to_string(),
				"d" => "D. {}".to_string(),
				"woo" => "WoO {}".to_string(),
				_ => format!("{} {{}}", scheme.to_uppercase()),
			}
		});
	
	// Handle sub-numbers (e.g., "10/2" → "op. 10 no. 2")
	if let Some((main, sub)) = number.split_once('/') {
		let formatted_main = base_format.replace("{}", main);
		format!("{} no. {}", formatted_main, sub)
	} else {
		base_format.replace("{}", number)
	}
}

pub fn truncate_instrumentation(inst: &str, max_chars: usize) -> String {
	if inst.len() <= max_chars {
		inst.to_string()
	} else {
		format!("{}…", &inst[..max_chars.saturating_sub(1)])
	}
}

pub struct ExpansionContext<'a> {
	pub composition: &'a Composition,
	pub collection: Option<&'a Collection>,
	pub position_in_collection: Option<usize>,
	pub config: &'a DisplayConfig,
}

pub fn expand_title(ctx: &ExpansionContext) -> String {
	let comp = ctx.composition;
	let config = ctx.config;

	// 1. Use explicit title if present
	if let Some(title) = &comp.title {
		if let Some(t) = title.get(&config.language) {
			return t.clone();
		}
		if let Some(t) = title.get("en") {
			return t.clone();
		}
		if let Some((_, t)) = title.iter().next() {
			return t.clone();
		}
	}

	// 2. Use collection expansion_pattern if available
	if let Some(coll) = ctx.collection {
		if let Some(patterns) = &coll.expansion_pattern {
			let pattern = patterns
				.get(&config.language)
				.or_else(|| patterns.get("en"))
				.or_else(|| patterns.values().next());

			if let Some(p) = pattern {
				return expand_pattern(p, ctx);
			}
		}
	}

	// 3. Use default patterns from config
	let pattern = if ctx.position_in_collection.is_some() {
		&config.patterns.with_number
	} else {
		&config.patterns.generic
	};

	expand_pattern(pattern, ctx)
}

fn expand_pattern(pattern: &str, ctx: &ExpansionContext) -> String {
	let comp = ctx.composition;
	let config = ctx.config;

	let form = format_form(&comp.form);
	let key = comp
		.key
		.as_ref()
		.map(|k| expand_key(k, config))
		.unwrap_or_default();

	let num = ctx.position_in_collection.map(|n| n.to_string()).unwrap_or_default();

	let catalog = comp
		.attribution
		.first()
		.and_then(|a| a.catalog.as_ref())
		.and_then(|c| c.first())
		.map(|c| format!("{}:{}", c.scheme.to_uppercase(), c.number))
		.unwrap_or_default();

	let instrumentation = comp
		.instrumentation
		.as_ref()
		.map(|i| truncate_instrumentation(i, config.patterns.instrumentation_max_chars))
		.unwrap_or_default();

	pattern
		.replace("{form}", &form)
		.replace("{key}", &key)
		.replace("{num}", &num)
		.replace("{catalog}", &catalog)
		.replace("{instrumentation}", &instrumentation)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_expand_key_major() {
		let config = DisplayConfig::default();
		assert_eq!(expand_key("C", &config), "C major");
		assert_eq!(expand_key("F#", &config), "F♯ major");
		assert_eq!(expand_key("Bb", &config), "B♭ major");
	}

	#[test]
	fn test_expand_key_minor() {
		let config = DisplayConfig::default();
		assert_eq!(expand_key("c", &config), "c minor");
		assert_eq!(expand_key("f#", &config), "f♯ minor");
	}

	#[test]
	fn test_expand_key_german() {
		let config = DisplayConfig {
			language: "de".into(),
			..Default::default()
		};
		assert_eq!(expand_key("C", &config), "C-Dur");
		assert_eq!(expand_key("c", &config), "c-Moll");
		assert_eq!(expand_key("Bb", &config), "B-Dur");
	}

	#[test]
	fn test_format_form() {
		assert_eq!(format_form("sonata"), "Sonata");
		assert_eq!(format_form("character piece"), "Character Piece");
		assert_eq!(format_form("FUGUE"), "Fugue");
	}

	#[test]
	fn test_truncate_instrumentation() {
		assert_eq!(truncate_instrumentation("piano", 10), "piano");
		assert_eq!(truncate_instrumentation("violin, viola, and cello", 15), "violin, viola,…");
	}

	#[test]
	fn test_expand_key_already_expanded() {
		let config = DisplayConfig::default();
		assert_eq!(expand_key("D minor", &config), "D minor");
		assert_eq!(expand_key("B minor", &config), "B minor");
		assert_eq!(expand_key("G major", &config), "G major");
		assert_eq!(expand_key("F-sharp minor", &config), "F-sharp minor");
	}

	#[test]
	fn test_format_catalog_simple() {
		assert_eq!(format_catalog("bwv", "812", None), "BWV 812");
		assert_eq!(format_catalog("op", "27", None), "op. 27");
	}

	#[test]
	fn test_format_catalog_with_subnumber() {
		assert_eq!(format_catalog("op", "10/2", None), "op. 10 no. 2");
		assert_eq!(format_catalog("op", "2/1", None), "op. 2 no. 1");
	}
}

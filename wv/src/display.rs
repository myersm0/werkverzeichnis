use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::catalog::cached_regex;
use crate::config::{DisplayConfig, KeySymbols};
use crate::types::{CatalogDefinition, Collection, Composition};

#[derive(Debug, Deserialize)]
struct KeyLanguageProfile {
	major: String,
	minor: String,
	mode: String,
	#[serde(default)]
	modes: HashMap<String, String>,
	#[serde(default)]
	notes: HashMap<String, String>,
}

fn key_language_profiles() -> &'static HashMap<String, KeyLanguageProfile> {
	static PROFILES: OnceLock<HashMap<String, KeyLanguageProfile>> = OnceLock::new();
	PROFILES.get_or_init(|| {
		toml::from_str(include_str!("../key-languages.toml"))
			.expect("bundled key-language profiles must be valid TOML")
	})
}

fn key_language_profile(language: &str) -> &'static KeyLanguageProfile {
	let profiles = key_language_profiles();
	profiles
		.get(language)
		.or_else(|| profiles.get("en"))
		.expect("bundled key-language profiles must define 'en'")
}

pub fn expand_key(code: &str, config: &DisplayConfig) -> String {
	if let Some(expanded) = config.keys.get(code) {
		return expanded.clone();
	}

	let Some((is_minor, note, accidental, mode_suffix)) = parse_key_code(code) else {
		return code.to_string();
	};
	let profile = key_language_profile(&config.language);
	let canonical_note = format!("{}{}", note, accidental);
	let note_str = profile
		.notes
		.get(&canonical_note)
		.cloned()
		.unwrap_or_else(|| match config.key_symbols {
			KeySymbols::Unicode => format_note_unicode(&note, &accidental),
			KeySymbols::Ascii => format_note_ascii(&note, &accidental),
		});

	if let Some(mode_suffix) = mode_suffix {
		let Some(mode) = profile.modes.get(&mode_suffix) else {
			return code.to_string();
		};
		return apply_key_template(&profile.mode, &note_str, Some(mode));
	}

	let template = if is_minor { &profile.minor } else { &profile.major };
	apply_key_template(template, &note_str, None)
}

fn apply_key_template(template: &str, note: &str, mode: Option<&str>) -> String {
	let mut result = template
		.replace("{note}", note)
		.replace("{note_lower}", &note.to_lowercase());
	if let Some(mode) = mode {
		result = result.replace("{mode}", mode);
	}
	result
}

fn parse_key_code(code: &str) -> Option<(bool, String, String, Option<String>)> {
	let code = code.trim();
	let (main, mode) = match code.split_once('.') {
		Some((main, mode)) if !mode.is_empty() && !mode.contains('.') => {
			(main, Some(mode.to_lowercase()))
		}
		Some(_) => return None,
		None => (code, None),
	};

	let mut chars = main.chars();
	let first = chars.next()?;
	if !matches!(first.to_ascii_uppercase(), 'A'..='G') {
		return None;
	}
	let accidental = chars.collect::<String>();
	let accidental = match accidental.as_str() {
		"" | "#" | "b" | "bb" | "##" | "x" => accidental,
		"X" => "x".to_string(),
		_ => return None,
	};

	Some((
		first.is_ascii_lowercase(),
		first.to_ascii_uppercase().to_string(),
		accidental,
		mode,
	))
}

fn format_note_unicode(note: &str, accidental: &str) -> String {
	let acc = match accidental {
		"#" => "♯",
		"b" => "♭",
		"bb" => "𝄫",
		"##" | "x" => "𝄪",
		_ => "",
	};
	format!("{}{}", note, acc)
}

fn format_note_ascii(note: &str, accidental: &str) -> String {
	let acc = match accidental {
		"#" => "#",
		"b" => "b",
		"bb" => "bb",
		"##" | "x" => "##",
		_ => "",
	};
	format!("{}{}", note, acc)
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

fn apply_display_transform(s: &str, transform: &str) -> String {
	match transform {
		"upper" => s.to_uppercase(),
		"lower" => s.to_lowercase(),
		"title" => {
			let mut chars = s.chars();
			match chars.next() {
				None => String::new(),
				Some(first) => first.to_uppercase().chain(chars).collect(),
			}
		}
		_ => s.to_string(),
	}
}

pub fn format_number_for_display(number: &str, defn: Option<&CatalogDefinition>) -> String {
	let defn = match defn {
		Some(d) => d,
		None => return number.to_string(),
	};

	let pattern = match &defn.pattern {
		Some(p) => p,
		None => return number.to_string(),
	};

	let sort_keys = match &defn.sort_keys {
		Some(sks) => sks,
		None => return number.to_string(),
	};

	let Some(re) = cached_regex(pattern) else {
		return number.to_string();
	};

	let caps = match re.captures(number) {
		Some(c) => c,
		None => return number.to_string(),
	};

	let mut transforms: Vec<(usize, usize, &str)> = Vec::new();

	for sk in sort_keys {
		if let Some(display) = &sk.display {
			if let Some(m) = caps.get(sk.group) {
				transforms.push((m.start(), m.end(), display.as_str()));
			}
		}
	}

	if transforms.is_empty() {
		return number.to_string();
	}

	transforms.sort_by_key(|(start, _, _)| *start);

	let mut result = String::new();
	let mut pos = 0;

	for (start, end, transform) in transforms {
		if start > pos {
			result.push_str(&number[pos..start]);
		}
		result.push_str(&apply_display_transform(&number[start..end], transform));
		pos = end;
	}

	if pos < number.len() {
		result.push_str(&number[pos..]);
	}

	result
}

pub fn format_catalog(scheme: &str, number: &str, defn: Option<&CatalogDefinition>) -> String {
	let display_number = format_number_for_display(number, defn);
	let display_number = match (
		display_number.split_once('/'),
		defn.and_then(|definition| definition.part_format.as_deref()),
	) {
		(Some((main, part)), Some(part_format)) => part_format
			.replace("{main}", main)
			.replace("{part}", part),
		_ => display_number,
	};

	let format = defn
		.and_then(|definition| definition.canonical_format.as_deref())
		.map(str::to_string)
		.unwrap_or_else(|| format!("{} {{number}}", scheme.to_uppercase()));

	format.replace("{number}", &display_number)
}

pub fn truncate_instrumentation(inst: &str, max_chars: usize) -> String {
	if inst.chars().count() <= max_chars {
		return inst.to_string();
	}
	let kept: String = inst.chars().take(max_chars.saturating_sub(1)).collect();
	format!("{}…", kept)
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

	let pattern = if comp.key.is_none() {
		&config.patterns.generic_no_key
	} else if ctx.position_in_collection.is_some() {
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
	fn expand_title_without_key_uses_no_key_pattern() {
		let comp = Composition {
			id: "12345678".into(),
			title: None,
			form: "variations".into(),
			key: None,
			instrumentation: None,
			note: None,
			attribution: Vec::new(),
			movements: None,
			sections: None,
			xref: None,
		};
		let config = DisplayConfig::default();
		let ctx = ExpansionContext {
			composition: &comp,
			collection: None,
			position_in_collection: None,
			config: &config,
		};

		assert_eq!(expand_title(&ctx), "Variations");
	}

	#[test]
	fn truncate_instrumentation_counts_characters_not_bytes() {
		let inst = "violoncello e cembalo obbligato";
		assert_eq!(truncate_instrumentation(inst, 100), inst);
		assert_eq!(truncate_instrumentation("abcdef", 4), "abc…");
		assert_eq!(truncate_instrumentation("abcdef", 6), "abcdef");
	}

	#[test]
	fn truncate_instrumentation_does_not_split_multibyte_characters() {
		// each of these is multiple bytes but one char
		let inst = "flûte à bec, viole de gambe, clavecin, théorbe";
		assert!(inst.len() > inst.chars().count());
		let truncated = truncate_instrumentation(inst, 10);
		assert_eq!(truncated.chars().count(), 10);
		assert!(truncated.ends_with('…'));
	}

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
		assert_eq!(expand_key("F#", &config), "Fis-Dur");
		assert_eq!(expand_key("eb", &config), "es-Moll");
	}

	#[test]
	fn test_expand_key_modes_use_language_profile() {
		let english = DisplayConfig::default();
		assert_eq!(expand_key("e.phr", &english), "E Phrygian");

		let german = DisplayConfig {
			language: "de".into(),
			..Default::default()
		};
		assert_eq!(expand_key("e.phr", &german), "E-Phrygisch");
	}

	#[test]
	fn test_expand_key_ascii_symbols() {
		let config = DisplayConfig {
			key_symbols: KeySymbols::Ascii,
			..Default::default()
		};
		assert_eq!(expand_key("F#", &config), "F# major");
		assert_eq!(expand_key("bb", &config), "bb minor");
	}

	#[test]
	fn test_expand_key_unknown_language_falls_back_to_english_profile() {
		let config = DisplayConfig {
			language: "xx".into(),
			..Default::default()
		};
		assert_eq!(expand_key("F#", &config), "F♯ major");
		assert_eq!(expand_key("d.dor", &config), "D Dorian");
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
	fn test_format_catalog_uses_definition() {
		let defn = CatalogDefinition {
			name: "Opus".into(),
			canonical_format: Some("op. {number}".into()),
			part_format: Some("{main} no. {part}".into()),
			..Default::default()
		};
		assert_eq!(format_catalog("op", "27", Some(&defn)), "op. 27");
		assert_eq!(format_catalog("op", "10/2", Some(&defn)), "op. 10 no. 2");
	}

	#[test]
	fn test_format_catalog_does_not_assign_slash_semantics_without_metadata() {
		let defn = CatalogDefinition {
			name: "Example".into(),
			canonical_format: Some("Ex. {number}".into()),
			..Default::default()
		};
		assert_eq!(format_catalog("ex", "10/2", Some(&defn)), "Ex. 10/2");
		assert_eq!(format_catalog("ex", "10/2", None), "EX 10/2");
	}

	#[test]
	fn test_format_number_for_display() {
		use crate::types::{CatalogDefinition, SortKey};

		let hob_defn = CatalogDefinition {
			name: "Hoboken".into(),
			description: None,
			canonical_format: Some("Hob. {number}".into()),
			pattern: Some(r"^([ivxlcdm]+):(\d+)$".into()),
			sort_keys: Some(vec![
				SortKey { group: 1, sort_type: "roman".into(), display: Some("upper".into()), none_last: None },
				SortKey { group: 2, sort_type: "int".into(), display: None, none_last: None },
			]),
			group_by: None,
			aliases: None,
			editions: None,
			..Default::default()
		};

		assert_eq!(format_number_for_display("i:1", Some(&hob_defn)), "I:1");
		assert_eq!(format_number_for_display("xvi:52", Some(&hob_defn)), "XVI:52");
		assert_eq!(format_number_for_display("300k", None), "300k");
	}

	#[test]
	fn test_format_number_bwv_anhang() {
		use crate::types::{CatalogDefinition, SortKey};

		let bwv_defn = CatalogDefinition {
			name: "BWV".into(),
			description: None,
			canonical_format: Some("BWV {number}".into()),
			pattern: Some(r"^(anh\.|app\.)?(\s*)([ivxlcdm]+|[a-d])?(\s*)(\d+)(?:\.(\d+))?([a-z]|r)?$".into()),
			sort_keys: Some(vec![
				SortKey { group: 1, sort_type: "str".into(), display: Some("title".into()), none_last: None },
				SortKey { group: 3, sort_type: "roman".into(), display: Some("upper".into()), none_last: None },
				SortKey { group: 5, sort_type: "int".into(), display: None, none_last: None },
				SortKey { group: 6, sort_type: "int".into(), display: None, none_last: None },
				SortKey { group: 7, sort_type: "str".into(), display: None, none_last: None },
			]),
			group_by: None,
			aliases: None,
			editions: None,
			..Default::default()
		};

		assert_eq!(format_number_for_display("anh. iii 141", Some(&bwv_defn)), "Anh. III 141");
		assert_eq!(format_number_for_display("anh. ii 23", Some(&bwv_defn)), "Anh. II 23");
		assert_eq!(format_number_for_display("812", Some(&bwv_defn)), "812");
		assert_eq!(format_number_for_display("1080.1", Some(&bwv_defn)), "1080.1");
	}

	#[test]
	fn test_format_catalog_hoboken() {
		use crate::types::{CatalogDefinition, SortKey};

		let hob_defn = CatalogDefinition {
			name: "Hoboken".into(),
			description: None,
			canonical_format: Some("Hob. {number}".into()),
			pattern: Some(r"^([ivxlcdm]+):(\d+)$".into()),
			sort_keys: Some(vec![
				SortKey { group: 1, sort_type: "roman".into(), display: Some("upper".into()), none_last: None },
				SortKey { group: 2, sort_type: "int".into(), display: None, none_last: None },
			]),
			group_by: None,
			aliases: None,
			editions: None,
			..Default::default()
		};

		assert_eq!(format_catalog("hob", "i:1", Some(&hob_defn)), "Hob. I:1");
		assert_eq!(format_catalog("hob", "xvi:52", Some(&hob_defn)), "Hob. XVI:52");
	}
}

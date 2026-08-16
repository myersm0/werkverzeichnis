//! Integration tests for werkverzeichnis
//!
//! These tests create temporary data directories with sample compositions
//! and verify end-to-end behavior of indexing and querying.

use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

use werkverzeichnis::{
	build_index, load_edition_index, normalize_catalog_number, write_edition_indexes,
	write_index,
};

fn setup_test_repo() -> TempDir {
	let tmp = TempDir::new().unwrap();
	let root = tmp.path();

	// Create directory structure
	fs::create_dir_all(root.join("compositions/ab")).unwrap();
	fs::create_dir_all(root.join("compositions/cd")).unwrap();
	fs::create_dir_all(root.join("compositions/ef")).unwrap();
	fs::create_dir_all(root.join("collections/bach")).unwrap();
	fs::create_dir_all(root.join("composers")).unwrap();
	fs::create_dir_all(root.join("catalogs")).unwrap();
	fs::create_dir_all(root.join("inventories")).unwrap();
	fs::create_dir_all(root.join("schemas")).unwrap();
	fs::create_dir_all(root.join(".indexes/editions")).unwrap();

	tmp
}

fn write_composition(root: &std::path::Path, id: &str, json: &str) {
	let prefix = &id[..2];
	let suffix = &id[2..];
	let path = root.join("compositions").join(prefix).join(format!("{}.json", suffix));
	fs::write(path, json).unwrap();
}

fn write_collection(root: &std::path::Path, composer: &str, name: &str, json: &str) {
	let path = root.join("collections").join(composer).join(format!("{}.json", name));
	fs::write(path, json).unwrap();
}

fn run_wv(root: &std::path::Path, args: &[&str]) -> Output {
	let home = root.join("test-home");
	let config_home = root.join("test-config");
	fs::create_dir_all(&home).unwrap();
	fs::create_dir_all(&config_home).unwrap();
	let mut command = Command::new(env!("CARGO_BIN_EXE_wv"));
	command.args(args);
	command.arg("--data-dir").arg(root);
	command.env("HOME", home);
	command.env("XDG_CONFIG_HOME", config_home);
	command.output().unwrap()
}

fn setup_inventory_cli_repo() -> TempDir {
	let tmp = setup_test_repo();
	let root = tmp.path();
	fs::create_dir_all(root.join("inventories/beethoven")).unwrap();

	fs::write(
		root.join("catalogs/op.json"),
		r#"{
			"id": "op",
			"name": "Opus number",
			"canonical_format": "op. {number}",
			"pattern": "^(\\d+)(?:/(\\d+))?$",
			"sort_keys": [
				{"group": 1, "type": "int"},
				{"group": 2, "type": "int"}
			],
			"group_by": [1],
			"constraints": [
				{"group": 2, "name": "sub-number", "min": 1}
			]
		}"#,
	)
	.unwrap();

	fs::write(
		root.join("composers/beethoven.json"),
		r#"{
			"id": "beethoven",
			"name": {"full": "Ludwig van Beethoven", "sort": "Beethoven, Ludwig van"},
			"default_scheme": "op",
			"catalogs": {
				"op": {
					"name": "Opus",
					"canonical_format": "op. {number}",
					"constraints": [
						{"group": 1, "name": "opus number", "min": 1, "max": 138}
					]
				}
			}
		}"#,
	)
	.unwrap();

	fs::write(
		root.join("inventories/beethoven/op.toml"),
		r#"composer = "beethoven"
scheme = "op"
complete = true
entries = ["2/1", "2/2", "2/3", "138"]
"#,
	)
	.unwrap();

	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "sonata",
		"key": "C",
		"attribution": [{
			"composer": "beethoven",
			"catalog": [{"scheme": "op", "number": "2/3"}]
		}]
	}"#);

	tmp
}

#[test]
fn test_get_fails_on_corrupt_dataset_composition() {
	let tmp = setup_test_repo();
	let root = tmp.path();
	fs::write(root.join("compositions/ab/123456.json"), "{").unwrap();

	let output = run_wv(root, &["get", "beethoven"]);
	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("Error loading dataset"));
	assert!(stderr.contains("123456.json"));
}

#[test]
fn test_get_by_id_fails_on_corrupt_existing_composition() {
	let tmp = setup_test_repo();
	let root = tmp.path();
	fs::write(root.join("compositions/ab/123456.json"), "{").unwrap();

	let output = run_wv(root, &["get", "ab123456"]);
	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("Error loading composition"));
	assert!(stderr.contains("123456.json"));
}

#[test]
fn test_index_fails_on_invalid_inventory() {
	let tmp = setup_test_repo();
	let root = tmp.path();
	fs::create_dir_all(root.join("inventories/beethoven")).unwrap();
	fs::write(root.join("inventories/beethoven/op.toml"), "not = [valid").unwrap();

	let output = run_wv(root, &["index"]);
	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("Error building index"));
	assert!(stderr.contains("op.toml"));
}

#[test]
fn test_get_fails_on_invalid_catalog_metadata() {
	let tmp = setup_test_repo();
	let root = tmp.path();
	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "sonata",
		"attribution": [{
			"composer": "beethoven",
			"catalog": [{"scheme": "op", "number": "2/1"}]
		}]
	}"#);
	fs::write(root.join("catalogs/op.json"), "{").unwrap();

	let output = run_wv(root, &["get", "beethoven", "op"]);
	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("Error loading catalog metadata"));
	assert!(stderr.contains("op.json"));
}

#[test]
fn test_broad_get_fails_on_invalid_composer_metadata() {
	let tmp = setup_test_repo();
	let root = tmp.path();
	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "sonata",
		"attribution": [{
			"composer": "beethoven",
			"catalog": [{"scheme": "op", "number": "2/1"}]
		}]
	}"#);
	fs::write(root.join("composers/beethoven.json"), "{").unwrap();

	let output = run_wv(root, &["get", "beethoven"]);
	assert!(!output.status.success());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("Error querying dataset"));
	assert!(stderr.contains("beethoven.json"));
}

#[test]
fn test_single_json_result_is_array() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "suite",
		"attribution": []
	}"#);

	let output = run_wv(root, &["get", "ab123456", "--json"]);
	assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
	let results = value.as_array().expect("--json output should always be an array");
	assert_eq!(results.len(), 1);
	assert_eq!(results[0]["id"], "ab123456");
}

#[test]
fn test_inventory_only_json_result_is_array() {
	let tmp = setup_inventory_cli_repo();
	let root = tmp.path();

	let output = run_wv(root, &["get", "beethoven", "op", "138", "--json"]);
	assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
	let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
	let results = value.as_array().expect("--json output should always be an array");
	assert_eq!(results.len(), 1);
	assert_eq!(results[0]["number"], "138");
	assert_eq!(results[0]["populated"], false);
}

#[test]
fn test_index_status_uses_stderr() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	let output = run_wv(root, &["index"]);
	assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
	assert!(output.stdout.is_empty());
	let stderr = String::from_utf8_lossy(&output.stderr);
	assert!(stderr.contains("Building index"));
	assert!(stderr.contains("Done."));
}

#[test]
fn test_hoboken_category_query_does_not_match_longer_roman_category() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	fs::write(
		root.join("composers/haydn.json"),
		r#"{
			"id": "haydn",
			"name": {"full": "Franz Joseph Haydn", "sort": "Haydn, Joseph"},
			"catalogs": {
				"hob": {
					"name": "Hoboken-Verzeichnis",
					"pattern": "^([ivxlcdm]+):(\\d+)$",
					"sort_keys": [
						{"group": 1, "type": "roman"},
						{"group": 2, "type": "int"}
					],
					"categories": {"I": "symphonies", "III": "string quartets"}
				}
			}
		}"#,
	)
	.unwrap();

	write_composition(root, "ab000001", r#"{
		"id": "ab000001",
		"form": "symphony",
		"attribution": [{
			"composer": "haydn",
			"catalog": [{"scheme": "hob", "number": "i:104"}]
		}]
	}"#);
	write_composition(root, "cd000002", r#"{
		"id": "cd000002",
		"form": "string quartet",
		"attribution": [{
			"composer": "haydn",
			"catalog": [{"scheme": "hob", "number": "iii:31"}]
		}]
	}"#);

	let output = run_wv(root, &["get", "haydn", "hob", "i", "--terse"]);
	assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
	assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "ab000001");
}

#[test]
fn test_broad_get_queries_are_sorted_by_catalog_order() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	fs::write(
		root.join("composers/beethoven.json"),
		r#"{
			"id": "beethoven",
			"name": {"full": "Ludwig van Beethoven", "sort": "Beethoven, Ludwig van"},
			"default_scheme": "op",
			"catalogs": {
				"op": {
					"name": "Opus",
					"pattern": "^(\\d+)$",
					"sort_keys": [{"group": 1, "type": "int"}],
					"primary": true
				},
				"woo": {
					"name": "WoO",
					"pattern": "^(\\d+)$",
					"sort_keys": [{"group": 1, "type": "int"}]
				}
			}
		}"#,
	)
	.unwrap();

	write_composition(root, "ab000010", r#"{
		"id": "ab000010",
		"form": "sonata",
		"attribution": [{
			"composer": "beethoven",
			"catalog": [{"scheme": "op", "number": "10"}]
		}]
	}"#);
	write_composition(root, "cd000002", r#"{
		"id": "cd000002",
		"form": "sonata",
		"attribution": [{
			"composer": "beethoven",
			"catalog": [{"scheme": "op", "number": "2"}]
		}]
	}"#);
	write_composition(root, "ef000001", r#"{
		"id": "ef000001",
		"form": "piece",
		"attribution": [{
			"composer": "beethoven",
			"catalog": [{"scheme": "woo", "number": "1"}]
		}]
	}"#);

	let output = run_wv(root, &["get", "beethoven", "op", "--terse"]);
	assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
	assert_eq!(
		String::from_utf8_lossy(&output.stdout).trim(),
		"cd000002\nab000010"
	);

	let output = run_wv(root, &["get", "beethoven", "--terse"]);
	assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
	assert_eq!(
		String::from_utf8_lossy(&output.stdout).trim(),
		"cd000002\nab000010\nef000001"
	);
}

// ============================================================================
// Index round-trip tests
// ============================================================================

#[test]
fn test_index_roundtrip() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "sonata",
		"key": "C",
		"attribution": [{
			"composer": "mozart",
			"catalog": [{"scheme": "k", "number": "545"}]
		}]
	}"#);

	write_composition(root, "cd789012", r#"{
		"id": "cd789012",
		"form": "sonata",
		"key": "a",
		"attribution": [{
			"composer": "mozart",
			"catalog": [{"scheme": "k", "number": "331"}]
		}]
	}"#);

	let index = build_index(root).unwrap();

	// Verify lookups work
	let result = index.query().composer("mozart").scheme("k").number("545").fetch_one().unwrap();
	assert_eq!(result, Some("ab123456".to_string()));

	let result = index.query().composer("mozart").scheme("k").number("331").fetch_one().unwrap();
	assert_eq!(result, Some("cd789012".to_string()));

	// Verify composer index
	let mozart_works = index.by_composer.get("mozart").unwrap();
	assert_eq!(mozart_works.len(), 2);
}

#[test]
fn test_index_persists_and_reloads() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "suite",
		"attribution": [{
			"composer": "bach",
			"catalog": [{"scheme": "bwv", "number": "812"}]
		}]
	}"#);

	let index = build_index(root).unwrap();
	let index_path = root.join(".indexes").join("index.json");
	let composer_path = root.join(".indexes").join("composer-index.json");
	write_index(&index, &index_path).unwrap();
	werkverzeichnis::write_composer_index(&index, &composer_path).unwrap();

	let loaded = werkverzeichnis::load_index(root).unwrap();

	let result = loaded.query().composer("bach").scheme("bwv").number("812").fetch_one().unwrap();
	assert_eq!(result, Some("ab123456".to_string()));
}

// ============================================================================
// Cumulative edition tests
// ============================================================================

#[test]
fn test_cumulative_editions() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	// K. 545 - exists since edition 1, unchanged
	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "sonata",
		"attribution": [{
			"composer": "mozart",
			"catalog": [{"scheme": "k", "number": "545", "edition": "1"}]
		}]
	}"#);

	// K. 331/300i - renumbered in edition 9
	// Old number (edition 1)
	write_composition(root, "cd789012", r#"{
		"id": "cd789012",
		"form": "sonata",
		"attribution": [{
			"composer": "mozart",
			"catalog": [
				{"scheme": "k", "number": "331", "edition": "9"},
				{"scheme": "k", "number": "300i", "edition": "1"}
			]
		}]
	}"#);

	let index = build_index(root).unwrap();
	write_edition_indexes(&index, root).unwrap();

	// Edition 1 should have: 545, 300i (not 331)
	let ed1 = load_edition_index(root, "mozart", "k", "1").unwrap().unwrap();
	assert!(ed1.contains_key("545"));
	assert!(ed1.contains_key("300i"));
	assert!(!ed1.contains_key("331"));

	// Edition 9 should have: 545 (inherited), 331 (not 300i)
	let ed9 = load_edition_index(root, "mozart", "k", "9").unwrap().unwrap();
	assert!(ed9.contains_key("545"), "545 should be inherited into edition 9");
	assert!(ed9.contains_key("331"), "331 should be in edition 9");
	assert!(!ed9.contains_key("300i"), "300i should be superseded by 331 in edition 9");
}

#[test]
fn test_corrupt_edition_index_is_an_error_but_missing_is_not() {
	let tmp = setup_test_repo();
	let root = tmp.path();
	let index = build_index(root).unwrap();

	fs::write(root.join(".indexes/editions/mozart-k-1.json"), "{").unwrap();
	let error = index
		.query()
		.composer("mozart")
		.scheme("k")
		.edition("1")
		.number("545")
		.data_dir(root)
		.fetch_one()
		.unwrap_err();
	assert!(error.to_string().contains("invalid edition index"));

	let missing = index
		.query()
		.composer("mozart")
		.scheme("k")
		.edition("2")
		.number("545")
		.data_dir(root)
		.fetch_one()
		.unwrap();
	assert_eq!(missing, None);
}

#[test]
fn test_index_command_writes_edition_indexes_at_expected_path() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "sonata",
		"attribution": [{
			"composer": "mozart",
			"catalog": [{"scheme": "k", "number": "545", "edition": "1"}]
		}]
	}"#);

	werkverzeichnis::commands::index::run(root);

	assert!(root.join(".indexes/editions/mozart-k-1.json").exists());
	assert!(!root.join(".indexes/editions/.indexes/editions").exists());
}

// ============================================================================
// Case normalization tests
// ============================================================================

#[test]
fn test_normalize_catalog_number() {
	assert_eq!(normalize_catalog_number("BWV 812"), "bwv 812");
	assert_eq!(normalize_catalog_number("K. 331"), "k. 331");
	assert_eq!(normalize_catalog_number("Hob. I:104"), "hob. i:104");
	assert_eq!(normalize_catalog_number("Op. 2/1"), "op. 2/1");
	assert_eq!(normalize_catalog_number("ANH. III 141"), "anh. iii 141");
}

#[test]
fn test_case_insensitive_query() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "symphony",
		"attribution": [{
			"composer": "haydn",
			"catalog": [{"scheme": "hob", "number": "i:104"}]
		}]
	}"#);

	let index = build_index(root).unwrap();

	// Query with various case combinations - number is normalized by library
	let result = index.query().composer("haydn").scheme("hob").number("i:104").fetch_one().unwrap();
	assert_eq!(result, Some("ab123456".to_string()));

	// Uppercase number gets normalized
	let result = index.query().composer("haydn").scheme("hob").number("I:104").fetch_one().unwrap();
	assert_eq!(result, Some("ab123456".to_string()));

	// Composer/scheme normalization happens at CLI layer, so these need lowercase
	// This matches real usage: wv.rs does .to_lowercase() before calling query
	let result = index.query().composer("haydn").scheme("hob").number("I:104").fetch_one().unwrap();
	assert_eq!(result, Some("ab123456".to_string()));
}

// ============================================================================
// Superseded catalog number tests
// ============================================================================

#[test]
fn test_superseded_lookup() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "sonata",
		"attribution": [{
			"composer": "mozart",
			"catalog": [
				{"scheme": "k", "number": "331"},
				{"scheme": "k", "number": "300i"}
			]
		}]
	}"#);

	let index = build_index(root).unwrap();

	// Current number works
	let result = index.query().composer("mozart").scheme("k").number("331").fetch_one().unwrap();
	assert_eq!(result, Some("ab123456".to_string()));

	// Superseded number also works (non-strict mode)
	let result = index.query().composer("mozart").scheme("k").number("300i").fetch_one().unwrap();
	assert_eq!(result, Some("ab123456".to_string()));

	// Strict mode rejects superseded
	let result = index.query().composer("mozart").scheme("k").number("300i").strict(true).fetch_one().unwrap();
	assert_eq!(result, None);
}

#[test]
fn test_superseded_has_current_number() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "sonata",
		"attribution": [{
			"composer": "mozart",
			"catalog": [
				{"scheme": "k", "number": "331"},
				{"scheme": "k", "number": "300i"}
			]
		}]
	}"#);

	let index = build_index(root).unwrap();

	let results = index
		.query()
		.composer("mozart")
		.scheme("k")
		.number("300i")
		.data_dir(root)
		.fetch().unwrap();

	assert_eq!(results.len(), 1);
	assert!(results[0].superseded);
	assert_eq!(results[0].current_number, Some("331".to_string()));
}

// ============================================================================
// Multi-composer attribution tests
// ============================================================================

#[test]
fn test_multi_composer_attribution() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	// Piece attributed to both Telemann (current) and Bach (historical)
	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "cantata",
		"attribution": [
			{
				"composer": "telemann",
				"catalog": [{"scheme": "twv", "number": "1:183"}]
			},
			{
				"composer": "bach",
				"catalog": [
					{"scheme": "bwv", "number": "anh. iii 141"},
					{"scheme": "bwv", "number": "141"}
				]
			}
		]
	}"#);

	let index = build_index(root).unwrap();

	// Telemann lookup
	let result = index.query().composer("telemann").scheme("twv").number("1:183").fetch_one().unwrap();
	assert_eq!(result, Some("ab123456".to_string()));

	// Bach current (Anhang)
	let result = index.query().composer("bach").scheme("bwv").number("anh. iii 141").fetch_one().unwrap();
	assert_eq!(result, Some("ab123456".to_string()));

	// Bach superseded
	let result = index.query().composer("bach").scheme("bwv").number("141").fetch_one().unwrap();
	assert_eq!(result, Some("ab123456".to_string()));

	// Bach superseded in strict mode
	let result = index.query().composer("bach").scheme("bwv").number("141").strict(true).fetch_one().unwrap();
	assert_eq!(result, None);
}

// ============================================================================
// Collection membership tests
// ============================================================================

#[test]
fn test_collection_membership() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	// Collection lists compositions
	write_collection(root, "bach", "wtc-1", r#"{
		"id": "bach-wtc-1",
		"title": {"en": "Well-Tempered Clavier, Book 1"},
		"attribution": [{"composer": "bach"}],
		"scheme": "bwv",
		"compositions": ["846", "847"]
	}"#);

	// Composition has explicit attribution (no cf needed)
	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "prelude and fugue",
		"key": "C",
		"attribution": [{
			"composer": "bach",
			"catalog": [{"scheme": "bwv", "number": "846"}]
		}]
	}"#);

	let index = build_index(root).unwrap();

	// Should be indexed under bach
	let result = index.query().composer("bach").scheme("bwv").number("846").fetch_one().unwrap();
	assert_eq!(result, Some("ab123456".to_string()));

	// Should appear in composer index
	let bach_works = index.by_composer.get("bach");
	assert!(bach_works.is_some());
	assert!(bach_works.unwrap().contains(&"ab123456".to_string()));
}

// ============================================================================
// Note field tests
// ============================================================================

#[test]
fn test_note_in_index() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "cantata",
		"attribution": [{
			"composer": "bach",
			"catalog": [{
				"scheme": "bwv",
				"number": "anh. iii 141",
				"note": "spurious; now attributed to Telemann"
			}]
		}]
	}"#);

	let index = build_index(root).unwrap();

	let results = index
		.query()
		.composer("bach")
		.scheme("bwv")
		.number("anh. iii 141")
		.data_dir(root)
		.fetch().unwrap();

	assert_eq!(results.len(), 1);
	assert_eq!(results[0].note, Some("spurious; now attributed to Telemann".to_string()));
}

// ============================================================================
// Range query tests
// ============================================================================

#[test]
fn test_range_query() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	for (i, id) in ["ab000001", "ab000002", "ab000003", "ab000004", "ab000005"].iter().enumerate() {
		let num = i + 1;
		write_composition(root, id, &format!(r#"{{
			"id": "{}",
			"form": "symphony",
			"attribution": [{{
				"composer": "haydn",
				"catalog": [{{"scheme": "hob", "number": "i:{}"}}]
			}}]
		}}"#, id, num));
	}

	let index = build_index(root).unwrap();

	let results = index
		.query()
		.composer("haydn")
		.scheme("hob")
		.range("i:2", "i:4")
		.data_dir(root)
		.sorted(root)
		.fetch().unwrap();

	assert_eq!(results.len(), 3);
}

// ============================================================================
// Group query tests
// ============================================================================

#[test]
fn test_group_query() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	write_composition(root, "ab000001", r#"{
		"id": "ab000001",
		"form": "sonata",
		"attribution": [{
			"composer": "beethoven",
			"catalog": [{"scheme": "op", "number": "2/1"}]
		}]
	}"#);

	write_composition(root, "ab000002", r#"{
		"id": "ab000002",
		"form": "sonata",
		"attribution": [{
			"composer": "beethoven",
			"catalog": [{"scheme": "op", "number": "2/2"}]
		}]
	}"#);

	write_composition(root, "ab000003", r#"{
		"id": "ab000003",
		"form": "sonata",
		"attribution": [{
			"composer": "beethoven",
			"catalog": [{"scheme": "op", "number": "2/3"}]
		}]
	}"#);

	write_composition(root, "ab000004", r#"{
		"id": "ab000004",
		"form": "sonata",
		"attribution": [{
			"composer": "beethoven",
			"catalog": [{"scheme": "op", "number": "7"}]
		}]
	}"#);

	let index = build_index(root).unwrap();

	let results = index
		.query()
		.composer("beethoven")
		.scheme("op")
		.group("2")
		.data_dir(root)
		.sorted(root)
		.fetch().unwrap();

	// Group "2" should match 2/1, 2/2, 2/3 but not 7
	assert_eq!(results.len(), 3);

	let numbers: Vec<_> = results.iter().filter_map(|r| r.number.as_ref()).collect();
	assert!(numbers.contains(&&"2/1".to_string()));
	assert!(numbers.contains(&&"2/2".to_string()));
	assert!(numbers.contains(&&"2/3".to_string()));
}

// ============================================================================
// Köchel edition mapping tests (based on real catalog data)
// ============================================================================

#[test]
fn test_kochel_331_edition_mapping() {
	// K. 331 (Alla Turca sonata) - famously renumbered
	// Edition 1 (1862): K. 331
	// Edition 6 (1964): K. 300i
	// Edition 9 (2024): K. 331 (reverted)
	
	let tmp = setup_test_repo();
	let root = tmp.path();

	write_composition(root, "ab331331", r#"{
		"id": "ab331331",
		"form": "sonata",
		"key": "A",
		"attribution": [{
			"composer": "mozart",
			"catalog": [
				{"scheme": "k", "number": "331", "edition": "9"},
				{"scheme": "k", "number": "300i", "edition": "6"},
				{"scheme": "k", "number": "331", "edition": "1"}
			]
		}]
	}"#);

	let index = build_index(root).unwrap();
	write_edition_indexes(&index, root).unwrap();

	// Current number (ed 9) should work
	let result = index.query().composer("mozart").scheme("k").number("331").fetch_one().unwrap();
	assert_eq!(result, Some("ab331331".to_string()));

	// Edition 6 number should work
	let result = index.query().composer("mozart").scheme("k").number("300i").fetch_one().unwrap();
	assert_eq!(result, Some("ab331331".to_string()));

	// Edition 1 had same number as current
	let ed1 = load_edition_index(root, "mozart", "k", "1").unwrap().unwrap();
	assert!(ed1.contains_key("331"));

	// Edition 6 should have 300i, not 331
	let ed6 = load_edition_index(root, "mozart", "k", "6").unwrap().unwrap();
	assert!(ed6.contains_key("300i"));
	assert!(!ed6.contains_key("331"));

	// Edition 9 should have 331, not 300i
	let ed9 = load_edition_index(root, "mozart", "k", "9").unwrap().unwrap();
	assert!(ed9.contains_key("331"));
	assert!(!ed9.contains_key("300i"));
}

#[test]
fn test_kochel_anh_reclassification() {
	// K. 19a was Anh. 223 in edition 1, then 19a in edition 6
	// This tests works being "promoted" from Anhang to main catalog
	
	let tmp = setup_test_repo();
	let root = tmp.path();

	write_composition(root, "ab19a19a", r#"{
		"id": "ab19a19a",
		"form": "symphony",
		"attribution": [{
			"composer": "mozart",
			"catalog": [
				{"scheme": "k", "number": "19a", "edition": "6"},
				{"scheme": "k", "number": "anh. 223", "edition": "1"}
			]
		}]
	}"#);

	let index = build_index(root).unwrap();
	write_edition_indexes(&index, root).unwrap();

	// Current number should work
	let result = index.query().composer("mozart").scheme("k").number("19a").fetch_one().unwrap();
	assert_eq!(result, Some("ab19a19a".to_string()));

	// Old Anhang number should also work (superseded)
	let result = index.query().composer("mozart").scheme("k").number("anh. 223").fetch_one().unwrap();
	assert_eq!(result, Some("ab19a19a".to_string()));

	// Edition 1 should have anh. 223
	let ed1 = load_edition_index(root, "mozart", "k", "1").unwrap().unwrap();
	assert!(ed1.contains_key("anh. 223"));
	assert!(!ed1.contains_key("19a"));

	// Edition 6 should have 19a
	let ed6 = load_edition_index(root, "mozart", "k", "6").unwrap().unwrap();
	assert!(ed6.contains_key("19a"));
	assert!(!ed6.contains_key("anh. 223"));
}


#[test]
fn test_cli_distinguishes_catalog_failure_modes() {
	let tmp = setup_inventory_cli_repo();
	let root = tmp.path();

	let output = run_wv(root, &["get", "beethoven", "op", "2/3"]);
	assert!(output.status.success());
	assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "Sonata in C major, op. 2 no. 3");

	let output = run_wv(root, &["get", "beethoven", "op", "2/4"]);
	assert!(output.status.success());
	assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "No such catalog entry: op. 2 no. 4");

	let output = run_wv(root, &["get", "beethoven", "op", "2/e"]);
	assert!(output.status.success());
	assert_eq!(
		String::from_utf8_lossy(&output.stderr).trim(),
		"Invalid catalog number for beethoven / Opus: \"2/e\""
	);

	let output = run_wv(root, &["get", "beethoven", "op", "2/0"]);
	assert!(output.status.success());
	assert_eq!(
		String::from_utf8_lossy(&output.stderr).trim(),
		"Catalog number out of range for beethoven / Opus: \"2/0\" (sub-number 0 is below the minimum 1)"
	);

	let output = run_wv(root, &["get", "beethoven", "op", "138"]);
	assert!(output.status.success());
	assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "op. 138");
	assert_eq!(
		String::from_utf8_lossy(&output.stderr).trim(),
		"catalog entry known; detailed record not yet available"
	);

	let output = run_wv(root, &["get", "beethoven", "op", "139"]);
	assert!(output.status.success());
	assert_eq!(
		String::from_utf8_lossy(&output.stderr).trim(),
		"Catalog number out of range for beethoven / Opus: \"139\" (opus number 139 is above the maximum 138)"
	);
}

#[test]
fn test_cli_coverage_uses_inventory_as_denominator() {
	let tmp = setup_inventory_cli_repo();
	let root = tmp.path();

	let output = run_wv(root, &["coverage", "beethoven", "op"]);
	assert!(output.status.success());
	assert_eq!(
		String::from_utf8_lossy(&output.stdout).trim(),
		"beethoven / op\nInventory: complete\nInventory entries: 4\nPopulated: 1\nMissing: 3\nCoverage: 25.0%"
	);

	let output = run_wv(root, &["coverage", "beethoven", "op", "--missing"]);
	assert!(output.status.success());
	assert_eq!(
		String::from_utf8_lossy(&output.stdout).trim(),
		"beethoven / op\nInventory: complete\nInventory entries: 4\nPopulated: 1\nMissing: 3\nCoverage: 25.0%\nop. 2 no. 1\nop. 2 no. 2\nop. 138"
	);
}

#[test]
fn test_cli_inventory_only_group_reports_missing_detail() {
	let tmp = setup_inventory_cli_repo();
	let root = tmp.path();
	fs::write(
		root.join("inventories/beethoven/op.toml"),
		r#"composer = "beethoven"
scheme = "op"
complete = true
entries = ["2/1", "2/2", "2/3", "9/1", "9/2", "9/3", "138"]
"#,
	)
	.unwrap();

	let output = run_wv(root, &["get", "beethoven", "op", "9"]);
	assert!(output.status.success());
	assert_eq!(
		String::from_utf8_lossy(&output.stdout).trim(),
		"op. 9 no. 1\nop. 9 no. 2\nop. 9 no. 3"
	);
	assert_eq!(
		String::from_utf8_lossy(&output.stderr).trim(),
		"3 catalog entries known; detailed records not yet available"
	);
}

#[test]
fn test_cli_partially_populated_group_overlays_inventory_members() {
	let tmp = setup_inventory_cli_repo();
	let root = tmp.path();
	fs::write(
		root.join("inventories/beethoven/op.toml"),
		r#"composer = "beethoven"
scheme = "op"
complete = true
entries = ["2/1", "2/2", "2/3", "9/1", "9/2", "9/3", "138"]
"#,
	)
	.unwrap();
	write_composition(root, "cd123456", r#"{
		"id": "cd123456",
		"form": "sonata",
		"key": "G",
		"attribution": [{
			"composer": "beethoven",
			"catalog": [{"scheme": "op", "number": "9/2"}]
		}]
	}"#);

	let output = run_wv(root, &["get", "beethoven", "op", "9"]);
	assert!(output.status.success());
	assert_eq!(
		String::from_utf8_lossy(&output.stdout).trim(),
		"op. 9 no. 1\nSonata in G major, op. 9 no. 2\nop. 9 no. 3"
	);
	assert_eq!(
		String::from_utf8_lossy(&output.stderr).trim(),
		"2 catalog entries known; detailed records not yet available"
	);
}

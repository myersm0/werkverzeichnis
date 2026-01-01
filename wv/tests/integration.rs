//! Integration tests for werkverzeichnis
//!
//! These tests create temporary data directories with sample compositions
//! and verify end-to-end behavior of indexing and querying.

use std::fs;
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

	let index = build_index(root);

	// Verify lookups work
	let result = index.query().composer("mozart").scheme("k").number("545").fetch_one();
	assert_eq!(result, Some("ab123456".to_string()));

	let result = index.query().composer("mozart").scheme("k").number("331").fetch_one();
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

	let index = build_index(root);
	let index_path = root.join(".indexes").join("index.json");
	let composer_path = root.join(".indexes").join("composer-index.json");
	write_index(&index, &index_path).unwrap();
	werkverzeichnis::write_composer_index(&index, &composer_path).unwrap();

	let loaded = werkverzeichnis::load_index(root).unwrap();

	let result = loaded.query().composer("bach").scheme("bwv").number("812").fetch_one();
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

	let index = build_index(root);
	write_edition_indexes(&index, root).unwrap();

	// Edition 1 should have: 545, 300i (not 331)
	let ed1 = load_edition_index(root, "mozart", "k", "1").unwrap();
	assert!(ed1.contains_key("545"));
	assert!(ed1.contains_key("300i"));
	assert!(!ed1.contains_key("331"));

	// Edition 9 should have: 545 (inherited), 331 (not 300i)
	let ed9 = load_edition_index(root, "mozart", "k", "9").unwrap();
	assert!(ed9.contains_key("545"), "545 should be inherited into edition 9");
	assert!(ed9.contains_key("331"), "331 should be in edition 9");
	assert!(!ed9.contains_key("300i"), "300i should be superseded by 331 in edition 9");
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

	let index = build_index(root);

	// Query with various case combinations - number is normalized by library
	let result = index.query().composer("haydn").scheme("hob").number("i:104").fetch_one();
	assert_eq!(result, Some("ab123456".to_string()));

	// Uppercase number gets normalized
	let result = index.query().composer("haydn").scheme("hob").number("I:104").fetch_one();
	assert_eq!(result, Some("ab123456".to_string()));

	// Composer/scheme normalization happens at CLI layer, so these need lowercase
	// This matches real usage: wv.rs does .to_lowercase() before calling query
	let result = index.query().composer("haydn").scheme("hob").number("I:104").fetch_one();
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

	let index = build_index(root);

	// Current number works
	let result = index.query().composer("mozart").scheme("k").number("331").fetch_one();
	assert_eq!(result, Some("ab123456".to_string()));

	// Superseded number also works (non-strict mode)
	let result = index.query().composer("mozart").scheme("k").number("300i").fetch_one();
	assert_eq!(result, Some("ab123456".to_string()));

	// Strict mode rejects superseded
	let result = index.query().composer("mozart").scheme("k").number("300i").strict(true).fetch_one();
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

	let index = build_index(root);

	let results = index
		.query()
		.composer("mozart")
		.scheme("k")
		.number("300i")
		.data_dir(root)
		.fetch();

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

	let index = build_index(root);

	// Telemann lookup
	let result = index.query().composer("telemann").scheme("twv").number("1:183").fetch_one();
	assert_eq!(result, Some("ab123456".to_string()));

	// Bach current (Anhang)
	let result = index.query().composer("bach").scheme("bwv").number("anh. iii 141").fetch_one();
	assert_eq!(result, Some("ab123456".to_string()));

	// Bach superseded
	let result = index.query().composer("bach").scheme("bwv").number("141").fetch_one();
	assert_eq!(result, Some("ab123456".to_string()));

	// Bach superseded in strict mode
	let result = index.query().composer("bach").scheme("bwv").number("141").strict(true).fetch_one();
	assert_eq!(result, None);
}

// ============================================================================
// Collection hydration tests
// ============================================================================

#[test]
fn test_collection_hydration() {
	let tmp = setup_test_repo();
	let root = tmp.path();

	// Collection defines the composer
	write_collection(root, "bach", "wtc-1", r#"{
		"id": "bach-wtc-1",
		"title": {"en": "Well-Tempered Clavier, Book 1"},
		"attribution": [{"composer": "bach"}],
		"scheme": "bwv",
		"compositions": ["846", "847"]
	}"#);

	// Composition uses cf reference instead of explicit composer
	write_composition(root, "ab123456", r#"{
		"id": "ab123456",
		"form": "prelude and fugue",
		"key": "C",
		"attribution": [{
			"cf": "bach-wtc-1",
			"catalog": [{"scheme": "bwv", "number": "846"}]
		}]
	}"#);

	let index = build_index(root);

	// Should be indexed under bach via hydration
	let result = index.query().composer("bach").scheme("bwv").number("846").fetch_one();
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

	let index = build_index(root);

	let results = index
		.query()
		.composer("bach")
		.scheme("bwv")
		.number("anh. iii 141")
		.data_dir(root)
		.fetch();

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

	let index = build_index(root);

	let results = index
		.query()
		.composer("haydn")
		.scheme("hob")
		.range("i:2", "i:4")
		.data_dir(root)
		.sorted(root)
		.fetch();

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

	let index = build_index(root);

	let results = index
		.query()
		.composer("beethoven")
		.scheme("op")
		.group("2")
		.data_dir(root)
		.sorted(root)
		.fetch();

	// Group "2" should match 2/1, 2/2, 2/3 but not 7
	assert_eq!(results.len(), 3);

	let numbers: Vec<_> = results.iter().filter_map(|r| r.number.as_ref()).collect();
	assert!(numbers.contains(&&"2/1".to_string()));
	assert!(numbers.contains(&&"2/2".to_string()));
	assert!(numbers.contains(&&"2/3".to_string()));
}

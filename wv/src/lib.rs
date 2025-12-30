pub mod add;
pub mod catalog;
pub mod commands;
pub mod config;
pub mod display;
pub mod index;
pub mod merge;
pub mod output;
pub mod parse;
pub mod query;
pub mod types;
pub mod validate;

pub use add::{add_composition, generate_id, scaffold_composition, AddError, AddResult};
pub use catalog::{
	is_fallback_key, load_catalog_def, looks_like_group, matches_group,
	normalize_catalog_number, sort_key, sort_numbers, sort_numbers_by_scheme, SortValue,
};
pub use config::{resolve_data_dir, resolve_editor, Config, DisplayConfig, KeySymbols, PatternConfig};
pub use display::{
	expand_key, expand_title, format_catalog, format_form, truncate_instrumentation,
	ExpansionContext,
};
pub use index::{
	build_index, get_or_build_index, index_is_stale, load_index, write_composer_index,
	write_edition_indexes, write_index, Index, SchemeIndex,
};
pub use merge::{
	all_catalog_entries, collection_path_from_id, current_catalog_number,
	current_catalog_number_for_edition, current_composer, merge_attribution,
	merge_attribution_with_collections, state_as_of, MergedAttribution,
};
pub use parse::{load_collection, load_composer, load_composition, ParseError};
pub use query::{QueryBuilder, QueryResult};
pub use types::*;
pub use validate::{validate_all, validate_file, ValidationError, Validator};

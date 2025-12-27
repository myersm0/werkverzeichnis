pub mod add;
pub mod catalog;
pub mod index;
pub mod merge;
pub mod parse;
pub mod query;
pub mod types;
pub mod validate;

pub use add::{add_composition, generate_id, scaffold_composition, AddError, AddResult};
pub use catalog::{load_catalog_def, matches_group, sort_key, sort_numbers, sort_numbers_by_scheme};
pub use index::{build_index, write_composer_index, write_edition_indexes, write_index, Index};
pub use merge::{
	all_catalog_entries, current_catalog_number, current_catalog_number_for_edition,
	current_composer, merge_attribution, merge_attribution_with_collections, state_as_of,
	MergedAttribution,
};
pub use parse::{load_collection, load_composer, load_composition, ParseError};
pub use query::{QueryBuilder, QueryResult};
pub use types::*;
pub use validate::{validate_all, validate_file, ValidationError, Validator};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Composition {
	pub id: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub title: Option<HashMap<String, String>>,
	pub form: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub key: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub instrumentation: Option<String>,
	pub attribution: Vec<AttributionEntry>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub movements: Option<Vec<Movement>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub sections: Option<Vec<Section>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub xref: Option<Xref>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
	Certain,
	Probable,
	Doubtful,
	Spurious,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributionEntry {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub composer: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub cf: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub dates: Option<Dates>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub status: Option<Status>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub catalog: Option<Vec<CatalogEntry>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub since: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dates {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub composed: Option<i32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub published: Option<i32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub premiered: Option<i32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub revised: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
	pub scheme: String,
	pub number: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub edition: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub since: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Movement {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub title: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub key: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub form: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub soloists: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub sections: Option<Vec<Section>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub title: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub key: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub form: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub soloists: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub movements: Option<Vec<Movement>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub sections: Option<Vec<Section>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Xref {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub oo: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mb: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub imslp: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub wp: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub wd: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub viaf: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
	pub id: String,
	pub title: HashMap<String, String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub expansion_pattern: Option<HashMap<String, String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub composer: Option<String>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub attribution: Vec<AttributionEntry>,
	pub scheme: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	pub compositions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Composer {
	pub id: String,
	pub name: ComposerName,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub default_scheme: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub born: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub died: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub nationality: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub catalogs: Option<HashMap<String, CatalogDefinition>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub xref: Option<Xref>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposerName {
	pub full: String,
	pub sort: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogDefinition {
	pub name: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub canonical_format: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub pattern: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub sort_keys: Option<Vec<SortKey>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub group_by: Option<Vec<usize>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub aliases: Option<Vec<String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub editions: Option<HashMap<String, EditionInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortKey {
	pub group: usize,
	#[serde(rename = "type")]
	pub sort_type: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub display: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditionInfo {
	pub year: i32,
	pub editor: String,
}

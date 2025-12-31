use std::path::Path;

use crate::config::Config;
use crate::display::{expand_title, format_catalog, ExpansionContext};
use crate::parse::load_composition;
use crate::query::QueryResult;
use crate::types::{CatalogDefinition, Composition};

pub fn id_to_path(data_dir: &Path, id: &str) -> std::path::PathBuf {
	data_dir
		.join("compositions")
		.join(&id[..2])
		.join(format!("{}.json", &id[2..]))
}

pub fn format_id_header(comp: &Composition, id: &str, data_dir: &Path) -> String {
	if let Some(attr) = comp.attribution.first() {
		if let Some(cat) = attr.catalog.as_ref().and_then(|c| c.first()) {
			let catalog_defn = crate::catalog::load_catalog_def(
				data_dir,
				&cat.scheme,
				attr.composer.as_deref(),
			);
			return format_catalog(&cat.scheme, &cat.number, catalog_defn.as_ref());
		}
	}
	id.to_string()
}

pub struct OutputContext<'a> {
	pub data_dir: &'a Path,
	pub config: &'a Config,
	pub scheme: Option<&'a str>,
	pub catalog_defn: Option<&'a CatalogDefinition>,
}

pub fn output_json(results: &[QueryResult], ctx: &OutputContext) {
	let mut output: Vec<serde_json::Value> = Vec::new();

	for result in results {
		let comp_path = id_to_path(ctx.data_dir, &result.id);
		if let Ok(comp) = load_composition(&comp_path) {
			output.push(serde_json::to_value(&comp).unwrap_or(serde_json::Value::Null));
		}
	}

	if output.len() == 1 {
		println!("{}", serde_json::to_string_pretty(&output[0]).unwrap());
	} else {
		println!("{}", serde_json::to_string_pretty(&output).unwrap());
	}
}

pub fn output_movements(results: &[QueryResult], ctx: &OutputContext) {
	let multi = results.len() > 1;

	for result in results {
		let comp_path = id_to_path(ctx.data_dir, &result.id);

		if let Ok(comp) = load_composition(&comp_path) {
			if multi {
				let header = match (&result.number, ctx.scheme) {
					(Some(n), Some(s)) => format_catalog(s, n, ctx.catalog_defn),
					(Some(n), None) => n.clone(),
					(None, _) => result.id.clone(),
				};
				println!("{}:", header);
			}

			let prefix = if multi { "  " } else { "" };

			if let Some(movements) = &comp.movements {
				for (i, movement) in movements.iter().enumerate() {
					let title = movement
						.title
						.as_deref()
						.or(movement.form.as_deref())
						.unwrap_or("?");
					println!("{}{}. {}", prefix, i + 1, title);
				}
			} else if let Some(sections) = &comp.sections {
				for (i, section) in sections.iter().enumerate() {
					let title = section
						.title
						.as_deref()
						.or(section.form.as_deref())
						.unwrap_or("?");
					println!("{}{}. {}", prefix, i + 1, title);
				}
			}

			if multi {
				println!();
			}
		}
	}
}

pub fn output_terse(results: &[QueryResult], scheme: Option<&str>) {
	for result in results {
		match (&result.number, scheme) {
			(Some(n), Some(s)) => println!("{}:{}\t{}", s, n, result.id),
			(Some(n), None) => println!("{}\t{}", n, result.id),
			(None, _) => println!("{}", result.id),
		}
	}
}

pub fn output_pretty(results: &[QueryResult], ctx: &OutputContext) {
	for result in results {
		let comp_path = id_to_path(ctx.data_dir, &result.id);

		if let Ok(comp) = load_composition(&comp_path) {
			let expansion_ctx = ExpansionContext {
				composition: &comp,
				collection: None,
				position_in_collection: None,
				config: &ctx.config.display,
			};
			let title = expand_title(&expansion_ctx);
			match (&result.number, ctx.scheme) {
				(Some(n), Some(s)) => {
					let formatted = format_catalog(s, n, ctx.catalog_defn);
					println!("{}, {}", title, formatted);
				}
				(Some(n), None) => println!("{}, {}", title, n),
				(None, _) => println!("{} [{}]", title, result.id),
			}
		} else {
			match (&result.number, ctx.scheme) {
				(Some(n), Some(s)) => {
					let formatted = format_catalog(s, n, ctx.catalog_defn);
					println!("{}", formatted);
				}
				(Some(n), None) => println!("{}", n),
				(None, _) => println!("{}", result.id),
			}
		}
	}
}

pub fn output_by_ids(
	ids: &[String],
	data_dir: &Path,
	config: &Config,
	terse: bool,
	movements: bool,
	json: bool,
) {
	let results: Vec<QueryResult> = ids
		.iter()
		.map(|id| QueryResult {
			id: id.clone(),
			number: None,
			superseded: false,
			current_number: None,
			note: None,
		})
		.collect();

	let ctx = OutputContext {
		data_dir,
		config,
		scheme: None,
		catalog_defn: None,
	};

	if json {
		output_json(&results, &ctx);
	} else if movements {
		output_movements(&results, &ctx);
	} else if terse {
		output_terse(&results, None);
	} else {
		for id in ids {
			let comp_path = id_to_path(data_dir, id);
			if let Ok(comp) = load_composition(&comp_path) {
				let expansion_ctx = ExpansionContext {
					composition: &comp,
					collection: None,
					position_in_collection: None,
					config: &config.display,
				};
				let title = expand_title(&expansion_ctx);
				let header = format_id_header(&comp, id, data_dir);
				println!("{}, {}", title, header);
			} else {
				println!("{}", id);
			}
		}
	}
}

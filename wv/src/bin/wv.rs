use std::path::PathBuf;

use clap::{Parser, Subcommand};
use werkverzeichnis::commands;
use werkverzeichnis::config::{resolve_data_dir, Config};
use werkverzeichnis::add::generate_id;

#[derive(Parser)]
#[command(name = "wv")]
#[command(about = "Werkverzeichnis - Classical music catalog tools")]
#[command(version)]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

#[derive(Subcommand)]
enum Commands {
	ParseComposition {
		path: PathBuf,
	},

	ParseComposer {
		path: PathBuf,
	},

	ParseCollection {
		path: PathBuf,
	},

	Sort {
		scheme: String,
		#[arg(long)]
		composer: Option<String>,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	SortKey {
		scheme: String,
		number: String,
		#[arg(long)]
		composer: Option<String>,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	Merge {
		path: PathBuf,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	Index {
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	Get {
		#[arg(help = "Composer slug, or composition ID(s)")]
		target: Option<String>,
		#[arg(help = "Catalog scheme (e.g., bwv, op)")]
		scheme: Option<String>,
		#[arg(help = "Catalog number or range (e.g., 812, 2-10)")]
		number: Option<String>,
		#[arg(long)]
		edition: Option<String>,
		#[arg(long, help = "Filter to a group (e.g., op 2 includes 2, 2/1, 2/2)")]
		group: Option<String>,
		#[arg(long)]
		sorted: bool,
		#[arg(short, long, help = "Terse output (scheme:number and ID only)")]
		terse: bool,
		#[arg(short, long, help = "Show movement structure")]
		movements: bool,
		#[arg(long, help = "Full JSON output")]
		json: bool,
		#[arg(short, long, help = "Quiet mode (suppress messages)")]
		quiet: bool,
		#[arg(short, long, help = "Open in editor")]
		edit: bool,
		#[arg(long, help = "Read IDs from stdin")]
		stdin: bool,
		#[arg(long, help = "Only match current catalog numbers (no superseded)")]
		strict: bool,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	Format {
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	Validate {
		path: Option<PathBuf>,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	Add {
		path: PathBuf,
		#[arg(short, long)]
		force: bool,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	New {
		form: String,
		composer: String,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	Id,

	Collection {
		id: String,
		#[arg(long, help = "Verify all members exist in index")]
		verify: bool,
		#[arg(long, help = "Show full composition details")]
		hydrate: bool,
		#[arg(short, long, help = "Terse output (scheme:number only)")]
		terse: bool,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},

	Collections {
		#[arg(help = "Composition ID or catalog number (e.g., bwv:812)")]
		query: String,
		#[arg(long, value_name = "PATH")]
		data_dir: Option<PathBuf>,
	},
}

fn main() {
	let cli = Cli::parse();
	let config = Config::load();

	match cli.command {
		Commands::ParseComposition { path } => {
			commands::parse::run_composition(&path);
		}
		Commands::ParseComposer { path } => {
			commands::parse::run_composer(&path);
		}
		Commands::ParseCollection { path } => {
			commands::parse::run_collection(&path);
		}
		Commands::Sort { scheme, composer, data_dir } => {
			let data_dir = resolve_data_dir(data_dir.as_ref(), &config);
			commands::sort::run_sort(&scheme, composer.as_deref(), &data_dir);
		}
		Commands::SortKey { scheme, number, composer, data_dir } => {
			let data_dir = resolve_data_dir(data_dir.as_ref(), &config);
			commands::sort::run_sort_key(&scheme, &number, composer.as_deref(), &data_dir);
		}
		Commands::Merge { path, data_dir } => {
			let data_dir = resolve_data_dir(data_dir.as_ref(), &config);
			commands::merge::run(&path, &data_dir);
		}
		Commands::Index { data_dir } => {
			let data_dir = resolve_data_dir(data_dir.as_ref(), &config);
			commands::index::run(&data_dir);
		}
		Commands::Get {
			target,
			scheme,
			number,
			edition,
			group,
			sorted,
			terse,
			movements,
			json,
			quiet,
			edit,
			stdin,
			strict,
			data_dir,
		} => {
			let data_dir = resolve_data_dir(data_dir.as_ref(), &config);
			let args = commands::get::GetArgs {
				target: target.map(|x| x.to_lowercase()),
				scheme: scheme.map(|x| x.to_lowercase().trim_end_matches('.').to_string()),
				number,
				edition: edition.map(|x| x.to_lowercase()),
				group,
				sorted,
				terse,
				movements,
				json,
				quiet,
				edit,
				stdin,
				strict,
			};
			commands::get::run(args, data_dir, &config);
		}
		Commands::Format { data_dir } => {
			let data_dir = resolve_data_dir(data_dir.as_ref(), &config);
			commands::format::run(&data_dir, &config);
		}
		Commands::Validate { path, data_dir } => {
			let data_dir = resolve_data_dir(data_dir.as_ref(), &config);
			commands::validate::run(path.as_deref(), &data_dir);
		}
		Commands::Add { path, force, data_dir } => {
			let data_dir = resolve_data_dir(data_dir.as_ref(), &config);
			commands::add::run(&path, force, &data_dir);
		}
		Commands::New { form, composer, data_dir } => {
			let data_dir = resolve_data_dir(data_dir.as_ref(), &config);
			commands::new::run(&form, &composer, &data_dir);
		}
		Commands::Id => {
			println!("{}", generate_id());
		}
		Commands::Collection { id, verify, hydrate, terse, data_dir } => {
			let data_dir = resolve_data_dir(data_dir.as_ref(), &config);
			commands::collection::run_collection(&id, verify, hydrate, terse, &data_dir, &config);
		}
		Commands::Collections { query, data_dir } => {
			let data_dir = resolve_data_dir(data_dir.as_ref(), &config);
			commands::collection::run_collections(&query, &data_dir);
		}
	}
}

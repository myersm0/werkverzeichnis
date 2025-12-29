# werkverzeichnis
**werkverzeichnis** (German: "catalog of works") provides human- and machine-readable data about classical compositions: catalog numbers, keys, instrumentation, movement structures, and attribution history.

The name comes from the well-known *Bach-Werke-Verzeichnis* (catalog of Bach's works), but the scope is broader. Catalogs have accumulated over centuries for each composer — BWV for Bach, Köchel-Verzeichnis for Mozart, opus numbers for most Romantic composers, and dozens more. This project aims to bring all these disparate systems together under one simple, structured, queryable format.

This project prioritizes:
- **Structured data** — JSON files with consistent schemas
- **Human-readable source files** — Plain text JSON files (like [this one](compositions/27/c3084a.json)) you can open in any text editor, inspect, and understand
- **Catalog precision and rigor** — Allow disambiguation of multiple numbering systems per composer
- **Temporal accuracy** — Track attribution and catalog changes over time (reattributions, revised dates)
- **Practical tooling** — We provide a powerful command-line interface in the Rust language for retrieval, validating, and managing the dataset
- **Configurable output** — Display preferences (language, formatting) can be customized per user

## Roadmap
This project is still in an early stage of development. By the end of 2025 the following are expected to be complete:
- [x] Bach keyboard suite collections (six keyboard partitas, French & English suites)
- [x] Bach solo string suites (cello suites, sonatas and partitas for solo violin)
- [x] Bach Well-Tempered Clavier I & II, Goldberg Variations
- [ ] Bach complete cantatas
- [x] Beethoven: the 32 piano sonatas
- [x] Mozart: the 19 piano sonatas
- [ ] Haydn complete piano sonatas
- [x] Schubert complete piano sonatas

## Quick start
A future version will provide compiled binaries so that you don't have to build it yourself and don't even need to have Rust installed on your system. But for now:
```bash
# Clone the repository
git clone https://github.com/myersm0/werkverzeichnis
cd werkverzeichnis

# Build the CLI tool
cd wv
cargo build --release
alias wv="$(pwd)/target/release/wv"
```

### Basic usage
Here are some common retrieval patterns:
```
$ wv get beethoven op 2
Sonata in f minor, op. 2 no. 1
Sonata in A major, op. 2 no. 2
Sonata in C major, op. 2 no. 3

# Or use the --terse flag to get just the catalog and id numbers:
$ wv get beethoven op 2 --terse
op:2/1  fba99784
op:2/2  edfa8309
op:2/3  7023f148

# Output results as JSON (not shown here for brevity):
$ wv get beethoven op 2 --json

# Open the matching JSON file(s) in a text editor (customize this in your config.toml):
$ wv get beethoven op 2 --edit

# Get results from a range of opus numbers:
$ wv get beethoven op 2-11
Sonata in f minor, op. 2 no. 1
Sonata in A major, op. 2 no. 2
Sonata in C major, op. 2 no. 3
Sonata in E♭ major, op. 7
Sonata in c minor, op. 10 no. 1
Sonata in F major, op. 10 no. 2
Sonata in D major, op. 10 no. 3

# Show movements for a piece:
$ wv get beethoven op 2/1 --movements
1. Allegro
2. Adagio
3. Menuetto and Trio (Allegretto)
4. Prestissimo

```

### Attribution and catalog disambiguation
A key feature of our project is that we provide a stable ID for each composition and a way of disambiguating catalogue references. For example, Mozart's famous "Alla Turca" sonata is numbered K. 331 in the original 1862 Köchel catalog, then it changed to  K. 300i in the 1964 sixth edition, and most recently in 2024 it changed back to K. 331 for the ninth edition. Both numbers are commonly used today.

```
# By default, `wv get` will operate with respect to the *latest* catalog numbering:
$ wv get mozart k 331
Sonata in A major, K. 331

# Equivalent to the above, but explicitly reference the 9th Köchel edition:
$ wv get mozart k 331 --edition 9
Sonata in A major, K. 331

# Or reference the 6th edition instead:
$ wv get mozart k 300i --edition 6
Sonata in A major, K. 300i
```

All three calls above resolve to the same composition ID in our system. 

Alternatively if you search for the older catalog number _without_ specifying which edition you mean to use, you will still get the desired result but along with a warning:
```
$ wv get mozart k 300i
warning: K 300i is superseded (current: 331)
Sonata in A major, K. 300i
```

Or set the `--strict` flag to prohibit this behavior:
```
$ wv get mozart k 300i --strict
No results found.
```

Note: when retrieving a _range_ of catalog numbers, `--strict` is on by default to prevent returning potenially duplicated results.

## Configuration
The CLI can be customized to match your preferences. Create a config file at `~/.config/wv/config.toml`:
```toml
[display]
language = "en"
```

### Language
Output like key signatures and titles adapts to your language setting:

| Language | Example |
|----------|---------|
| `en` | C major, F♯ minor |
| `de` | C-Dur, fis-Moll |

Currently English and German are supported; more languages may be added later.

### Display patterns
When a composition doesn't have an explicit title, one is generated from its form, key, and position. You can customize the pattern:
```toml
[display.patterns]
generic = "{form} in {key}"
with_number = "{form} no. {num} in {key}"
```

For example, `"{form} no. {num} in {key}"` produces something like "Suite no. 3 in B minor".

### Symbols
Choose between Unicode and ASCII for accidentals:
```toml
[display]
key_symbols = "unicode"  # F♯, B♭
# key_symbols = "ascii"  # F#, Bb
```

See [wv/README.md](wv/README.md) for full configuration options.

## Repository structure
```
werkverzeichnis/
├── compositions/       # Individual composition files (by ID prefix)
│   ├── 1a/
│   ├── 2b/
│   └── ...
├── composers/          # Composer metadata and catalog definitions
├── catalogs/           # Shared catalog schemes (op, k, etc.)
├── collections/        # Curated groupings (by composer)
│   ├── bach/
│   ├── beethoven/
│   └── ...
├── schemas/            # JSON schemas for validation
├── wv/                 # CLI tool (Rust)
└── .indexes/           # Generated index files (gitignored)
```

## Data model
### Compositions
Each composition has a stable 8-character ID and lives in `compositions/{prefix}/{suffix}.json`. For example, Beethoven's first piano sonata (opus 2, number 1):
```json
{
	"id": "fba99784",
	"key": "f",  # stored in terse format; expands to "f minor" or "f-Moll" on output
	"form": "sonata",
	"instrumentation": "piano",
	"attribution": [
		{
			"composer": "beethoven",
			"dates": {"composed": 1795, "published": 1796},
			"catalog": [{"scheme": "op", "number": "2/1"}]
		}
	],
	"movements": [
		{"title": "Allegro"},
		{"title": "Adagio", "key": "F"},
		{"title": "Menuetto and Trio (Allegretto)"},
		{"title": "Prestissimo"}
  ]
}
```

### Attribution over time
Attribution entries are sorted in reverse-chronological order, with the newest entry at the top. The optional `since` field tracks when attribution information became accepted:
```json
{
  "attribution": [
    {
      "composer": "bach",
      "catalog": [{"scheme": "bwv", "number": "565"}],
      "status": "doubtful",
      "since": "1980"
    },
    {
      "composer": "bach",
      "catalog": [{"scheme": "bwv", "number": "565"}],
      "status": "certain",
      "since": "1708"
    }
  ]
}
```

### Collections
Ordered groupings like "French Suites" or "Well-Tempered Clavier, Book 1":
```json
{
  "id": "bach-french-suites",
  "title": {"en": "French Suites"},
  "expansion_pattern": {"en": "French Suite no. {num} in {key}"},
  "composer": "bach",
  "scheme": "bwv",
  "compositions": ["812", "813", "814", "815", "816", "817"]
}
```

### Catalog schemes
Catalog definitions specify parsing, sorting, and display rules. For example, here's a simplified definition for BWV:
```json
{
  "id": "bwv",
  "name": "Bach-Werke-Verzeichnis",
  "canonical_format": "BWV {number}",
  "pattern": "^(\\d+)([a-z])?$",
  "sort_keys": [
    {"group": 1, "type": "int"},
    {"group": 2, "type": "str"}
  ]
}
```

(The actual definition is more complex, to allow for records like "BWV Anh. III 135".)

## Data generation
This dataset is compiled using AI large language models (LLMs) to process and structure information from public sources (catalogs, Wikipedia, musicological references). Specifically, I select reference texts myself, and then use an efficient voice-driven workflow for coordinating several independent `claude-haiku-4-5` instances to do a multi-stage generation and review based on those materials. Any questionable or conflicting results are flagged for human review. Generated materials are then validated against our schema and styleguide before acceptance into the database. This has resulted in the best balance of human labor, LLM cost (typically less than one cent per composition), and accuracy of the final product.

This generation-and-review process is still evolving. We aim to maintain a very high standard of accuracy and quality — because if the data isn't reliable, it isn't useful. Still, despite our best efforts, errors are inevitable in any project of this scope, so corrections are welcome via pull request or issue.

## CLI tool
The `wv` command-line tool provides:
- **get** — Look up compositions by composer, catalog, range
- **collection** — List and verify collections
- **validate** — Check files against schemas
- **index** — Build search indexes
- **add** / **new** — Create new composition entries

See [wv/README.md](wv/README.md) for detailed usage.

## Schemas
JSON schemas in `schemas/` define the structure of all data files:
- `composition.schema.json`
- `composer.schema.json`
- `collection.schema.json`
- `catalog.schema.json`

## References and acknowledgments
This project is focused on providing a unified, machine-readable structure to available information, _not_ on inventing any new information or applying any new research or insights. Therefore, we're indebted to a number of existing resources on the web, including:
- Wikipedia
- [bach-cantatas.com](https://www.bach-cantatas.com/)

## License
This project is licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). You are free to use, adapt, and redistribute the data with attribution.


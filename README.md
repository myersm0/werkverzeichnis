# werkverzeichnis
**werkverzeichnis** (German: "catalog of works") provides human- and machine-readable data about classical compositions: catalog numbers, keys, instrumentation, movement structures, and attribution history.

The name evokes the well-known *Bach-Werke-Verzeichnis* (catalog of Bach's works), but the scope is broader. Classical music catalogs have accumulated over centuries for each composer — BWV for Bach, Köchel-Verzeichnis for Mozart, opus numbers for most Romantic composers, and dozens more. This project aims to bring all these disparate systems together under one simple, structured, queryable format.

This project prioritizes:
- **Structured data** — JSON files with consistent schemas
- **Human-readable source files** — Plain text JSON files (like [this one](compositions/27/c3084a.json)) you can open in any text editor, inspect, and understand
- **Catalog precision and rigor** — Allow for multiple numbering systems per composer, with cross-references
- **Temporal accuracy** — Attribution and catalog changes over time (reattributions, revised dates) are tracked
- **Practical tooling** — We provide a powerful command-line interface in the Rust language for querying, validating, and managing the dataset
- **Configurable output** — Display preferences (language, formatting) can be customized per user

## Roadmap
This project is still in an early stage of development. By the end of 2025 the following are expected to be complete:
- [x] Bach keyboard suite collections (six keyboard partitas, French & English suites)
- [x] Bach solo string suites (cello suites, sonatas and partitas for solo violin)
- [x] Bach Well-Tempered Clavier I & II, Goldberg Variations
- [ ] Bach complete cantatas, masses, passions
- [x] Beethoven: the 32 piano sonatas
- [x] Mozart: the 19 piano sonatas
- [ ] Haydn complete piano sonatas
- [ ] Schubert complete piano sonatas

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

Here are some common query patterns:
```
$ wv query beethoven op 2
op:2/1  fba99784
op:2/2  edfa8309
op:2/3  7023f148

# Nicer output with the --pretty flag:
$ wv query beethoven op 2 --pretty
Sonata in f minor, op. 2 no. 1
Sonata in A major, op. 2 no. 2
Sonata in C major, op. 2 no. 3

# Output sorted over a range of opus numbers:
$ wv query beethoven op --range 2-11 --pretty
Sonata in f minor, op. 2 no. 1
Sonata in A major, op. 2 no. 2
Sonata in C major, op. 2 no. 3
Sonata in E♭ major, op. 7
Sonata in c minor, op. 10 no. 1
Sonata in F major, op. 10 no. 2
Sonata in D major, op. 10 no. 3

# Show movements for a piece:
$ wv query beethoven op 2/1 --movements
1. Allegro
2. Adagio
3. Menuetto and Trio (Allegretto)
4. Prestissimo
```

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
Each composition has a stable 8-character ID and lives in `compositions/{prefix}/{suffix}.json`:
```json
{
  "id": "2e0c3f46",
  "form": "suite",
  "key": "D minor",
  "attribution": [
    {
      "composer": "bach",
      "catalog": [{"scheme": "bwv", "number": "812"}],
      "dates": {"composed": 1722}
    }
  ],
  "movements": [
    {"title": "Allemande"},
    {"title": "Courante"},
    {"title": "Sarabande"}
  ]
}
```

### Attribution over time
The `since` field tracks when attribution information became accepted:
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
Catalog definitions specify parsing, sorting, and display rules:
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

## Data generation
This dataset is compiled using AI large language models (LLMs) to process and structure information from public sources (catalogs, Wikipedia, musicological references). Manual curation at this scale would be an enormous undertaking — LLMs make it feasible.

However, LLM output is notoriously subject to hallucination and requires careful verification. To that end, our workflow includes multiple review stages:
1. **Structured extraction** — LLMs parse source material into our schema format
2. **Cross-reference validation** — Catalog numbers, dates, and keys are checked against authoritative sources
3. **Schema validation** — All entries must pass JSON schema and styleguide checks
4. **Human review** — Spot checks and corrections

This generation-and-review process is still evolving. The goal is accuracy above all else — because if the data isn't reliable, it isn't useful. But errors are inevitable in any project of this scope; corrections are welcome via pull request or issue.

## CLI tool
The `wv` command-line tool provides:
- **query** — Look up compositions by composer, catalog, range
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


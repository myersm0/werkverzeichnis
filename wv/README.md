# wv — werkverzeichnis CLI

Command-line tool for querying and maintaining the werkverzeichnis dataset.

The project-level README describes the data model and dataset. This file focuses on the `wv` command itself.

## Installation

For a normal installation, use the repository-level installer described in the main README. Release archives contain both the executable and a matching `data/` directory, so a separate repository clone or hand-written `data_dir` configuration is not required.

For development:

```bash
cd wv
cargo build --release
```

The binary will be at `target/release/wv`. When run anywhere inside a werkverzeichnis checkout, it automatically searches upward for the repository data.

## Configuration

The optional config file is normally `~/.config/wv/config.toml`:

```toml
data_dir = "/path/to/werkverzeichnis"
editor = "nvim"

[display]
language = "en"
key_symbols = "unicode"

[display.patterns]
generic = "{form} in {key}"
with_number = "{form} no. {num} in {key}"
instrumentation_max_chars = 40

[xref]
mb_database = "/path/to/mb.db"
```

`data_dir` is normally unnecessary. It is useful for development checkouts or alternate datasets.

Data directory resolution order:

1. `--data-dir`
2. non-empty `WV_DATA_DIR`
3. `data_dir` in `config.toml`
4. nearest ancestor containing the werkverzeichnis data directories
5. a bundled `data/` directory beside the executable
6. platform-standard application data

On macOS, the installed data directory is normally:

```text
~/Library/Application Support/werkverzeichnis
```

On Linux it is:

```text
$XDG_DATA_HOME/werkverzeichnis
```

or, when `XDG_DATA_HOME` is unset:

```text
~/.local/share/werkverzeichnis
```

A valid data directory contains all of:

```text
catalogs/
collections/
composers/
compositions/
schemas/
```

An explicit override that is not a complete werkverzeichnis dataset is an error. If no dataset can be found, `wv` exits with a diagnostic instead of silently using the current directory.

The editor used by `get --edit` is selected in this order:

1. `editor` in `config.toml`
2. `$EDITOR`
3. `vi`

The `[xref]` section is needed only for commands that consult a local MusicBrainz database.

## Querying compositions

### Basic queries

Query by composer, catalog scheme, and number:

```bash
$ wv get bach bwv 812
Suite in d minor, BWV 812

$ wv get beethoven op 2
Sonata in f minor, op. 2 no. 1
Sonata in A major, op. 2 no. 2
Sonata in C major, op. 2 no. 3
```

Query a range:

```bash
$ wv get bach bwv 812-817
Suite in d minor, BWV 812
Suite in c minor, BWV 813
...
```

Catalog numbers with multiple parts do not need shell quoting:

```
$ wv get bach bwv anh. iii 141
Das ist je gewißlich wahr, BWV Anh. III 141
```

Catalog categories and abbreviated ranges can be written naturally when the catalog defines them:

```
$ wv get haydn hob iii
$ wv get haydn hob iii 32
$ wv get haydn hob iii 31-33
$ wv get haydn hob iii:31-33
```

Query all indexed works for a composer:

```bash
$ wv get bach
...
```

Query directly by stable composition ID:

```bash
$ wv get 2e0c3f46
Suite in d minor, BWV 812
```

Multiple IDs can be supplied at once:

```bash
$ wv get 2e0c3f46 3f4d5e6a
```

Or read IDs from standard input:

```bash
$ printf '%s\n' 2e0c3f46 3f4d5e6a | wv get --stdin
```

### Output modes

Default output is human-readable.

`--terse` outputs stable composition IDs only:

```bash
$ wv get bach bwv 812 --terse
2e0c3f46
```

`--movements` shows the movement or section structure:

```bash
$ wv get bach bwv 812 --movements
1. Allemande
2. Courante
3. Sarabande
...
```

`--json` emits the complete composition object:

```bash
$ wv get bach bwv 812 --json
{
  "id": "2e0c3f46",
  ...
}
```

`--edit` opens the matching JSON file or files in the configured editor:

```bash
$ wv get bach bwv 812 --edit
```

Because an edit may change indexed data, `get --edit` marks the index stale after the editor exits.

### Catalog history and editions

By default, historical catalog references may resolve to the same stable composition as their current reference. A warning is printed when a superseded number is used.

Use `--strict` to match only current catalog information:

```bash
$ wv get mozart k 300i --strict
No results found.
```

Use `--edition` for catalogs that explicitly track editions:

```bash
$ wv get mozart k 300i --edition 6
$ wv get mozart k 331 --edition 9
```

Ranges and grouped queries are sorted according to the catalog definition:

```bash
$ wv get beethoven op 2-10
$ wv get beethoven op --group 2
```

Use `--sorted` to request catalog sorting explicitly.

### Catalog knowledge and inventories

A catalog query can fail for different reasons, and `wv` keeps those reasons separate:

```bash
$ wv get beethoven op 2/e
Invalid catalog number for beethoven / Opus: "2/e"

$ wv get beethoven op 2/0
Catalog number out of range for beethoven / Opus: "2/0" (sub-number 0 is below the minimum 1)

$ wv get beethoven op 2/4
No such catalog entry: op. 2 no. 4

$ wv get beethoven op 138
op. 138
catalog entry known; detailed record not yet available
```

The first two results come from the catalog definition: its parser decides whether an identifier is well formed, and its structural constraints define allowed component domains. The latter two use catalog inventories under `inventories/`, which enumerate identifiers assigned by the catalog independently of the rich composition records.

An inventory marked `complete = true` makes absence authoritative. An incomplete inventory makes positive assertions only: an unlisted identifier remains unknown. Inventory files are TOML and may use comments freely; comments are ignored by `wv`. See the repository-level `inventories/README.md` for the source format.

Use `coverage` to compare a catalog inventory with the detailed records currently loaded:

```bash
$ wv coverage beethoven op
$ wv coverage beethoven op --missing
```

The report gives inventory size, populated count, missing count, and percentage coverage. Omit the scheme to report every inventory available for the composer. `--missing` turns the inventory into a work queue by listing catalogued identifiers that do not yet have detailed composition records.

### Collections as query input

One or more collections can be expanded as input to `get`:

```bash
$ wv get --collection bach-french-suites
$ wv get --collection bach-french-suites --terse
$ wv get --collection bach-french-suites --movements
$ wv get --collection bach-french-suites --json
```

Official collections and user collections can both be used this way.

### MusicBrainz cross-reference lookup

When `[xref].mb_database` is configured, `--xref mb` performs a lookup against that local database:

```bash
$ wv get beethoven op 2 --xref mb
```

This does not modify werkverzeichnis data. See `wv set` below for writing MusicBrainz cross-references into composition files.

### `get` flags

- `-t, --terse` — output stable composition IDs only
- `-m, --movements` — show movement or section structure
- `--json` — output complete JSON
- `-q, --quiet` — suppress informational messages and warnings
- `-e, --edit` — open matching files in the configured editor
- `--stdin` — read composition IDs from standard input
- `--sorted` — sort by catalog order
- `--group NUM` — restrict to a catalog group
- `--edition NAME` — query a particular catalog edition
- `--strict` — use current catalog references only
- `--xref TYPE` — perform a configured external cross-reference lookup
- `-c, --collection ID...` — expand collection IDs as input
- `--data-dir PATH` — override dataset discovery

## Collections

Collections are ordered groups of catalog references such as the French Suites or Well-Tempered Clavier, Book I.

### List collections

```bash
$ wv collection list
```

Restrict to one composer:

```bash
$ wv collection list bach
```

List user collections instead of canonical repository collections:

```bash
$ wv collection list --user
```

### Show a collection

```bash
$ wv collection show bach-french-suites
French Suites

Suite in d minor, BWV 812
Suite in c minor, BWV 813
...
```

`show` checks canonical collections first and then `user-collections/`.

### Find collections containing a work

```bash
$ wv collection find bwv:812
bach-french-suites
```

`find` searches canonical collections.

## Validation

`wv validate` checks both JSON structure and cross-file consistency.

Validate the complete canonical dataset:

```bash
$ wv validate
Validating dataset in "/path/to/werkverzeichnis"...
No validation errors found.
```

Validate one canonical composition, composer, catalog, or collection file:

```bash
$ wv validate compositions/2e/0c3f46.json
```

Full-dataset validation includes:

- validation against the appropriate JSON Schema;
- rejection of unknown fields where the data contract is closed;
- composition, composer, catalog, and collection ID/path consistency;
- references to existing composers and applicable catalog schemes;
- validation of composer `default_scheme` values;
- catalog numbers matching the declared catalog regex;
- catalog numbers satisfying declared structural-domain constraints;
- inventory TOML parsing, identity, uniqueness, and catalog-number validity;
- composition references belonging to an applicable complete inventory;
- edition labels existing in the corresponding catalog definition;
- uniqueness of each current `(composer, scheme, number)` identifier;
- canonical collection members resolving to current compositions;
- rejection of duplicate members within a canonical collection.

These checks are intentionally structural and referential. They do not attempt to decide musicological questions such as whether an attribution or date is historically correct.

## Indexes

Composer/catalog queries use generated files under `.indexes/`:

```text
.indexes/
├── index.json
├── composer-index.json
├── inventory-index.json
├── editions/
└── metadata.json
```

Build them explicitly with:

```bash
$ wv index
Building index from "/path/to/werkverzeichnis"...
Found 533 compositions
Found 596 catalog entries
Found ... inventory entries
Wrote .../.indexes/index.json
Wrote .../.indexes/composer-index.json
Wrote .../.indexes/inventory-index.json
Wrote edition indexes to .../.indexes/editions
Wrote .../.indexes/metadata.json
Done.
```

`metadata.json` records when the index was built and whether it has been marked dirty.

Index policy:

- a missing index is rebuilt automatically;
- a missing metadata file is treated as stale;
- a dirty index is rebuilt on the next indexed query;
- an index more than 24 hours old is rebuilt automatically;
- `add`, `new`, `set`, and `get --edit` mark the index dirty;
- `wv index` forces an immediate rebuild.

The 24-hour check is a safety net for changes made outside `wv`, such as manual JSON/TOML edits, Git operations, or copying/restoring the dataset. After a manual edit, run `wv index` when an immediate refresh is important.

## Maintaining composition data

### add

Add a reviewed composition JSON file to the repository layout:

```bash
$ wv add /tmp/composition.json
Added /tmp/composition.json -> /path/to/werkverzeichnis/compositions/ab/cdef12.json
ID: abcdef12
```

Use `--force` to overwrite an existing destination:

```bash
$ wv add /tmp/composition.json --force
```

A successful add marks the index dirty.

### new

Create a minimal composition scaffold:

```bash
$ wv new sonata schubert
Created /path/to/werkverzeichnis/compositions/ab/cdef12.json
ID: abcdef12
```

The generated file contains an ID, form, and initial composer attribution. A successful `new` marks the index dirty.

### id

Generate a new 8-character composition ID without creating a file:

```bash
$ wv id
7b2f9c4e
```

### set

`set` currently supports adding MusicBrainz cross-references from a configured local MusicBrainz database:

```bash
$ wv set beethoven op 2 --xref mb
```

A range can also be supplied:

```bash
$ wv set beethoven op 2-10 --xref mb
```

This modifies matching composition JSON files and marks the index dirty.

## JSON pipelines

`wv get --json` can be combined with `jq`, and `wv format` converts composition JSON back to normal human-readable output.

```bash
$ wv get bach bwv 812 --json | jq '.movements[].title'
"Allemande"
"Courante"
...
```

```bash
$ wv get beethoven op 2-20 --json \
  | jq '.[] | select(.attribution[0].dates.composed < 1800)' \
  | wv format
```

`wv format` accepts either one composition object or an array of composition objects on standard input.

## Catalog utilities

Catalog parsing, formatting, and sorting rules live in the catalog JSON definitions rather than being hard-coded into individual queries.

Sort catalog numbers read from standard input:

```bash
$ printf '%s\n' 10 2/2 2 2/1 | wv sort op
2
2/1
2/2
10
```

For a composer-specific catalog definition:

```bash
$ printf '%s\n' 331 300i 545 | wv sort k --composer mozart
```

Inspect the internal sort key for one number:

```bash
$ wv sort-key op 2/1
```

## Developer/debugging utilities

These commands are mainly useful when developing or checking the data model.

### merge

Show the effective attribution obtained by merging a composition's attribution history:

```bash
$ wv merge compositions/2e/0c3f46.json
```

This is diagnostic output; it does not rewrite the file.

### parse-composition

Deserialize a composition and print its Rust debug representation:

```bash
$ wv parse-composition compositions/2e/0c3f46.json
```

Equivalent commands exist for the other structured types:

```bash
$ wv parse-composer composers/bach.json
$ wv parse-collection collections/bach/french-suites.json
```

## Catalog formatting

Human-readable catalog labels are controlled by their catalog definitions. For example, definitions specify the parsing regex, canonical display format, sort groups, aliases, and—where applicable—catalog editions.

This lets queries use one common mechanism for schemes such as BWV, K., opus numbers, Hoboken numbers, Deutsch numbers, and WoO without embedding those conventions in the query code.

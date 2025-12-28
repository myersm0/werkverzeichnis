# wv — werkverzeichnis CLI

Command-line tool for querying and managing the werkverzeichnis dataset.

## Installation

```bash
cd wv
cargo build --release
```

Binary will be at `target/release/wv`. Add to PATH or create an alias:

```bash
alias wv="$(pwd)/target/release/wv"
```

## Configuration

Optional config file at `~/.config/wv/config.toml`:

```toml
# Path to data repository (if not running from repo directory)
data_dir = "/path/to/werkverzeichnis"

# Editor for --edit flag (defaults to $EDITOR, then vi)
editor = "nvim"

[display]
language = "en"          # en, de
key_symbols = "unicode"  # unicode (♯ ♭) or ascii (# b)

[display.patterns]
generic = "{form} in {key}"
with_number = "{form} no. {num} in {key}"
```

Data directory resolution order:
1. `--data-dir` flag
2. `WV_DATA_DIR` environment variable
3. Config file
4. Current or parent directory (if `composers/` exists)

## Commands

### get

Retrieve compositions by composer and catalog number, or by ID. Default output is human-readable with expanded titles.

```bash
$ wv get bach bwv 812
Suite in D minor, BWV 812

$ wv get bach bwv 812-817
Suite in D minor, BWV 812
Suite in C minor, BWV 813
Suite in B minor, BWV 814
...

$ wv get beethoven op 2
Sonata in F minor, op. 2 no. 1
Sonata in A major, op. 2 no. 2
Sonata in C major, op. 2 no. 3
```

**Get by ID:**

```bash
$ wv get 2e0c3f46
Suite in D minor, BWV 812

$ wv get 2e0c3f46 3f4d5e6a
Suite in D minor, BWV 812
Suite in C minor, BWV 813

$ echo "2e0c3f46" | wv get --stdin
Suite in D minor, BWV 812
```

**Output modes:**

```bash
# Default: pretty (expanded titles)
$ wv get bach bwv 812
Suite in D minor, BWV 812

# Terse: machine-readable (scheme:number + ID)
$ wv get bach bwv 812 -t
bwv:812    2e0c3f46

# Movements
$ wv get bach bwv 812 -m
1. Allemande
2. Courante
3. Sarabande
4. Menuet I
5. Menuet II
6. Gigue

# JSON: full composition data
$ wv get bach bwv 812 --json
{
  "id": "2e0c3f46",
  "form": "suite",
  ...
}
```

**Edit in your editor:**

```bash
$ wv get bach bwv 812 --edit
# Opens compositions/2e/0c3f46.json in your configured editor

$ wv get bach bwv 812-817 --edit
# Opens all 6 files
```

**Flags:**
- `-t, --terse` — Machine-readable output (scheme:number and ID)
- `-m, --movements` — Show movement structure
- `--json` — Full JSON output (pipe to `jq` for filtering)
- `-q, --quiet` — Suppress messages
- `-e, --edit` — Open in editor (skips output)
- `--stdin` — Read IDs from stdin
- `--sorted` — Sort results by catalog number
- `--group NUM` — Filter to a group (e.g., op 2 includes 2/1, 2/2, 2/3)
- `--edition NAME` — Filter by edition

**Multi-work movements:**

```bash
$ wv get bach bwv 812-813 -m
BWV 812:
  1. Allemande
  2. Courante
  ...

BWV 813:
  1. Allemande
  2. Courante
  ...
```

### format

Prettify JSON input from stdin. Useful for formatting filtered results.

```bash
$ wv get beethoven op 2 --json | jq '.[0]' | wv format
Sonata in F minor, op. 2 no. 1

$ wv get beethoven op 2-20 --json \
  | jq '.[] | select(.attribution[0].dates.composed < 1800)' \
  | wv format
```

### collection

List compositions in a collection.

```bash
# Default: pretty output
$ wv collection bach-french-suites
French Suite no. 1 in D minor, BWV 812
French Suite no. 2 in C minor, BWV 813
...

# Terse: scheme:number only
$ wv collection bach-french-suites -t
bwv:812
bwv:813
...

# Verify all members exist in index
$ wv collection bach-french-suites --verify
bwv:812 ✓
bwv:813 ✓
...

# Show full composition details
$ wv collection bach-french-suites --hydrate
bwv:812 [2e0c3f46]
  Form: suite
  Key: D minor
...
```

**Flags:**
- `-t, --terse` — Machine-readable output
- `--verify` — Check all members exist
- `--hydrate` — Show full composition details

### collections

Find collections containing a composition.

```bash
$ wv collections bwv:812
Collections containing 'bwv:812':
  bach-french-suites
```

### validate

Validate composition files against schemas.

```bash
# Validate single file
$ wv validate path/to/composition.json

# Validate all files in repository
$ wv validate
```

### index

Build index files for fast lookups.

```bash
$ wv index
Building index...
Found 150 compositions
Wrote .indexes/index.json
Wrote .indexes/composer-index.json
Done.
```

### add

Add a composition file to the repository.

```bash
# Add a reviewed composition
$ wv add path/to/composition.json

# Force overwrite if exists
$ wv add path/to/composition.json --force
```

### new

Scaffold a new composition file.

```bash
$ wv new sonata bach
Created compositions/3a/3a7e4d21.json
ID: 3a7e4d21
```

### id

Generate a random composition ID.

```bash
$ wv id
7b2f9c4e
```

## JSON output and jq

The `--json` flag outputs full composition data, which can be piped to `jq` for filtering:

```bash
# Get just the movements
$ wv get bach bwv 812 --json | jq '.movements[].title'
"Allemande"
"Courante"
...

# Get composed date
$ wv get bach bwv 812 --json | jq '.attribution[0].dates.composed'
1722

# Filter works by key
$ wv get bach bwv 846-869 --json | jq '.[] | select(.key | contains("minor"))'

# Filter and reformat
$ wv get beethoven op 2-20 --json \
  | jq '.[] | select(.attribution[0].dates.composed < 1800)' \
  | wv format
```

## Catalog formatting

Catalog numbers are formatted per scheme conventions:

| Scheme | Example |
|--------|---------|
| op | `op. 27`, `op. 2 no. 1` |
| bwv | `BWV 812` |
| k | `K. 331` |
| hob | `Hob. XVI:52` |
| d | `D. 960` |
| woo | `WoO 59` |

Custom formats can be defined in catalog JSON files via `canonical_format`.

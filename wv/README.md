# wv — werkverzeichnis CLI

Command-line tool for querying and managing the werkverzeichnis dataset.

## Installation

```bash
cd wv
cargo build --release
```

Binary will be at `target/release/wv`. Optionally add to PATH or create an alias.

## Configuration

Optional config file at `~/.config/wv/config.toml`:

```toml
# Path to data repository (if not running from repo directory)
data_dir = "/path/to/werkverzeichnis"

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

### query

Look up compositions by composer and catalog number.

```bash
# Basic query
wv query bach bwv 812
# bwv:812    2e0c3f46

# Pretty output (expanded titles)
wv query bach bwv 812 -p
# Suite in D minor, BWV 812

# Show movements
wv query bach bwv 812 -m
# 1. Allemande
# 2. Courante
# 3. Sarabande
# 4. Menuet I
# 5. Menuet II
# 6. Gigue

# Range query (requires scheme)
wv query bach bwv --range 846-869 -p
# Prelude and Fugue in C major, BWV 846
# Prelude and Fugue in C minor, BWV 847
# ...

# Group query (all works in a group)
wv query beethoven op 2 -p
# Sonata in F minor, op. 2 no. 1
# Sonata in A major, op. 2 no. 2
# Sonata in C major, op. 2 no. 3

# By edition
wv query bach bwv 812 --edition bga
```

**Flags:**
- `-p, --pretty` — Expanded titles with formatted catalog numbers
- `-m, --movements` — Show movement structure
- `--sorted` — Sort results by catalog number
- `--range START-END` — Filter to catalog number range
- `--group NUM` — Filter to a group (e.g., op 2 includes 2/1, 2/2, 2/3)
- `--edition NAME` — Filter by edition

### collection

List compositions in a collection.

```bash
# Basic listing
wv collection bach-french-suites
# French Suites
#
# bwv:812
# bwv:813
# ...

# Pretty output
wv collection bach-french-suites -p
# French Suite no. 1 in D minor, BWV 812
# French Suite no. 2 in C minor, BWV 813
# ...

# Verify all members exist in index
wv collection bach-french-suites --verify
# bwv:812 ✓
# bwv:813 ✓
# ...

# Show full composition details
wv collection bach-french-suites --hydrate
# bwv:812 [2e0c3f46]
#   Form: suite
#   Key: D minor
# ...
```

**Flags:**
- `-p, --pretty` — Expanded titles
- `--verify` — Check all members exist
- `--hydrate` — Show full composition details

### collections

Find collections containing a composition.

```bash
wv collections bwv:812
# Collections containing 'bwv:812':
#   bach-french-suites
```

### validate

Validate composition files against schemas.

```bash
# Validate single file
wv validate path/to/composition.json

# Validate all files in repository
wv validate
```

### index

Build index files for fast lookups.

```bash
wv index
# Building index...
# Found 150 compositions
# Wrote .indexes/index.json
# Wrote .indexes/composer-index.json
# Done.
```

### add

Add a composition file to the repository.

```bash
# Add a reviewed composition
wv add path/to/composition.json

# Force overwrite if exists
wv add path/to/composition.json --force
```

### new

Scaffold a new composition file.

```bash
wv new sonata bach
# Created compositions/3a/3a7e4d21.json
# ID: 3a7e4d21
```

### id

Generate a random composition ID.

```bash
wv id
# 7b2f9c4e
```

### Other commands

```bash
# Parse and display a composition file
wv parse-composition path/to/file.json

# Parse a composer file
wv parse-composer composers/bach.json

# Get sort key for a catalog number
wv sort-key bwv 812
wv sort-key op 2/1 --composer beethoven

# Merge attribution with collection data
wv merge path/to/composition.json
```

## Output formats

### Default (machine-readable)

Tab-separated: `scheme:number    id`

```
bwv:812    2e0c3f46
bwv:813    3f4d5e6a
```

### Pretty (`-p`)

Human-readable titles following OpenOpus style:

```
French Suite no. 1 in D minor, BWV 812
Piano Sonata no. 14 in C-sharp minor, op. 27 no. 2
Le nozze di Figaro, K. 492
```

### Movements (`-m`)

Numbered movement list:

```
1. Allegro
2. Adagio
3. Rondo: Allegretto
```

## Catalog formatting

Catalog numbers are formatted per scheme conventions:

| Scheme | Format |
|--------|--------|
| op | `op. 27`, `op. 2 no. 1` |
| bwv | `BWV 812` |
| k | `K. 331` |
| hob | `Hob. XVI:52` |
| d | `D. 960` |
| woo | `WoO 59` |

Custom formats can be defined in catalog JSON files via `canonical_format`.

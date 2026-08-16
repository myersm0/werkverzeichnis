# Catalog inventories

Inventories record which identifiers actually exist in a catalog independently of the richer records in `compositions/`.

They are hand-maintained TOML files, normally one per composer and catalog scheme, and one per edition where a catalog has editions.

```text
inventories/
    beethoven/
        op.toml
        woo.toml
        hess.toml
    mozart/
        k-1.toml
        k-6.toml
        k-9.toml
```

A basic inventory looks like this:

```toml
composer = "beethoven"
scheme = "op"
complete = false

entries = [
    # Piano trios
    "1/1", "1/2", "1/3",

    # Piano sonatas
    "2/1", "2/2", "2/3",

    "3", # string trio
    "4", # string quintet
]
```

All catalog identifiers are quoted strings. Comments are for maintainer orientation only and carry no semantics.

`complete = true` means every identifier assigned by that catalog or catalog edition is listed. Absence from a complete inventory is therefore a confident negative. Absence from an incomplete inventory is unknown.

Structural validity is defined separately in catalog metadata. Catalog constraints can reject impossible components before inventory lookup, such as Beethoven opus numbers above 138, subordinate number 0, an undefined Hoboken category, or a Telemann genre outside the catalog's genre domain. Inventories answer whether an in-domain identifier was actually assigned.

## Structural domains

Inventories enumerate assigned identifiers. They do not define the grammar or numerical domain of a catalog. Those rules live in the catalog definition and are checked before inventory membership.

For example, the shared opus definition can require subordinate numbers to begin at 1:

```json
"constraints": [
    {"group": 4, "name": "sub-number", "min": 1}
]
```

A composer can add a narrower constraint to the same scheme. Beethoven's opus definition, for example, limits the main opus-number capture group to 1 through 138. Constraints may also use multiple allowed integer ranges for discontinuous domains, and Roman-numeral capture groups use the same mechanism through a `roman` sort key.

The distinction is deliberate: a structurally impossible identifier is rejected even without an inventory, while an in-domain identifier is known not to exist only when it is absent from an applicable complete inventory.

For catalogs with editions, add `edition`:

```toml
composer = "mozart"
scheme = "k"
edition = "9"
complete = false
entries = []
```

## Groups

Inventories list actual catalog identifiers only. Parent/group queries are derived from the catalog definition's `group_by` rule.

For example, if an opus inventory contains:

```toml
entries = ["2/1", "2/2", "2/3"]
```

then `wv get beethoven op 2` can enumerate those three entries even though `"2"` is not itself listed. `"2/4"` is known not to exist only when the inventory is complete.

Letter suffixes remain part of the identifier according to the catalog parser. For example, WoO `2a` does not imply a parent WoO `2` merely because it begins with the same characters.

A bare identifier and subordinate identifiers are allowed to coexist if a catalog genuinely assigns both. Exact membership and derived grouping are separate facts.

## Validation

`wv validate` checks that:

- the TOML parses into the inventory format;
- the composer and scheme exist;
- an edition, if supplied, is defined for the catalog;
- every entry is unique after normalization;
- every entry is well-formed under the catalog's parser;
- every entry satisfies the catalog's structural-domain constraints;
- no composition record refers to an identifier outside the structural domain or absent from an applicable complete inventory.

`wv coverage <composer> [scheme]` compares the inventory with the rich composition records. Add `--missing` to list catalogued entries that do not yet have detailed records.

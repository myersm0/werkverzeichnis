# Catalog inventories

Inventories record which identifiers actually exist in a catalog independently of the richer records in `compositions/`.

A basic inventory looks like this:

```json
{
  "composer": "beethoven",
  "scheme": "op",
  "complete": false,
  "sources": [
    {
      "title": "Source title",
      "url": "https://example.org/"
    }
  ],
  "entries": [
    {
      "number": "2",
      "member_range": 3,
      "label": "piano sonatas"
    },
    {
      "number": "7",
      "label": "septet"
    }
  ]
}
```

`member_range: 3` means that the group contains members 1 through 3. The catalog definition supplies the formatting rule; for opus numbers, `2` expands to `2/1`, `2/2`, and `2/3`. The parent `2` is a group, not an additional leaf entry.

`label` is optional free text. It is only a brief description for human or machine readers, not a controlled classification field.

Set `complete` to `true` only when every entry in that catalog or catalog edition is represented. Absence from a complete inventory means that the requested catalog entry does not exist; absence from an incomplete inventory makes no such claim.

For catalogs with editions, add an edition field:

```json
{
  "composer": "mozart",
  "scheme": "k",
  "edition": "9",
  "complete": false,
  "entries": []
}
```

Inventory files may be organized beneath the composer directory as convenient. Their identity comes from the `composer`, `scheme`, and optional `edition` fields rather than the filename.

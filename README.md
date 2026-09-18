# nhentai lookup plugin

Uses `rs-plugin-common-interfaces` 0.41.0. Book queries support `name`,
`author`, `ids`, `people`, `series`, `tags`, and `pageKey` across `lookup`,
`lookup_metadata`, and `lookup_metadata_images`.

Filters can be combined or used without a title:

```json
{
  "query": {
    "book": {
      "people": [{"name": "sample artist", "role": "Author"}],
      "series": [{"name": "sample series"}],
      "tags": [{"name": "full color"}],
      "pageKey": "2"
    }
  }
}
```

- Filters are combined with AND; native nhentai relation IDs take precedence
  over display names. Names containing spaces are quoted.
- Legacy `author` searches try the strict artist query first, then retry as an
  unscoped creator search so group-only creators can still be found.
- People IDs support `nhentai-artist:`, `nhentai-group:`, and
  `nhentai-character:`. Name-only people without a role use a broad text search.
  `Author` maps to artist, `Character` to character; custom roles `artist`,
  `group`, and `character` are also supported.
- Series names map to `parody:`; IDs use `nhentai-parody:`.
- Tag names map to `tag:`. Native IDs support `nhentai-tags:`,
  `nhentai-language:`, and `nhentai-category:`. Relation IDs are also accepted
  in filter names, as in legacy title queries.
- Unsupported roles, unresolved ID-only filters, and invalid filter literals
  make the search unsupported instead of silently dropping a constraint.
- Direct gallery IDs retain priority. If retrieval fails, the fallback name
  search includes the supplied filters.
- No language filter is added implicitly. Add `language:english` to
  `custom_search_params` when English-only results are wanted; custom search
  parameters and pagination apply to every search request.
- People metadata uses the flattened `PersonWithRoles` format, with canonical
  `Author` and `Character` types and the custom `group` type.

## Validation

Always finish the WASM release build before running tests:

```sh
cargo build --target wasm32-unknown-unknown --release
cargo test --test nhentai_parser_test --test retry_test --test lookup_test
```

Most tests in `lookup_test` contact nhentai.net and require network access.

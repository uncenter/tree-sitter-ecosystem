# zeta

Zed extensions analysis tool.

## Reference

### `count <CATEGORY>`

Count extensions by basic properties like type, manifest format, Git provider, and theme schema.

`<CATEGORY>` is one of:

- `by-type`: Count extensions by type (theme or language)
- `by-manifest`: Count extensions by manifest format (TOML or JSON)
- `by-git-provider`: Count extensions by Git provider (e.g. GitHub, GitLab)
- `by-theme-schema`: Count theme extensions by theme schema: V1, V2, or Invalid (no theme schema / unknown)

### `analyze <QUERY>`

Analyze extensions with various queries, mostly related to captures.

#### `captures-by-usage`

| Argument    | Value                         |
| ----------- | ----------------------------- |
| `<ORDER>`   | `asc[ending]`, `desc[ending]` |
| `[--limit]` | integer (default: 10)         |

Query the most (order: desc) or least (order: asc) used captures in language extensions.

#### captures-by-theme-support

| Argument    | Value                         |
| ----------- | ----------------------------- |
| `<ORDER>`   | `asc[ending]`, `desc[ending]` |
| `[--limit]` | integer (default: 10)         |

Query the most (order: desc) or least (order: asc) supported captures in theme extensions.

#### themes-supporting-capture

| Argument    | Value                    |
| ----------- | ------------------------ |
| `<CAPTURE>` | string (capture name)    |
| `[--count]` | boolean (default: false) |

Query the themes supporting a specific capture.

#### languages-using-capture

| Argument    | Value                    |
| ----------- | ------------------------ |
| `<CAPTURE>` | string (capture name)    |
| `[--count]` | boolean (default: false) |

Query the languages using a specific capture.

#### languages-by-theme-support

| Argument    | Value                         |
| ----------- | ----------------------------- |
| `<ORDER>`   | `asc[ending]`, `desc[ending]` |
| `[--limit]` | integer (default: 10)         |

Roughly score and rank languages by the depth (average number of themes supporting each capture used in a language) and breadth (number of themes supporting at least one capture) of theme support. The score is calculated as `(7 * depth / number of captures) + (3 * breadth)`. The best languages will have a high score (order: desc) and the worst languages will have a low score (order: asc)..

#### themes-by-capture-support

| Argument    | Value                         |
| ----------- | ----------------------------- |
| `<ORDER>`   | `asc[ending]`, `desc[ending]` |
| `[--limit]` | integer (default: 10)         |

Query the themes supporting the most (order: desc) or least (order: asc) _USED_ captures. Captures are considered used if they are used in any language extension.

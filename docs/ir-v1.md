# Axonix IR v1 (Draft)

Axonix IR is the stable contract between:

- developer-facing APIs (TypeScript builder, future DSL, visual editor)
- Rust execution runtime

## Why IR-first

Instead of making string DSL the core, Axonix keeps a typed IR as the canonical format.
Any API style compiles into IR.

## IR shape

```json
{
  "source": {
    "kind": {
      "Collection": {
        "name": "posts"
      }
    }
  },
  "transforms": [
    {
      "kind": {
        "Grid": {
          "columns": 3
        }
      }
    }
  ],
  "view": {
    "kind": {
      "Card": null
    }
  }
}
```

## Mapping examples

Pipeline string:

```text
Db.Stream("posts") |> layout.Grid(3) |> Card()
```

TypeScript builder:

```ts
from("posts").grid(3).card()
```

Both map to the same IR payload shown above.

## Current v1 stage support

- Source: `Db.Stream("collection")`, `from("collection")`
- Transform: `layout.Grid(n)`, `grid(n)`
- View: `Card()`, `view.Card()`, `view("CustomComponent")`

## Runtime output

`axonix-runtime` currently returns a `RenderPlan` stub:

- source collection
- layout strategy (grid + columns)
- target view component

This is the base for next steps:

- real data fetching
- diff/patch render payload
- SSR and stream responses


# {{APP_NAME}}

Generated with `create-axonyx` using the `docs` template.

{{AXONYX_RUNTIME_SOURCE_NOTE}}

This starter is focused on product docs, framework references, and example-driven documentation pages built directly in Axonyx.

The `docs` template already vendors `axonyx-ui`, syncs the Foundry CSS snapshot into `public/css/axonyx-ui`, and starts with the `silver` theme wired in `app/layout.ax`.

## Authoring Direction

This starter follows the recommended AX v2 authoring path:

- JSX-like `.ax` files
- `app/layout.ax` and `app/page.ax` route entrypoints
- nested docs pages under `app/.../page.ax`
- imports from `@axonyx/ui/...` for Foundry primitives

Older indentation-first `.ax` syntax still exists for compatibility, but new docs pages
should be authored in JSX-like `.ax`.

## Run

```bash
cargo run
```

This generates a first page preview at `target/axonyx-preview.html`.

If `cargo-axonyx` is installed, you can also run:

```bash
cargo ax run dev
```

That serves docs routes locally with nested layout composition, static assets from `public/`, and dev-time browser refresh.

Suggested first edit:

- open `app/getting-started/page.ax`
- update one section title or paragraph
- run `cargo ax run dev`
- reload `http://127.0.0.1:3000/getting-started`

## Starter Shape

- docs-first `app/page.ax`
- section pages for `getting-started`, `reference`, and `examples`
- reusable Foundry imports from `@axonyx/ui/...`
- static brand assets in `public/`
- room to add explicit `routes/` and `jobs/` later when the docs site needs APIs or automation

## Entry Routes

- `/`
- `/getting-started`
- `/reference`
- `/examples`

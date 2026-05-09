# {{APP_NAME}}

Generated with `create-axonyx` using the `docs` template.

{{AXONYX_RUNTIME_SOURCE_NOTE}}

This starter is focused on product docs, framework references, and example-driven documentation pages built directly in Axonyx.

The `docs` template already vendors `axonyx-ui`, serves Foundry CSS through `/_ax/pkg/axonyx-ui/index.css`, and starts with the `silver` theme wired in `app/layout.ax`.

## Authoring Direction

This starter follows the recommended AX v2 authoring path:

- JSX-like `.ax` files
- `app/layout.ax` and `app/page.ax` route entrypoints
- nested docs pages under `app/.../page.ax`
- imports from `@axonyx/ui/...` for Foundry primitives

Older indentation-first `.ax` syntax still exists for compatibility, but new docs pages
should be authored in JSX-like `.ax`.

## Build And Run

```bash
cargo ax check
cargo ax build --clean
cargo ax run dev
```

This validates `.ax` sources, regenerates `src/generated/backend.rs`, writes static HTML into `dist/`, and starts the route-aware dev server at `http://127.0.0.1:3000`.

Static build output:

```text
dist/
  index.html
  getting-started/index.html
  reference/index.html
  examples/index.html
```

Dynamic docs routes can be prerendered through `Axonyx.toml`:

```toml
[prerender]
routes = [
  { route = "/guides/:slug", params = [{ slug = "hello-axonyx" }] },
]
```

```bash
cargo ax run start --host 0.0.0.0 --port 3000
```

Use `run start` for a production-style process without the dev live-reload client. On hosted platforms, pass the platform `PORT` value to `--port`.

The older `cargo run` preview loop still generates `target/axonyx-preview.html`, but new docs work should prefer the `cargo ax` route-aware loop.

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

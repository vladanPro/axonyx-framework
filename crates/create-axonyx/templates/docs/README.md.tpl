# {{APP_NAME}}

Generated with `create-axonyx` using the `docs` template.

{{AXONYX_RUNTIME_SOURCE_NOTE}}

This starter is focused on product docs, framework references, and example-driven documentation pages built directly in Axonyx.

The `docs` template already depends on the published `axonyx-ui` Cargo package, activates Foundry assets with `use "@axonyx/ui"`, and starts with a preflight `silver` theme in `app/layout.ax`.

It includes a product-docs shell with:

- top navigation
- left docs sidebar
- theme switcher
- Foundry component showcase
- branded 404/error pages

## Authoring Direction

This starter follows the recommended AX v2 authoring path:

- JSX-like `.ax` files
- `app/layout.ax` and `app/page.ax` route entrypoints
- nested docs pages under `app/.../page.ax`
- imports from `@axonyx/ui/...` for Foundry primitives

Older indentation-first `.ax` syntax still exists for compatibility, but new docs pages
should be authored in JSX-like `.ax`.

## Build And Run

Start the local server:

```bash
cargo ax run dev
```

Then, in a second terminal, run the validation loop before sharing or deploying:

```bash
cargo ax check
cargo ax doctor
cargo ax test
cargo ax build --clean
```

The dev server runs at `http://127.0.0.1:3000`. The validation loop checks
`.ax` sources, verifies the starter routes, regenerates
`src/generated/backend.rs`, and writes static HTML into `dist/`.

This template is fully static. Add route actions later only when the documentation
site genuinely needs a server-side interaction.

## Fast QA

This starter includes `aegis.toml` for fast route checks before deploy.

Keep `cargo ax run dev` running, then in a second terminal run:

```bash
cargo install axonyx-aegis --force
cargo ax test
```

Static build output:

```text
dist/
  index.html
  getting-started/index.html
  components/index.html
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

Use `run start` for a production-style process without the dev live-reload
client. On hosted platforms, pass the platform `PORT` value to `--port`.

Suggested first edit:

- open `app/getting-started/page.ax`
- update one section title or paragraph
- run `cargo ax run dev`
- reload `http://127.0.0.1:3000/getting-started`

## Starter Shape

- docs-first `app/page.ax`
- section pages for `getting-started`, `components`, `reference`, and `examples`
- reusable Foundry imports from `@axonyx/ui/...`
- static brand assets in `public/`
- room to add explicit `routes/` and `jobs/` later when the docs site needs APIs or automation

## Entry Routes

- `/`
- `/getting-started`
- `/components`
- `/reference`
- `/examples`

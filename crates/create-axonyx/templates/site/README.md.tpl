# {{APP_NAME}}

Generated with `create-axonyx` using the `site` template.

{{AXONYX_RUNTIME_SOURCE_NOTE}}

This starter is oriented around a marketing or presentation site shape with a stronger landing page voice while keeping the same Axonyx runtime and backend authoring model.

The `site` template already depends on the published `axonyx-ui` Cargo package, serves Foundry CSS through `/_ax/pkg/axonyx-ui/index.css`, and wires the `silver` theme in `app/layout.ax`.

## Authoring Direction

This starter follows the recommended AX v2 authoring path:

- JSX-like `.ax` files
- `app/layout.ax` and `app/page.ax` route entrypoints
- nested app routes for site sections
- imports from `@/components/...` and `@axonyx/ui/...`

Older indentation-first `.ax` syntax still exists for compatibility, but new site pages
should be authored in JSX-like `.ax`.

## Build And Run

```bash
cargo ax check
cargo ax doctor
cargo ax build --clean
cargo ax run dev
```

This validates `.ax` sources, regenerates `src/generated/backend.rs`, writes static HTML into `dist/`, and starts the route-aware dev server at `http://127.0.0.1:3000`.

Static build output:

```text
dist/
  index.html
  posts/index.html
  ...
```

Dynamic routes can be prerendered through `Axonyx.toml`:

```toml
[prerender]
routes = [
  { route = "/posts/:slug", params = [{ slug = "hello-axonyx" }, { slug = "foundry-ui" }] },
]
```

```bash
cargo ax run start --host 0.0.0.0 --port 3000
```

Use `run start` for a production-style process without the dev live-reload client. On hosted platforms, pass the platform `PORT` value to `--port`.

The older `cargo run` preview loop still generates `target/axonyx-preview.html`, but new site work should prefer the `cargo ax` route-aware loop.

Suggested first edit:

- open `app/page.ax`
- change hero copy or card titles
- run `cargo ax run dev`
- reload `http://127.0.0.1:3000`

## Starter Shape

- landing-focused `app/page.ax`
- featured posts section in `app/posts/page.ax`
- reusable Foundry imports from `@axonyx/ui/...`
- same backend route/loader/action/job draft structure as the minimal template

## Env

Copy `.env.example` to `.env` and set your runtime values.

- `AX_PUBLIC_APP_NAME`
- `AX_SECRET_DB_DIALECT`
- `AX_SECRET_DB_TRANSPORT`
- `AX_SECRET_DB_URL`
- `AX_PUBLIC_DATA_API_URL`
- `AX_SECRET_DATA_API_KEY`

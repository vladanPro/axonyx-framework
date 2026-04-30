# {{APP_NAME}}

Generated with `create-axonyx` using the `site` template.

{{AXONYX_RUNTIME_SOURCE_NOTE}}

This starter is oriented around a marketing or presentation site shape with a stronger landing page voice while keeping the same Axonyx runtime and backend authoring model.

The `site` template already vendors `axonyx-ui` into `vendor/axonyx-ui`, syncs the Foundry CSS into `public/css/axonyx-ui`, and wires the `silver` theme in `app/layout.ax`.

## Authoring Direction

This starter follows the recommended AX v2 authoring path:

- JSX-like `.ax` files
- `app/layout.ax` and `app/page.ax` route entrypoints
- nested app routes for site sections
- imports from `@/components/...` and `@axonyx/ui/...`

Older indentation-first `.ax` syntax still exists for compatibility, but new site pages
should be authored in JSX-like `.ax`.

## Run

```bash
cargo run
```

This generates a first page preview at `target/axonyx-preview.html`.

The preview composes `app/layout.ax` around `app/page.ax`, so the first loop already follows the intended Axonyx app structure.

If `cargo-axonyx` is installed, you can also run:

```bash
cargo ax run dev
```

That serves the current app routes locally with nested layout composition and dev-time browser refresh. The older `cargo axonyx dev` path can still stay as a compatibility alias.

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

# {{APP_NAME}}

Generated with `create-axonyx`.

{{AXONYX_RUNTIME_SOURCE_NOTE}}

If you selected the registry runtime source before the package is published, switch to `--runtime-source git` or `--runtime-source path` until the first public release is available.

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
  ...
```

Dynamic routes can be prerendered through `Axonyx.toml`:

```toml
[prerender]
routes = [
  { route = "/posts/:slug", params = [{ slug = "hello-axonyx" }] },
]
```

```bash
cargo ax run start --host 0.0.0.0 --port 3000
```

Use `run start` for a production-style process without the dev live-reload client. On hosted platforms, pass the platform `PORT` value to `--port`.

The older `cargo run` preview loop still generates `target/axonyx-preview.html`, but new app work should prefer the `cargo ax` route-aware loop.

## Authoring Direction

This starter follows the recommended AX v2 authoring path:

- JSX-like `.ax` files
- `app/layout.ax` and `app/page.ax` route entrypoints
- optional route-local `loader.ax` and `actions.ax`

Older indentation-first `.ax` syntax still exists in the framework for compatibility,
but new app work should prefer JSX-like `.ax`.

Use it as the smallest "Hello Axonyx" loop:

1. edit `app/layout.ax` or `app/page.ax`
2. run `cargo ax run dev`
3. reload `http://127.0.0.1:3000`

Suggested first edit:

- open `app/page.ax`
- change one heading or `Copy` body
- run `cargo ax run dev`
- reload `http://127.0.0.1:3000`

## Env

Copy `.env.example` to `.env` and set your runtime values.

Axonyx backend env convention:

- `Runtime.Env.public.app_name` -> `AX_PUBLIC_APP_NAME`
- `Runtime.Env.public.app_env` -> `AX_PUBLIC_APP_ENV`
- `Runtime.Env.secret.db_driver` -> `AX_SECRET_DB_DIALECT` with fallback to `AX_SECRET_DB_DRIVER`
- `Runtime.Env.secret.db_url` -> `AX_SECRET_DB_URL`

Recommended data config:

- `AX_SECRET_DB_DIALECT=postgres|mysql|sqlite|memory`
- `AX_SECRET_DB_TRANSPORT=direct|api`
- transport defaults to `direct` when omitted
- dialect defaults to `postgres` when omitted

Database adapter convention:

- `postgres` -> `postgres://...`
- `mysql` -> `mysql://...`
- `sqlite` -> `file:local.db` or a local sqlite path
- `memory` -> in-memory adapter for local prototyping

API transport convention:

- `AX_PUBLIC_DATA_API_URL=https://...`
- `AX_SECRET_DATA_API_KEY=...`
- provider-specific aliases can map into the same config shape

## Axonyx Structure

```text
app/
  layout.ax
  page.ax
  posts/
    page.ax
    loader.ax
    actions.ax
routes/
  api/
    posts.ax
jobs/
  digest.ax
src/
  generated/
  domain/
  db/
```

## Entry Files

- `app/page.ax`
- `app/layout.ax`
- `app/posts/loader.ax`
- `app/posts/actions.ax`
- `routes/api/posts.ax`

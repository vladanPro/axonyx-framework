# {{APP_NAME}}

Generated with `create-axonyx`.

{{AXONYX_RUNTIME_SOURCE_NOTE}}

## Build And Run

```bash
cargo ax check
cargo ax actions
cargo ax doctor
cargo ax build --clean
cargo ax run dev
```

This validates `.ax` sources, regenerates `src/generated/backend.rs`, writes static HTML into `dist/`, and starts the route-aware dev server at `http://127.0.0.1:3000`.

`cargo ax actions` prints route-local action contracts from `app/**/actions.ax`.
In this template it shows the `CreatePost` inputs, including the optional
`status?: string = "draft"` field used by the `ActionForm` on `/posts`.

## Fast QA

This starter includes `aegis.toml` for fast route checks before deploy.

Keep `cargo ax run dev` running, then in a second terminal run:

```bash
cargo install axonyx-aegis --force
aegis fast --config aegis.toml
```

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
- action-backed form demo with `ActionForm`, `ActionStatus`, typed inputs, and
  defaulted optional action fields

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
- `Auth.signedSession` -> `AX_SECRET_SESSION_KEY`

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

Auth convention:

- `Auth.bearer` reads `Authorization: Bearer ...`
- `Auth.session` reads the plain `session` cookie
- `Auth.signedSession` verifies the `session` cookie with `AX_SECRET_SESSION_KEY`
- `cargo ax check` reports `axonyx-auth-secret` when a route uses signed sessions without that secret

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

## Typed API Route Example

`routes/api/posts.ax` includes a typed `POST /api/posts` example:

```ax
route POST "/api/posts"
  input:
    title: string
    excerpt?: string = ""
    featured?: bool = false

  return json(input.title)
```

Try it with JSON:

```bash
curl -X POST http://127.0.0.1:3000/api/posts \
  -H "Content-Type: application/json" \
  -d "{\"title\":\"Hello Axonyx\",\"featured\":true}"
```

`cargo ax check` validates the `input:` block before build, including missing sections, duplicate fields, and unsupported route input types.

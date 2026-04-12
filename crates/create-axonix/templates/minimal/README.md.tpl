# {{APP_NAME}}

Generated with `create-axonix`.

{{AXONIX_RUNTIME_SOURCE_NOTE}}

If you selected the registry runtime source before the package is published, switch to `--runtime-source git` or `--runtime-source path` until the first public release is available.

## Run

```bash
cargo run
```

## Env

Copy `.env.example` to `.env` and set your runtime values.

Axonix backend env convention:

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

## Axonix Structure

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

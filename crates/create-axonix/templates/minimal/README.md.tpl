# {{APP_NAME}}

Generated with `create-axonix`.

## Run

```bash
cargo run
```

## Env

Copy `.env.example` to `.env` and set your runtime values.

Axonix backend env convention:

- `Runtime.Env.public.app_name` -> `AX_PUBLIC_APP_NAME`
- `Runtime.Env.public.app_env` -> `AX_PUBLIC_APP_ENV`
- `Runtime.Env.secret.db_driver` -> `AX_SECRET_DB_DRIVER`
- `Runtime.Env.secret.db_url` -> `AX_SECRET_DB_URL`

Database adapter convention:

- `postgres` -> `postgres://...`
- `mysql` -> `mysql://...`
- `sqlite` -> `file:local.db` or a local sqlite path
- `memory` -> in-memory adapter for local prototyping

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

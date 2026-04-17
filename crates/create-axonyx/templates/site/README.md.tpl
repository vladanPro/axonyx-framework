# {{APP_NAME}}

Generated with `create-axonyx` using the `site` template.

{{AXONYX_RUNTIME_SOURCE_NOTE}}

This starter is oriented around a marketing or presentation site shape with a stronger landing page voice while keeping the same Axonyx runtime and backend authoring model.

## Run

```bash
cargo run
```

## Starter Shape

- landing-focused `app/page.ax`
- featured posts section in `app/posts/page.ax`
- same backend route/loader/action/job draft structure as the minimal template

## Env

Copy `.env.example` to `.env` and set your runtime values.

- `AX_PUBLIC_APP_NAME`
- `AX_SECRET_DB_DIALECT`
- `AX_SECRET_DB_TRANSPORT`
- `AX_SECRET_DB_URL`
- `AX_PUBLIC_DATA_API_URL`
- `AX_SECRET_DATA_API_KEY`

# {{APP_NAME}}

Generated with `create-axonyx` using the `site` template.

{{AXONYX_RUNTIME_SOURCE_NOTE}}

This starter is oriented around a marketing or presentation site shape with a stronger landing page voice while keeping the same Axonyx runtime and backend authoring model.

The `site` template already depends on the published `axonyx-ui` Cargo package, activates Foundry assets with `use "@axonyx/ui"`, and wires the `silver` theme in `app/layout.ax`.

## Authoring Direction

This starter follows the recommended AX v2 authoring path:

- JSX-like `.ax` files
- `app/layout.ax` and `app/page.ax` route entrypoints
- nested app routes for site sections
- imports from `@/components/...` and `@axonyx/ui/...`

Older indentation-first `.ax` syntax still exists for compatibility, but new site pages
should be authored in JSX-like `.ax`.

## Build And Run

Start the local server first:

```bash
cargo ax run dev
```

Then, in a second terminal, run the validation loop before sharing or deploying:

```bash
cargo ax check
cargo ax actions
cargo ax doctor
cargo ax build --clean
```

The dev server runs at `http://127.0.0.1:3000`. The validation loop checks `.ax`
sources, regenerates `src/generated/backend.rs`, and writes static HTML into
`dist/`.

`cargo ax actions` prints the route-local action contracts from `app/**/actions.ax`.
In this template it shows the `CreatePost` inputs, including the optional
`status?: string = "draft"` default used by the `ActionForm` on `/posts`.

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
- action-backed form demo with `ActionForm`, `ActionStatus`, typed inputs, and
  defaulted optional action fields
- typed `POST /api/posts` route input example in `routes/api/posts.ax`
- reusable Foundry imports from `@axonyx/ui/...`
- same backend route/loader/action/job draft structure as the minimal template

## Typed API Route Example

`routes/api/posts.ax` includes a typed body example:

```ax
route POST "/api/posts"
  input:
    title: string
    excerpt?: string = ""
    featured?: bool = false

  return json(input.title)
```

This reads form data first, then JSON body fields, and gives clear `cargo ax check` diagnostics when the route input shape is invalid.

## Env

Copy `.env.example` to `.env` and set your runtime values.

- `AX_PUBLIC_APP_NAME`
- `AX_SECRET_DB_DIALECT`
- `AX_SECRET_DB_TRANSPORT`
- `AX_SECRET_DB_URL`
- `AX_PUBLIC_DATA_API_URL`
- `AX_SECRET_DATA_API_KEY`

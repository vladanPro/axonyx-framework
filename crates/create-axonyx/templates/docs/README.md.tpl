# {{APP_NAME}}

Generated with `create-axonyx` using the `docs` template.

{{AXONYX_RUNTIME_SOURCE_NOTE}}

This starter is focused on product docs, framework references, and example-driven documentation pages built directly in Axonyx.

The `docs` template already vendors `axonyx-ui`, syncs the Foundry CSS snapshot into `public/css/axonyx-ui`, and starts with the `silver` theme wired in `app/layout.ax`.

## Run

```bash
cargo run
```

This generates a first page preview at `target/axonyx-preview.html`.

If `cargo-axonyx` is installed, you can also run:

```bash
cargo ax run dev
```

That serves docs routes locally with nested layout composition, static assets from `public/`, and dev-time browser refresh.

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

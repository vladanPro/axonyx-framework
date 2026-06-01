# cargo-axonyx

Cargo helper commands for Axonyx apps.

This package installs the `cargo ax` and `cargo axonyx` subcommands used for the Axonyx local development loop.

## Install

```bash
cargo install cargo-axonyx
```

## Commands

```bash
cargo ax actions
cargo ax check
cargo ax content
cargo ax doctor
cargo ax melt
cargo ax build --clean
cargo ax run dev
cargo ax run dev --transport tokio
cargo ax stream
cargo ax test
```

## Typical Flow

```bash
create-axonyx my-site --yes --template site
cd my-site
cargo ax doctor
cargo ax run dev
```

The default dev/start server still uses the stable `std` transport. The Tokio
transport is available as the async preview path:

```bash
cargo ax run dev --transport tokio
cargo ax run start --host 0.0.0.0 --port 3000 --transport tokio
```

For hosted starts, `cargo ax run start` reads the platform `PORT` environment
variable when `--port` is omitted:

```bash
PORT=3000 cargo ax run start --host 0.0.0.0
```

`cargo ax run start` also performs the same `.ax` source diagnostics preflight
as `cargo ax build`, so production starts fail before binding a port when a page
has import, syntax, or route-source errors.

The server accepts request bodies up to `1mb` by default. Apps can change this
in `Axonyx.toml`:

```toml
[server]
max_body_bytes = "2mb"
```

Axonyx keeps the authoring model synchronous and structured; the runtime decides
whether the request path uses the std transport, Tokio tasks, streaming, or a
future worker layer behind the scenes.

`cargo ax doctor` verifies the app structure, runtime dependency, server page
streaming mode, Axonyx UI package resolution, stylesheet wiring, and `.ax`
source diagnostics. It also verifies that the Melt project graph can be
collected. Text output summarizes the public framework layers: Axonyx Pages,
Axonyx Server, Axonyx State, Axonyx Foundry, and Axonyx Melt.

`cargo ax melt` prints the first project graph snapshot across those layers:
routes, API routes, actions, state declarations, content collections, and source
diagnostics. Use `cargo ax melt --format json` when docs, CI, or future tooling
need the same graph as structured data. Use `cargo ax melt --check` as a short
CI preflight that verifies the graph can be collected without diagnostics.

`cargo ax content` reads `[content.collections]` from `Axonyx.toml` and prints the current Melt-time content manifest.
`cargo ax build` uses the Melt graph as its diagnostics preflight, writes the content manifest to `dist/_ax/content/manifest.json` when collections are configured, and always writes the Melt graph to `dist/_ax/melt/graph.json`.

`cargo ax routes` prints page/API routes and the current server
`stream_pages` setting. JSON output is a report object with `stream_pages` and
`routes`.

`cargo ax actions` prints route-local action contracts from `app/**/actions.ax`,
including input type, optional markers, and default values. JSON output is meant
for docs, editor tooling, and future endpoint/schema discovery. Use
`cargo ax actions --route /posts` or `cargo ax actions --name CreatePost` to
filter larger apps. Use `cargo ax actions --name CreatePost --schema` to print a
copy/paste `.ax` input type declaration.

`cargo ax stream` starts the dev server with a visible streaming probe URL. It is
not a replacement for `cargo ax run dev`; it exists to test Axonyx chunked
response support while UI streaming is being shaped.

`cargo ax test` delegates fast route QA to Aegis when an `aegis.toml` file is
present. Keep `cargo ax run dev` running in one terminal, then run:

```bash
cargo install axonyx-aegis --force
cargo ax test
cargo ax test --format json --fail-fast false
```

```bash
cargo ax stream
# open http://127.0.0.1:3000/__axonyx/stream
# open http://127.0.0.1:3000/__axonyx/stream/html
# open http://127.0.0.1:3000/__axonyx/events
```

`/__axonyx/events` is the first Server-Sent Events probe. It uses the shared
`AxHttpResponse::sse_events` runtime contract and is intended to become the
foundation for live state patch streams, CMS events, and build/runtime signals.

## Package Model

Generated apps depend on published Cargo packages by default:

```toml
axonyx-runtime = "0.1.14"
axonyx-ui = "0.0.40"
```

Local path and package override flows are still supported for framework development and UI dogfooding.

## Architecture Direction

Axonyx is not intended to be another React wrapper. It is aiming to become a Rust-first framework for sites, docs, CMS products, and full-stack apps where HTML-first output, explicit compilation, low JavaScript, and packageable themes/templates matter more than React compatibility.

Read the architecture comparison:

https://github.com/vladanPro/axonyx-framework/wiki/Next.js-vs-Axonyx

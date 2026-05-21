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
cargo ax build --clean
cargo ax run dev
cargo ax run dev --transport tokio
cargo ax stream
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

Axonyx keeps the authoring model synchronous and structured; the runtime decides
whether the request path uses the std transport, Tokio tasks, streaming, or a
future worker layer behind the scenes.

`cargo ax doctor` verifies the app structure, runtime dependency, server page
streaming mode, Axonyx UI package resolution, stylesheet wiring, and `.ax`
source diagnostics.

`cargo ax content` reads `[content.collections]` from `Axonyx.toml` and prints the current Melt-time content manifest.
`cargo ax build` writes the same manifest to `dist/_ax/content/manifest.json` when collections are configured.

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
axonyx-runtime = "0.1.9"
axonyx-ui = "0.0.34"
```

Local path and package override flows are still supported for framework development and UI dogfooding.

## Architecture Direction

Axonyx is not intended to be another React wrapper. It is aiming to become a Rust-first framework for sites, docs, CMS products, and full-stack apps where HTML-first output, explicit compilation, low JavaScript, and packageable themes/templates matter more than React compatibility.

Read the architecture comparison:

https://github.com/vladanPro/axonyx-framework/wiki/Next.js-vs-Axonyx

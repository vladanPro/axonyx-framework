# Working on Axonyx

- Start with [Getting Started](docs/getting-started.md) and the current `crates/create-axonyx/templates` sources. Historical release notes may show older syntax. For published apps, verify examples against the app's pinned CLI and dependencies.
- Pages and components are `.asx`. Write `page Name() { ... return ASX { ... } }`. Route-local `loader.ax` exposes typed server queries; bind their results with `data posts = loadPosts()` rather than fetching the app's own API from the browser.
- Server code is `.ax`. Keep validation and mutations in `actions.ax`, with typed parameters and values read through `input`. Put reusable domain logic in `domain.ax` or explicit imported modules. Keep SQL parameters separate from the query string.
- Page state belongs to the page; server-backed changes belong to actions. Do not assume every expression runs in WASM or move session data and secrets into browser state.
- `Native.call` invokes registered application Rust code on the server. Native hooks need `cargo ax build --compiled` and `cargo ax run start --compiled`. Do not hand-edit generated Rust or `dist/_ax` metadata.
- From the app root, run `cargo ax check` and `cargo ax doctor --deny-warnings`; then build and verify the relevant route over HTTP. Use a disposable database for mutating tests. With a matching CLI installed, run `pwsh -File scripts/smoke-production-server.ps1 -Template minimal` from the framework root to exercise the first-posts starter.

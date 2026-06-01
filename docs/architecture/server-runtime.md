# Axonyx Server Runtime

Axonyx keeps the user-facing server model declarative even if the internal
implementation moves from the current beta `std::net` loop to Hyper/Tokio.

The stable seam is `axonyx_runtime::server_prelude`:

- `AxServerConfig` owns host, port, and server mode.
- `AxServerMode` distinguishes `dev` from `start`.
- `AxHttpRequest` and `AxHttpResponse` describe framework-level request and
  response values before they are adapted to a concrete transport.
- `AxBody` describes either fixed bytes or streaming chunks.
- `AxServer` is the future trait for concrete server implementations.

This gives the CLI and runtime one small contract to share:

```rust
use axonyx_runtime::server_prelude::{AxServerConfig, AxServerMode};

let config = AxServerConfig::new("127.0.0.1", 3000, AxServerMode::Dev);
let bind = config.bind_addr();
```

## Why This Exists Before Tokio

The current beta server is intentionally simple. It can serve page routes,
public assets, package assets, route-local actions, and backend route previews.
That is enough for the first site loop.

It also has a streaming probe path:

```text
cargo ax stream
GET /__axonyx/stream
GET /__axonyx/stream/html
```

The probe uses `Transfer-Encoding: chunked` when the response body is
`AxBody::Chunks`. This proves the transport path before route rendering itself
becomes streaming-aware.

Page routes can also opt into the same response path during development:

```toml
[server]
stream_pages = true
```

or per request:

```text
GET /?__ax_stream=1
```

This still streams the already-rendered HTML in coarse shell/body/end chunks.
It is a transport milestone, not yet a full `<Await>` boundary implementation.

Request reads use a bounded timeout so slow or broken clients cannot hold a
connection forever:

```toml
[server]
request_timeout_seconds = 2
```

The same timeout is respected by the standard transport and the Tokio preview
transport. `cargo ax doctor` reports the resolved value and flags invalid
configuration before the server starts.

Tokio/Hyper should replace the transport underneath, not the framework shape
above it. The developer should still write:

```text
cargo ax run dev
cargo ax run start --host 0.0.0.0 --port 3000
cargo ax run start --production-server --host 0.0.0.0 --port 3000
```

`--production-server` is the user-facing preview flag for the future production
server path. It currently selects the Tokio transport underneath while
preserving the same route, loader, action, page, and state model. The Tokio
transport also installs a Ctrl+C shutdown listener so the accept loop can exit
cleanly during local stops and hosted deploy restarts. After the listener stops
accepting new connections, Axonyx waits a short grace period for active Tokio
connection tasks to finish before returning from the server.

Deployment checks should point at that same path:

```text
cargo ax doctor --deploy render
```

For Render, the recommended start command is:

```text
cargo ax run start --production-server --host 0.0.0.0 --port $PORT
```

and the app should still be authored through:

- `app/**/page.ax`
- nested `layout.ax`
- route-local `loader.ax`
- route-local `actions.ax`
- `routes/**/*.ax` API handlers
- `jobs/**/*.ax` background jobs

## Next Server Milestones

1. Add native chunked UI streaming for route rendering.
2. Keep expanding the production adapter behind `--production-server`.
3. Lower future `<Await>` or stream boundaries into `AxBody::Chunks`.
4. Keep structured async in Axonyx authoring through loaders, actions, jobs, and
   future `<Await>` boundaries instead of exposing promise-style timing.

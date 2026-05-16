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

Tokio/Hyper should replace the transport underneath, not the framework shape
above it. The developer should still write:

```text
cargo ax run dev
cargo ax run start --host 0.0.0.0 --port 3000
```

and the app should still be authored through:

- `app/**/page.ax`
- nested `layout.ax`
- route-local `loader.ax`
- route-local `actions.ax`
- `routes/**/*.ax` API handlers
- `jobs/**/*.ax` background jobs

## Next Server Milestones

1. Add a `TokioAxServer` or `HyperAxServer` adapter once async streaming starts.
2. Add native chunked UI streaming for route rendering.
3. Lower future `<Await>` or stream boundaries into `AxBody::Chunks`.
4. Keep structured async in Axonyx authoring through loaders, actions, jobs, and
   future `<Await>` boundaries instead of exposing promise-style timing.

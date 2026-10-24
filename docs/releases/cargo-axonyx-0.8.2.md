# cargo-axonyx 0.8.2

This CLI-only patch extends compiled production error boundaries to nested
routes. The closest applicable `not-found.asx` or `error.asx` is selected for
the request path, including dynamic route segments. Boundary rendering retains
the matching layouts, imports, UI assets and theme. The existing safe fallback,
access-denial separation, no-store response and empty HEAD body remain intact.

The runtime, core, scaffolder and LSP remain on 0.8.0.

Release gate: all five framework CI jobs, package verification against the
published 0.8.0 dependencies, then a fresh registry CLI install and compiled
HTTP smoke for nested 404 and 500 boundaries.

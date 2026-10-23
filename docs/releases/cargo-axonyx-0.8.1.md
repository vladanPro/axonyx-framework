# cargo-axonyx 0.8.1

This CLI-only patch brings global `app/not-found.asx` and `app/error.asx`
boundaries to the compiled production server. The root layout, imports, UI
assets and theme configuration are included. Missing routes and route-loader
not-found errors render the 404 boundary; unexpected page-render errors render
the 500 boundary with a safe fallback if boundary rendering itself fails.
Access-denial statuses remain separate, and HEAD responses have no body.

The runtime, core, scaffolder and LSP stay on 0.8.0. Nested route boundaries
are not part of this patch and remain tracked in issue #381.

Release gate: all five framework CI jobs, package verification against the
published 0.8.0 dependencies, then a fresh registry CLI install and compiled
HTTP smoke. Do not merge Legura's boundary adoption before the published CLI
passes that acceptance check.

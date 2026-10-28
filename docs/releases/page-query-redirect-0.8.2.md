# Page Query Redirect - Cargo Patch Release

Published on 2026-10-27: `axonyx-core` and `axonyx-runtime` 0.8.2,
`create-axonyx` 0.8.3, and `cargo-axonyx` 0.8.4. `axonyx-lsp` remains 0.8.1;
UI, npm, Docker and deployed applications are unchanged.

Query `require ... else redirect("/local-path")` now carries a typed control-flow
signal into preview and compiled page responses. Targets must be local absolute
paths, and statuses are limited to 302, 303, 307 and 308. Query errors in API,
action and data contexts do not become browser redirects. Redirect-bearing
page queries stay runtime-rendered even when the guard depends on SQLite
installation state rather than a session.

Runtime PR #286 and framework PR #401 passed their dev-to-main CI gates and
merged to main. Each crate passed package verification against published
dependencies before upload. The registry-only Beta 1 smoke then installed
the exact published tools into an isolated root, scaffolded site/blog/docs,
passed check and strict doctor, built each template, and served the site from
its compiled production binary. It verified a no-store HTTP 302 from a guarded
query, page routes, CSS assets and 404. No local path/git package was used.

# Page Query Redirect - Patch Candidate

Preparation only; these versions are not published yet. This candidate pairs
`axonyx-core` and `axonyx-runtime` 0.8.2 with `create-axonyx` 0.8.3 and
`cargo-axonyx` 0.8.4. `axonyx-lsp` remains 0.8.1; UI, npm, Docker and deployed
applications are unchanged.

Query `require ... else redirect("/local-path")` now carries a typed control-flow
signal into preview and compiled page responses. Targets must be local absolute
paths, and statuses are limited to 302, 303, 307 and 308. Query errors in API,
action and data contexts do not become browser redirects. Redirect-bearing
page queries stay runtime-rendered even when the guard depends on SQLite
installation state rather than a session.

Release gate: green runtime and framework dev-to-main PRs, package verification
after each dependency reaches crates.io, then publish core, runtime,
scaffolder and CLI in that order. Keep the registry-only Beta 1 smoke pinned to
the currently published versions until all four are available. Afterwards,
update the public install instructions and run a fresh registry-only compiled
HTTP proof before tagging or closing framework issue #326.

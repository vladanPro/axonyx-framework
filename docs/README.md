# Axonyx Docs

This folder is the first structured documentation layer for the Axonyx framework workspace.

## Start Here

- [Overview](./overview.md)
- [AX v2 Authoring](./ax-v2-authoring.md)
- [v0.1 Stable Surface](./v0-1-stable-surface.md)
- [Getting Started](./getting-started.md)
- [Proof App Checklist](./proof-app-checklist.md)
- [Runtime Sources](./runtime-sources.md)
- [Release Runbook](./release-runbook.md)
- [Templates](./templates.md)
- [Backend Authoring](./backend-authoring.md)

## Draft References

- [Structured Async In Axonyx](./architecture/structured-async.md)
- [Server Runtime](./architecture/server-runtime.md)
- [Reactivity v1](./reactivity-v1.md)
- [IR v1](./ir-v1.md)

The files above are deeper design drafts, while the docs in this index should become the developer-facing entrypoint.

For new work, use JSX-like `.asx` for pages, layouts, and components; use `.ax`
for loaders, actions, routes, jobs, and domain helpers. Older indentation-first
frontend `.ax` syntax is migration compatibility, not a second recommended path.

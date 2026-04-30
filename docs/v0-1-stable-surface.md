# v0.1 Stable Surface

This document defines the practical stability target for Axonyx framework `v0.1`.

The goal is not "every feature is done". The goal is that one clear path works reliably
for real app work.

## Scope

The `v0.1` stable surface should guarantee:

- one canonical AX v2 authoring flow
- one canonical scaffold-to-browser flow
- one canonical backend generation flow
- predictable diagnostics for common authoring errors

## Authoring Contract

Recommended and expected path:

- JSX-like `.ax` syntax
- top-level `page <Name>`
- `app/layout.ax` and `app/page.ax` route entrypoints
- nested `app/**/page.ax` for route trees
- local imports via `@/components/...`
- UI package imports via `@axonyx/ui/...`
- `Head` tags: `Title`, `Theme`, `Meta`, `Link`, `Script`

Legacy indentation-first syntax can remain for compatibility, but it is not the preferred
target for new examples or starter guidance.

## Runtime/CLI Contract

`create-axonyx` + `cargo ax run dev` should be enough for first success.

Expected behaviors:

- scaffolded app compiles with default settings
- route-aware dev server starts on local host
- nested layout composition works
- dynamic params and query context resolve for page routes
- `cargo ax build` regenerates backend module from `.ax` backend sources

## Import and Override Contract

Expected behaviors:

- import resolution for local app components works
- import resolution for `@axonyx/ui/...` works
- component override mapping from `Axonyx.toml` works
- package override mapping from `Axonyx.toml` works
- import-chain failures point to the failing source path and line

## Diagnostics Contract

For common errors, diagnostics should include:

- stable code family (parse/import/backend-parse)
- actionable message
- line number

Minimum expectation for `v0.1` is predictable guidance for:

- malformed AX v2 tags
- missing imports
- import cycles
- invalid override target sources

## Not Required For v0.1

These can remain out of scope:

- full client-side interactivity system
- advanced hydration semantics
- complete CMS feature set
- database abstraction completeness
- every future template type

## Release Readiness Signal

Axonyx framework core should be considered "v0.1-ready" when:

1. the proof app checklist passes on a clean machine
2. docs consistently point to AX v2 as the primary path
3. starter templates reinforce the same path
4. no critical regressions in scaffold, route render, or backend generation loops

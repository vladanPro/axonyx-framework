# Axonyx Overview

Axonyx is a Rust-first full-stack framework direction built around a few strong ideas:

- single-binary thinking
- authoring-first UX
- Algebraic UI as a first-class concept
- a runtime story that can move from local monorepo development to Git and eventually registry releases

## Workspace Shape

Today the broader Axonyx workspace is split into a few roles:

- `axonyx-framework`
  - main experimental framework repo
  - scaffold CLI, local dev CLI, templates, docs direction, and a pinned runtime submodule
- `axonyx-runtime`
  - standalone runtime workspace repo
  - parser, lowering, macros, runtime contract, and long-term package story for generated apps

## Main Building Blocks

- `create-axonyx`
  - scaffolds new Axonyx applications
- `cargo-axonyx`
  - local `cargo ax ...` developer workflow
- `axonyx-core`, `axonyx-runtime`, `axonyx-macros`
  - come from the `vendor/axonyx-runtime` git submodule and are imported into this repo by path

## Current Direction

The current goal is not to ship every feature at once.

The goal is to make these three developer stories feel real:

1. generate an app
2. depend on a stable runtime package
3. author UI and backend behavior in Axonyx-native shapes

## Recommended Authoring Path

For framework work today, the recommended Axonyx path is:

- JSX-like `.ax` files
- `app/layout.ax` and `app/page.ax` as the main route entrypoints
- nested app routes for site structure
- `loader.ax` and `actions.ax` for route-local backend behavior
- imports from local components and `@axonyx/ui`

That is the path already exercised by the current CLI, route rendering flow, and import
resolution logic.

## Legacy vs Current

Axonyx still contains older indentation-first `.ax` parsing because it is useful for
compatibility, reference material, and transition work.

But if we are deciding how to write new pages, new examples, or new starter templates,
the preferred answer should be the JSX-like `.ax` direction rather than the indentation
syntax.

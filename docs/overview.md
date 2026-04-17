# Axonyx Overview

Axonyx is a Rust-first full-stack framework direction built around a few strong ideas:

- single-binary thinking
- authoring-first UX
- Algebraic UI as a first-class concept
- a runtime story that can move from local monorepo development to Git and eventually registry releases

## Workspace Shape

Today the broader Axonyx workspace is split into a few roles:

- `axonix-framework`
  - main experimental framework repo
  - parser, lowering, scaffold CLI, and docs direction
- `axonyx-runtime`
  - standalone runtime workspace repo
  - intended long-term package story for generated apps

## Main Building Blocks

- `create-axonyx`
  - scaffolds new Axonyx applications
- `axonyx-core`
  - parser, lowering, SQL draft compiler, and authoring model
- `axonyx-runtime`
  - runtime contract, env loading, and backend execution planning
- `axonyx-macros`
  - ergonomic procedural macros

## Current Direction

The current goal is not to ship every feature at once.

The goal is to make these three developer stories feel real:

1. generate an app
2. depend on a stable runtime package
3. author UI and backend behavior in Axonyx-native shapes

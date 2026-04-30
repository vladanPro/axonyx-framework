# AX v2 Authoring

This document describes the recommended authoring path for Axonyx today.

## Current Recommendation

When writing new Axonyx pages and components, prefer JSX-like `.ax` files.

That means:

- `app/layout.ax` for shared layout structure
- `app/page.ax` for the root page
- nested `app/.../page.ax` files for route trees
- optional `loader.ax` and `actions.ax` files for route-local backend behavior
- component imports from `@/components/...` or `@axonyx/ui/...`

## Example

```ax
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"
import { SiteHero } from "@/components/SiteHero.ax"

page Home

<Head>
  <Title>Axonyx</Title>
  <Theme>silver</Theme>
  <Link rel="stylesheet" href="/css/axonyx-ui/index.css" />
</Head>

<SiteHero />

<SectionCard title="Hello Axonyx">
  <Copy>Rust-first authoring with a cleaner page shape.</Copy>
</SectionCard>
```

## What This Path Already Covers

The current framework/runtime path already includes support for:

- `import { ... } from "..."`
- top-level `page`
- paired tags and self-closing tags
- text and expression children
- fragment shorthand
- local component declarations
- `Head` tags such as `Title`, `Theme`, `Meta`, `Link`, and `Script`
- local component imports from app code
- package component imports from `@axonyx/ui`
- route params and query-aware page flows

## Legacy Syntax

Older indentation-first `.ax` syntax still exists in the repository and runtime stack.

That syntax should be treated as legacy or compatibility material. It is still useful for
older tests and transition work, but it is not the best default for new authoring examples.

## Practical Rule

If we are creating:

- a new starter
- a new docs example
- a new site page
- a new UI component import example

we should default to JSX-like `.ax`.

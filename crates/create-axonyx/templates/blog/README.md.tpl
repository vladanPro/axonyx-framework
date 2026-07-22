# {{APP_NAME}}

Generated with `create-axonyx` using the static `blog` template.

{{AXONYX_RUNTIME_SOURCE_NOTE}}

This starter turns Markdown files into a blog index and prerendered article
routes. It uses Axonyx content collections and Foundry UI without a database.

## Write

Add a file under `content/posts`:

```markdown
---
title: A new field note
description: One sentence used on the index page.
date: 2026-07-15
category: Engineering
reading_time: 4 min read
---
# A new field note

Start writing here.
```

The filename becomes the slug. `a-new-field-note.md` is published at
`/blog/a-new-field-note` during `cargo ax build --clean`.

## Develop

Start the local server:

```bash
cargo ax run dev
```

Inspect content and validate before deploy:

```bash
cargo ax content
cargo ax check
cargo ax doctor
cargo ax test
cargo ax build --clean
```

Generated output includes the home page, about page, content manifest, and one
static route for every Markdown entry.

# Templates

`create-axonyx` now supports starter templates.

## Current Templates

### `minimal`

This is the smallest general-purpose starter.

Good for:

- framework experimentation
- backend authoring tests
- minimal feature spikes

### `site`

This is a more presentation-oriented starter with a stronger landing page voice.

Good for:

- launch pages
- product sites
- framework presentations
- marketing-style Axonyx demos

### `docs`

This is a docs-first starter shaped around reference pages and example sections.

Good for:

- framework docs
- product documentation
- internal guides
- example libraries

## Example Commands

```bash
cargo run -p create-axonyx -- demo-minimal --yes --template minimal
```

```bash
cargo run -p create-axonyx -- demo-site --yes --template site
```

```bash
cargo run -p create-axonyx -- demo-docs --yes --template docs
```

## Planned Templates

The next likely addition is:

- `dashboard`

The goal is for templates to express app intent, not just visual variations.

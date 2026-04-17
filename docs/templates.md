# Templates

`create-axonix` now supports starter templates.

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

## Example Commands

```bash
cargo run -p create-axonix -- demo-minimal --yes --template minimal --runtime-source git
```

```bash
cargo run -p create-axonix -- demo-site --yes --template site --runtime-source git
```

## Planned Templates

The next likely additions are:

- `docs`
- `dashboard`

The goal is for templates to express app intent, not just visual variations.

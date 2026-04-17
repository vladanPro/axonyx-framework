# Runtime Sources

Generated Axonyx apps can currently point at the runtime in three different ways.

## 1. `path`

Best for local framework development.

```bash
cargo run -p create-axonyx -- my-app --yes --runtime-source path
```

This keeps iteration fast while the framework and runtime are evolving together.

## 2. `git`

Best for early adopters or cross-repo development.

```bash
cargo run -p create-axonyx -- my-app --yes --runtime-source git
```

This points the generated app at:

```text
https://github.com/vladanPro/axonyx-runtime
```

## 3. `registry`

Best for the long-term package story.

```bash
cargo run -p create-axonyx -- my-app --yes --runtime-source registry
```

This mode is already scaffold-ready, but it should only be used once the runtime crates are actually published.

## Mental Model

Think of it like this:

- `create-axonyx` creates the app
- `axonyx-runtime` is the runtime package the app depends on
- `axonyx-framework` is where the framework direction is still being shaped

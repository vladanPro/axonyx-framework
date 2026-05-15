# Runtime Sources

Generated Axonyx apps can point at the runtime in three different ways. The default generated flow is `registry`.

## 1. `registry`

Best for current public use.

```bash
cargo run -p create-axonyx -- my-app --yes
```

This points the generated app at:

```toml
axonyx-runtime = "0.1.5"
```

## 2. `path`

Best for local framework development.

```bash
cargo run -p create-axonyx -- my-app --yes --runtime-source path
```

This keeps iteration fast while the framework and runtime are evolving together.
The generated dependency points at the checked out `vendor/axonyx-runtime` submodule by default, with a sibling workspace fallback during migration.

## 3. `git`

Best for testing an unreleased runtime branch.

```bash
cargo run -p create-axonyx -- my-app --yes --runtime-source git
```

This points the generated app at:

```text
https://github.com/vladanPro/axonyx-runtime
```

## Mental Model

Think of it like this:

- `create-axonyx` creates the app
- `axonyx-runtime` is the runtime package the app depends on
- `axonyx-framework` is where the framework direction is still being shaped

## UI Package Resolution

Runtime source and UI package source are separate concerns.

The framework can resolve `.ax` imports such as:

```ax
import { Button } from "@axonyx/ui/foundry/Button.ax"
```

from an `axonyx-ui` Cargo dependency when that package exposes `Axonyx.package.toml`.
Local `component_overrides`, `package_overrides`, and vendored development copies still win first,
so apps can customize or dogfood UI components without changing the public import path.

The default generated UI setup is now:

```toml
axonyx-ui = "0.0.33"
```

That gives the app a normal Cargo dependency while Axonyx resolves `.ax`
components and package CSS through Cargo metadata.

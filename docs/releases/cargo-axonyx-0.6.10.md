# cargo-axonyx 0.6.10

CLI-only patch for compiled page loader arguments.

- Supports `query.field` and `request.query.field`, including `??` defaults.
- Preserves query strings during compiled document rendering and data refresh.
- Decodes query keys/values and uses the last duplicate value, matching the
  page renderer. Missing values are null; empty values do not trigger fallback.

Example:

```text
data listing = loadAdminPosts(query.status ?? "all", query.page ?? "1")
```

Older tooling omitted this binding from compiled route planning, which could
leave its session page unregistered and return 404 instead of the loader's
authorization response. Rebuild compiled apps after upgrading the CLI.

```sh
cargo install cargo-axonyx --version 0.6.10 --locked --force
cargo ax build --clean --compiled
```

No core, runtime, UI or scaffolder version change is required. This patch does
not add dynamic fluent limit/offset, numeric query coercion or arbitrary loader
argument expressions. Registry acceptance and release tags follow publication.

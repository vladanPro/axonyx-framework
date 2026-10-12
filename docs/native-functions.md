# Application native functions V0

This development feature connects trusted application Rust code to backend
`Native.call` bindings. It is not yet part of a new published CLI/runtime release.

Create `src/native.rs` in the application:

```rust
use axonyx_runtime::native::{AxNativeError, AxNativeRegistry};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Content {
    text: String,
}

pub fn register(registry: &mut AxNativeRegistry) -> Result<(), AxNativeError> {
    registry.register("content.prepare", |input: Content| {
        Ok::<_, ()>(Content { text: input.text.trim().to_owned() })
    })
}
```

Add `serde` with the `derive` feature to the application's Cargo dependencies.
The example uses the runtime version from this development branch; existing
registry versions may not contain this API.

Use the registered function in a server `.ax` route or action:

```ax
route POST "/api/content/prepare" {
  data prepared = Native.call("content.prepare", {text: request.form.text})
  return json(prepared)
}
```

The generated production entry includes the application-owned module and calls
`register` once, before sharing the database runtime and starting the listener.
Registration failures abort startup. Rebuild after changing native Rust code:

```sh
cargo ax build --compiled
cargo ax run start --compiled
```

Do not edit `src/generated/backend.rs` or `src/bin/axonyx-production.rs` to
register callbacks. Both are generated files. Applications without
`src/native.rs` keep their existing startup behavior.

## Preview boundary

The CLI preview executable cannot execute application-owned Rust callbacks.
When `src/native.rs` exists, noncompiled `run dev`, `run start` and `run stream`
fail with compiled-server instructions rather than silently ignoring it.
Library preview can invoke a caller-configured registry; an automatic compiled
development/rebuild loop is a separate future feature.

## Security and scope

The registry validates typed inputs and outputs with bounded JSON values and
sanitized errors. It is not zero-copy memory sharing, a WASM bridge, dynamic
symbol loading or a plugin sandbox. Callbacks are trusted server code. Protect
public routes/actions with the appropriate auth and CSRF policy, and retain
their transaction boundary. Registration does not authorize an operation.

Legura rich-text storage and client editor integration are separate follow-up
work. This feature alone does not enable them in the running CMS.

## Verification

`scripts/smoke-native-startup.ps1` builds an isolated application with local
runtime dependencies, checks compiled startup/registration failure and the
preview guard, and exercises typed native calls through HTTP. It never connects
to the operator's CMS database.

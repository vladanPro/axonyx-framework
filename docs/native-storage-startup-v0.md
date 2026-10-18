# Application storage startup V0

Prepared for CLI 0.6.15; publication is a separate release gate. Applications can
provide `src/native_storage.rs` alongside, or without, `src/native.rs`:

```rust
use axonyx_runtime::server::AxFileStorage;
use std::sync::Arc;

pub fn configure(
    fallback: Arc<dyn AxFileStorage>,
) -> Result<Arc<dyn AxFileStorage>, Box<dyn std::error::Error + Send + Sync>> {
    // Return an application adapter, or a wrapper delegating other capabilities.
    Ok(fallback)
}
```

The generated compiled entry builds the existing configured storage registry (or
AxUnavailableFileStorage) first, calls configure exactly once, then shares the
returned Arc across request handlers. Errors stop startup before listening.
No generated file needs manual edits. This API uses the existing runtime trait;
it does not add a runtime crate dependency, a JSON upload envelope or a second
application server. Without the file, generated storage startup stays unchanged.

An adapter may wrap the fallback and intercept only its named capability, or
replace it completely. The hook is trusted application Rust, not a plugin sandbox.
Do not silently delegate a validating media capability to raw storage on failure.
Auth/session/current-user and CSRF checks still belong before Storage.save.
Continue enforcing transport body limits, concurrency and safe public errors.

## Build and preview

Rebuild with cargo ax build --compiled after changing application Rust. Run with
cargo ax run start --compiled. Noncompiled dev/start/stream explicitly reject
this file, even when no src/native.rs exists: the CLI preview process cannot run
application Rust storage configuration. A directory at this file path is an error.
Static generation alone does not execute the hook.

## Capability diagnostics

Without the hook, unknown Storage.save names remain compile diagnostics. With
the hook, names absent from configured storage are delegated to application Rust
at runtime, since the compiler cannot infer that trusted adapter's registry.
The adapter MUST deny unknown names or delegate them to a denying fallback.
Configured read-only capabilities are still rejected by source diagnostics;
configured roots/access/limits are validated and their fallback is preserved.
This is explicit opt-in runtime resolution, not inferred capability discovery.

## Verification

CLI tests cover absent/present/invalid hook files, preview refusal without native
callbacks, startup ordering/single configuration, unchanged no-hook generation,
unknown-name delegation and retained configured read-only diagnostics.

scripts/smoke-native-startup.ps1 builds an isolated compiled application and checks
callback and storage configuration failures before listening, actual custom
multipart Storage.save, rejected fixture guards/cross-site requests, configured
body-limit rejection, unknown capability failure and fallback storage continuity.
Its custom fixture is not Legura image validation; it deliberately transforms a
small fixed payload to prove the application adapter receives actual file bytes.
Fixture guard keys are test-only, not administrator/session authentication.
Legura's separate consumer proof now covers session/CSRF upload, validated image
metadata, persistence and isolated recovery startup (Legura PRs 55 and 56).
Issue #356 tracks the completed hook. Operator media activation and CLI registry
publication remain separate gates, not outcomes implied by that issue closure.

The existing compiled Axum adapter dispatches synchronous handlers through
spawn_blocking; the hook does not move storage/decoder work onto async I/O tasks.
It also does not add an upload-specific semaphore, streaming or HTTP error mapping.
Current oversized multipart conversion returns 400, not a promised 413.

The smoke uses a dedicated native-startup-proof Cargo target, copies its built
server into the disposable app, and cleans only its verified temporary fixture
path. Dependencies stay cached without overwriting the shared application binary.
It does not touch an operator DB, storage directory or running application.
Existing native callback HTTP checks are
retained in that same mandatory CI smoke step.

The generated server now passes the configured max_body_bytes to the Axum
transport instead of a hardcoded 1 MiB. The smoke's 4 KiB setting exercises that
boundary with a 5 KiB multipart payload before custom storage can be invoked.

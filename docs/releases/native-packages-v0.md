# Native Package Bridge V0

Release candidates: axonyx-core/runtime 0.6.7, cargo-axonyx 0.6.11,
create-axonyx 0.6.8 and independent axonyx-ui 0.0.86.

Explicit Native.call bridges resolve typed, allowlisted Rust registry callbacks.
Compiled application startup registers src/native.rs before serving requests.
Missing callbacks, input/result validation failures and preview's unavailable
native environment fail closed. Native JSON result members and aliases lower
consistently in compiled handlers. This is not arbitrary Rust execution or a
zero-copy/browser memory bridge.

New projects and cargo ax upgrade pin the matching runtime and UI versions.
UI adds eleven native Foundry editor icons; editor/CMS packages are not released.
Proofs include native backend consumer parity, real compiled HTTP startup,
Legura authenticated content/save/reload and safe legacy migration/recovery.
Known CMS follow-ups remain public rich ASX rendering, native full-page recovery
and operator activation. Unchanged LSP/WASM/macros are not republished.

Release gate: required CI on dev/release and main, then core -> runtime -> UI ->
scaffolder -> CLI, waiting for registry propagation and verified package builds.
Test installation with --locked, then compile/run a disposable registry-only
consumer. Tags/releases follow confirmed publication. npm UI publishing may
require the owner's interactive 2FA; do not treat a Cargo publish as npm success.

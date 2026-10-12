# Shared Domain Validation V0

Release candidates: core/runtime 0.6.8, create-axonyx 0.6.9,
cargo-axonyx 0.6.12. UI 0.0.86, LSP, macros and WASM are unchanged.

Generated Domain helpers use an internal Result ABI, safe reusable String
arguments and checked typed record returns. Author signatures and normal calls
are unchanged. Nested calls and calls in validations/JSON expressions propagate
errors; wrong arity is diagnosed before compiling Rust.
Helper parameters map canonical String[]/Json types to Rust types. Record
results support member access in handler data. Record conversion is checked
serde conversion, not zero-copy or full static expression type checking.

Release gates: green runtime/framework preparation and main CI, core publication
then runtime publication, scaffolder then CLI, waiting for index propagation.
Do not publish UI/npm or Docker as part of this release.

Registry acceptance must install the published CLI with --locked into an isolated
root and run the Legura Posts HTTP gate using registry packages, without source
overrides. Do not change the operator checkout/DB or global CLI as proof setup.

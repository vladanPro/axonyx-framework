# Reactive Foundry table (source preview)

This example combines `List.filterEq`, `List.sortBy`, and a keyed `Each`
that renders native Foundry table rows. It starts on Archived, with no
matches. Draft displays two rows; Title descending reorders those same
rows. Both changes run locally on the loaded state.

Use the framework and vendored runtime from this checkout, not the
published CLI/runtime versions that predate these operations:

1. From a scratch directory, run
   `cargo run --manifest-path <framework>/Cargo.toml -p create-axonyx -- table-demo --yes --template site --runtime-source path`.
2. Copy [page.asx](page.asx) into `table-demo/app/reactive-table/page.asx`.
   The scaffold's layout loads the Foundry package.
3. From `table-demo`, run
   `cargo run --manifest-path <framework>/Cargo.toml -p cargo-axonyx --bin cargo-axonyx -- check`.
   Run the same command with `build --clean` and then `run start` instead of `check`.
4. Open `http://127.0.0.1:3000/reactive-table`.

The source list is page state, not a database query. Use the server to
filter or sort records that have not been loaded.

The framework's `scripts/smoke-core-loop.ps1 -Template site` copies this
route into a fresh scaffold and checks the native table in its build output.

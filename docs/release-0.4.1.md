# Axonyx 0.4.1

Axonyx 0.4.1 is a coordinated patch release of the runtime, language tooling,
scaffolder, and CLI.

## Highlights

- persistent SQLite and Postgres sessions
- server-only signed cookie response channels
- request-owned `Auth.subject`
- typed authenticated-user query composition
- `User?` result narrowing after direct guards
- safe `forbidden()` policy responses
- borrow-aware pure policy helpers without hidden record clones
- earlier `cargo ax check` diagnostics for unsafe optional access

## Packages

- `axonyx-core 0.4.1`
- `axonyx-runtime 0.4.1`
- `axonyx-lsp 0.4.1`
- `create-axonyx 0.4.1`
- `cargo-axonyx 0.4.1`

## Upgrade

```bash
cargo install cargo-axonyx --version 0.4.1 --force
cargo ax upgrade
cargo update
cargo ax check
cargo ax build --clean
```

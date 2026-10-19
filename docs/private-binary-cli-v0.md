# Private Binary CLI Integration V0

The private Binary contract is published in core/runtime/tooling 0.7.0. Its
public Rust enum changes required that minor boundary, not a compatible 0.6.x
patch. This source smoke uses the pinned runtime submodule; running it does not
upgrade an installed application's CLI/runtime dependencies.

```ax
route GET "/api/private/:id" -> Binary {
  return Native.privateResponse("media.read", { id: params.id })
}
```

`cargo ax check` shares the compiler-owned private-return validator. Binary is
not an unresolved record schema. Session-secret discovery and generated runtime
selection recognize implicit session requirements even without Auth.subject in
the source. Native callback registration remains application Rust code.

API/Melt reports retain `returns: Binary`, session authentication and the closed
403/404/500 failure contract. Request/response hashes retain the transport marker
and authentication, distinguishing this contract from JSON responses. OpenAPI
uses binary strings under PNG/JPEG/octet-stream, not an application/json record
reference, and documents private/no-store/nosniff success headers.

`scripts/smoke-private-binary.ps1` creates a disposable fixture, runs the source
CLI's check/OpenAPI/build pipeline and starts its generated production server on
loopback with API validation always. It exercises exact non-UTF8 bytes, private
headers, anonymous/logout denial, not-found and redacted failures. The fixture's
key-guarded session endpoint is test scaffolding only, NOT a product login flow.
It never uses the operator CMS, database or production environment file. Fixture
process cleanup affects only the process it started and its own generated path.

Legura registry adoption (PR 65) passed its seven CI gates and merged into dev;
operator activation remains separate. No private media is copied into public/dist.

## Compound Request Guard Regression

The runtime source fix from PR 269 resolves framework issue 368's inline guard
failure. The fixture no longer works around it with an intermediate binding:

```ax
require query.key == "isolated-fixture-only" else forbidden()
```

The real compiled Axum HTTP proof checks rejection before session creation,
accepted/rejected query/parameter/header and membership guards, a short-circuit
that does not read a missing RHS, compound Auth.subject session discovery,
literal preservation and post-logout denial. API validation stays always on.
These new regression checks require the adopted source runtime until a later
patch is actually published; they do not imply that immutable 0.7.0 was changed.

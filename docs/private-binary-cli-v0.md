# Private Binary CLI Integration V0 (Unpublished)

This adopts the source runtime's private Binary route contract. It does not
publish packages or change an installed application's CLI/runtime dependencies.
The enum changes require a coordinated next 0.7 release, not a compatible 0.6.x
patch. Until that release, use the pinned source submodule for these proofs.

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

Legura endpoint adoption, isolated CMS acceptance and coordinated registry
publication remain separate gates. No private media is copied into public/dist.

# Public Binary CLI Integration V0 (Unpublished)

This source candidate adopts runtime dev commit 1133bdf and its explicit public
callback/return-plan channel. No Cargo version is published or global CLI updated.

```ax
route GET "/api/posts/:slug/image/:id" -> Binary {
  return Native.publicResponse("post.image", { slug: params.slug, id: params.id })
}
```

The trusted Rust callback registered with register_public_response still checks
current publication/association before reading bytes. An arbitrary media ID does
not grant access, and public invocation cannot dispatch to private callbacks.
This is not a generic public static-directory endpoint or a callback sandbox.

## CLI Contract

- Check uses the shared compiler validator; invalid public returns get a public
  response-contract diagnostic at the authored call.
- Input/environment collection includes public binary payloads, including nested
  environment expressions without interpreting string examples as code.
- Session/database discovery does not infer requirements from a public binary
  response alone. Explicit Auth.subject references still request session setup.
- API/Melt contracts and OpenAPI retain the Binary marker, allowed binary MIME,
  failure statuses and conservative no-store/nosniff success headers. An anonymous
  public route has no signed-session security requirement. Explicit public auth
  guards/payloads are reflected separately; quoted Auth.subject text is not auth.
- Request hashes reflect authentication; identical output contracts retain their
  response hashes. Reports are not a substitute for runtime product authorization.

Native registration keeps the established compiled-only mode. Direct preview
does not execute arbitrary application Rust: it rejects registration explicitly
and points to build --compiled / run start --compiled. This is not transparent
native execution in the interpretive preview server.

## Proof

The existing scripts/smoke-private-binary.ps1 now exercises both channels through
the actual source CLI check/OpenAPI/build pipeline and generated Axum server.
Anonymous public requests preserve exact non-UTF8 bytes and MIME; conditional
headers do not produce a 304 or retain access after fixture publication is
revoked. Wrong names, private-channel targets and callback failures stay closed.
Existing private anonymous/logout denial, session/CSRF and compound guard proof
remain in the same isolated fixture. Response validation is always enabled.
The fixture's publication flag and key-guarded login are test scaffolding, not a
production CMS authentication or storage implementation.

The fixture process, database and temporary paths are isolated from the operator
CMS. The script's existing path checks constrain cleanup, and its finally block
restores only the environment variables it changed.

## Release Gate

The new public return-plan enum variant breaks exhaustive downstream matches.
Coordinate the next minor line, not a compatible 0.7.x patch. CLI/core/runtime
pins, fresh registry-only consumers and required CI must be validated before
publication. The source submodule pointer is not proof of published capability.
Legura public image delivery and browser acceptance remain a subsequent slice.

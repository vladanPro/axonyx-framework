# Scoped Action Body Limits V0

This development slice adopts runtime PR #261 for explicitly configured compiled
hosts. It is not a crates.io release or Legura product activation. Registry apps
will need the subsequent runtime/tooling release before using the new host API.

## Configuration

```toml
[server]
max_body_bytes = "64kb"

[[server.action_body_limits]]
path = "/admin/media"
action = "UploadImage"
max_body_bytes = "9mb"
```

`path` is the compiler's page route pattern, such as `/posts/:id`, not a concrete
parameter value or the physical `/__axonyx/action` endpoint. The action must be
available through that route's action sources. The existing backend source
collector includes shared backend/helper actions and excludes other routes'
`actions.ax` files. A page without `actions.ax` cannot receive a scoped budget.

`cargo ax check` and `cargo ax doctor` reject unknown route/action pairs, duplicate
pairs, unknown entry fields, missing values and non-positive/invalid byte limits.
Integers and existing byte-unit strings share the global limit parser. Every
entry requires `path`, `action` and `max_body_bytes`.

Use `cargo ax build --compiled`, then `cargo ax run start --compiled`. Preview,
std and noncompiled Tokio hosts explicitly refuse a nonempty scoped policy for
now, rather than silently applying a different budget. Without scoped entries,
existing host APIs and their legacy error behavior remain unchanged.

## Selection Before Body Reading

Only POST requests with a strict, unambiguous action selector can select a scoped
limit. The compiler embeds every page route, including actionless static pages,
and available action names. Page resolution happens before action membership:
`/posts/new` cannot borrow `/posts/:id`'s upload allowance. Exact branches take
priority over dynamic branches; dynamic siblings follow filesystem name order.
Unknown or ambiguous targets keep the global limit. Selectors never supply a
byte limit themselves. The generated matcher is also tested against filesystem
route resolution, not just a separate hand-written expectation.

This table controls budgets only. Existing action dispatch, cross-route form
rendering, authorization, origin checks and CSRF protection are not replaced by
it. A larger transport allowance does not authorize uploads or validate files.

The opt-in runtime collects the entire bounded body, including multipart
epilogues, before dispatch. Oversize bodies receive a generic no-store 413;
malformed bodies receive a safe 400. This is bounded buffering, not storage
streaming or a total process-memory/concurrency guarantee. Image/file limits,
decode concurrency and storage access remain separate boundaries.

## Acceptance

Unit tests cover strict selectors, route/action ownership, actionless shadowing,
dynamic branch precedence, generated policy embedding, config diagnostics and
the explicit preview refusal. The native HTTP smoke has a `-Scoped` mode proving
a 64 KiB default and one 9 MiB action budget with known-length and chunked bodies,
ambiguous/unknown selectors, independent storage validation and guarded writes.
The original smoke remains as a no-policy compatibility test.

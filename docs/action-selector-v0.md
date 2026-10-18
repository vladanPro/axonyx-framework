# Strict Action Selector V0

This is a prerequisite for scoped upload limits in issue #360, not a new
Axonyx.toml option, published CLI release or completed body-budget feature.

## One Parser For Preview And Compiled Actions

`crates/cargo-axonyx/src/action_selector.rs` is used by the CLI action handler and
embedded verbatim as a Rust module in generated compiled hosts. There is no
separately maintained parser with different decoding/duplicate behavior.

Valid action URLs have this form (parameter order is not significant):

```text
/__axonyx/action?path=%2Fadmin%2Fmedia&name=UploadImage
```

Both literal `path` and `name` keys are required exactly once. Duplicate keys and
percent-encoded aliases such as `%6Eame` are refused, including when their values
are identical. Generated forms already use the literal keys. Handwritten URLs
that relied on an omitted path falling back to `/` must now provide `path=%2F`.

Values use strict, single-pass form-query decoding: valid UTF-8 and `%XX` only,
with `+` representing a space. No lossy UTF-8 replacement or repeated decoding
is performed. Logical route query values are preserved for normal dispatch.
Paths must be local absolute routes without dot segments, backslashes in the
route, fragments, controls or surrounding route whitespace. Empty/control/space
action names are refused. Other query parameters do not select an action, but
their encodings must still be valid.

Invalid selectors return a generic no-store `400 invalid action selector` before
route lookup/action execution. Existing origin, content-type and authorization
boundaries are not removed. Valid source-generated actions retain their behavior.

## What Is Not Claimed

Syntactic validity is not proof that the named action exists or belongs to the
route. The next budget layer must check the compiler-owned route/action table,
using normal route resolution for dynamic/static precedence, before granting a
larger cap. There is no new body-limit setting or generated Axum policy wiring
in this slice. The runtime foundation merged separately as runtime PR #261.

## Verification

Parser tests cover reordered fields, nested route queries, Unicode, single-pass
decoding, duplicates/encoded aliases, malformed percent/UTF-8 input, missing keys,
unsafe paths and wrong endpoints. Preview/start handler tests refuse invalid
selectors even before route lookup. The compiled-source test checks that the
same implementation is embedded, and the native startup HTTP smoke refuses
ambiguous selectors without custom-storage writes, then succeeds with a valid
percent-encoded action name.

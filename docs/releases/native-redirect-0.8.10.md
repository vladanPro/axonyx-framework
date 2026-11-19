# Native Redirect Patch

## Versions

- `axonyx-core` 0.8.4: compiled redirect string extraction and the previously
  merged backend AX line-comment support.
- `cargo-axonyx` 0.8.10: adopt core 0.8.4 when generating compiled applications.
- `axonyx-runtime` stays 0.8.4. LSP, scaffold and UI do not need a new release
  for this patch. They keep their existing package versions.

## Fixed Behavior

An action can obtain a URL from a registered native function and return it:

```ax
action Complete() {
  data destination = Native.call("app.destination", null)
  return redirect(destination)
}
```

The function must return a String URL. Compiled code extracts that string
instead of calling JSON serialization and adding quote characters to the
redirect. Native aliases and object members work as well. Direct API redirect
returns use the same extraction, including an explicit redirect status.
Non-string targets are rejected with a fixed error, not converted to JSON URLs.
This does not grant arbitrary native functions or change redirect URL policy.

Existing compiled/preview API consumer tests and compiled action tests cover
the exact target. Preview action `return redirect(...)` remains a separate
existing limitation tracked by runtime issue #298.

## Release Gate

1. Promote the runtime release source from `dev` to `main` through a PR and
   require green main CI before publishing core 0.8.4.
2. Verify the core tarball/registry propagation, then promote framework `dev`
   through its own `main` PR and require green main CI.
3. Rehearse CLI packaging and publish 0.8.10; verify a locked registry install.
4. Pin Legura's test CLI to 0.8.10 and rerun category assignment HTTP and Aegis
   browser gates without a source toolchain override before merging its draft.

Source CLI tests are evidence for the fix, not registry acceptance. Do not
republish runtime/UI or migrate/deploy an operator database as part of this patch.

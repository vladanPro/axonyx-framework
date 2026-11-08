# Translation catalog validation V0

Opt in from `Axonyx.toml`:

```toml
[i18n]
fallback = "en"
```

Create one flat JSON object of string messages per locale:

```text
locales/en/messages.json
locales/sr-Latn/messages.json
```

For example, the English catalog can contain `{"pages.save":"Save"}`. The
fallback defines the known keys. Other locales may omit messages, but may not
introduce unknown keys or duplicate keys. Locale names are matched without
ASCII case distinctions; catalog directories and files must not be symlinks.

`cargo ax check` and build preflight report invalid catalogs as
`axonyx-i18n` errors. `cargo ax melt --format json` and the emitted Melt graph
list the fallback and catalog source paths without publishing message values.
Projects without `[i18n]` do not load these catalogs.

This step validates build inputs only. It does not yet add `t()` to ASX,
select a locale from a request, or translate pages, validation errors, or
user-authored content.

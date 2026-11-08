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

With `[i18n]` enabled, the ASX checker reserves the callable name `t` and
accepts exactly one literal key from the fallback catalog, such as
`t("pages.save")`. A local `fn t(...)` is rejected, but variables and member
names such as `item.t` are not reserved. Without `[i18n]`, existing calls and
functions are unaffected. Calls inside `Head` are not supported.

This step validates build inputs and ASX usage only. The CLI does not yet
attach a translator to page rendering or select a locale from a request.

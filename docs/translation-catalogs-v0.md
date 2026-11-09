# Translation catalog validation V0

Opt in from `Axonyx.toml`:

```toml
[i18n]
locale = "sr-Latn"
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
requires a literal key from the fallback catalog, such as `t("pages.save")`.
A local `fn t(...)` is rejected, but variables and member
names such as `item.t` are not reserved. Without `[i18n]`, existing calls and
functions are unaffected. Calls inside `Head` are not supported.

`locale` selects the catalog used to render pages. If omitted, it defaults to
the fallback. A configured locale must have a catalog. Preview and static
output read the configured locale; compiled servers embed validated catalogs
and use the same locale for SSR and refreshed fragments. Compiled i18n requires
a runtime that supports the localized render APIs (not the published 0.8.3
runtime). Projects without `[i18n]` keep the existing compiled render APIs.

Pass a locale string as the optional second argument to override the language
for one message. For example, use a preference returned by an admin loader:

```asx
<section lang={preferences.admin_language}>
  <button>{t("pages.save", preferences.admin_language)}</button>
</section>
```

An unsupported override uses the fallback catalog; missing messages in a
supported locale also fall back per key. An override never changes other
calls or the document's `<html lang>`. Set `lang` on a region when its language
differs. Automatic request-wide or per-user locale negotiation is not wired
into the CLI.

### Bind a language for one screen

Bind the locale once in a page or layout body and call the local translator
with literal keys:

```asx
const translate = I18n.withLocale(preferences.admin_language)

return ASX {
  <section lang={preferences.admin_language}>
    <button>{translate("pages.save")}</button>
  </section>
}
```

Declare the binding after its locale data is available. The page and layout
may each declare their own `translate`; neither affects the other or the
document language. Melt checks the keys just as it does for `t()`. This local
translator is not a general function value: it cannot be exported, passed as
a prop, or used from `Head`, another source file, or a local function body.
For a one-off override, use `t(key, locale)`.

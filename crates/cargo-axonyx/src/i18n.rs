use super::{display_path, line_for_config_table, CheckDiagnostic};
use axonyx_runtime::translation::{AxCatalogError, AxTranslationCatalog};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

mod usage;
pub(super) use usage::check_document;

pub(super) fn is_render_binding(
    document: &axonyx_core::ax_ast::AxDocument,
    value: &axonyx_core::ax_ast::AxExpr,
) -> bool {
    use axonyx_core::ax_ast::{AxExpr, AxStatement};
    use axonyx_runtime::translation::bound_translator_initializer;
    let AxExpr::Call { path, .. } = value else {
        return false;
    };
    path == &["t"]
        || bound_translator_initializer(value)
        || matches!(path.as_slice(), [name] if document.page.body.iter().any(|statement| {
            matches!(statement, AxStatement::Data(binding) if &binding.name == name && bound_translator_initializer(&binding.value))
        }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    fallback: String,
    locale: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct CatalogReport {
    pub(super) fallback: String,
    pub(super) locale: String,
    pub(super) sources: Vec<CatalogSource>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct CatalogSource {
    pub(super) locale: String,
    pub(super) file: String,
}

#[derive(Debug)]
pub(super) struct CatalogInputs {
    pub(super) report: CatalogReport,
    pub(super) catalog: AxTranslationCatalog,
    sources: Vec<(String, String)>,
}

impl CatalogInputs {
    pub(super) fn translator(&self) -> axonyx_runtime::translation::AxTranslator<'_> {
        self.catalog.translator(&self.report.locale)
    }

    pub(super) fn compiled_helpers(&self) -> String {
        let entries = self
            .sources
            .iter()
            .map(|(locale, json)| format!("({locale:?}, {json:?}),\n"))
            .collect::<String>();
        let fallback = &self.report.fallback;
        let locale = &self.report.locale;
        format!(
            r#"
fn compiled_translator() -> axonyx_runtime::translation::AxTranslator<'static> {{
    use axonyx_runtime::translation::AxTranslationCatalog;
    static CATALOG: std::sync::OnceLock<AxTranslationCatalog> = std::sync::OnceLock::new();
    CATALOG.get_or_init(|| AxTranslationCatalog::from_json({fallback:?}, [
        {entries}
    ]).expect("Melt validated the embedded translation catalogs")).translator({locale:?})
}}

fn render_localized_document(
    document_json: &str, import_sources: &[(&str, &str)], request_target: &str,
    route_params: &BTreeMap<String, String>, loader_values: &BTreeMap<String, Value>,
    form_result: Option<&axonyx_runtime::form_result::AxFormResult>,
) -> Result<String, axonyx_runtime::PreviewError> {{
    axonyx_runtime::AxCompiledPageRender {{
        document_json, import_sources, request_target, route_params, loader_values, form_result,
    }}.render_document(Some(compiled_translator()))
}}

fn render_localized_fragment(
    document_json: &str, import_sources: &[(&str, &str)], request_target: &str,
    route_params: &BTreeMap<String, String>, loader_values: &BTreeMap<String, Value>,
) -> Result<String, axonyx_runtime::PreviewError> {{
    axonyx_runtime::AxCompiledPageRender {{
        document_json, import_sources, request_target, route_params, loader_values, form_result: None,
    }}.render_fragment(Some(compiled_translator()))
}}
"#
        )
    }
}

pub(super) fn inspect(root: &Path) -> Result<Option<CatalogInputs>, CheckDiagnostic> {
    let config_path = root.join("Axonyx.toml");
    let source = match fs::read_to_string(&config_path) {
        Ok(source) => source,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(diagnostic(&config_path, 1, 1, error.to_string())),
    };
    // The general config checker owns TOML syntax diagnostics.
    let Ok(config) = source.parse::<toml::Value>() else {
        return Ok(None);
    };
    let Some(value) = config.get("i18n") else {
        return Ok(None);
    };
    let config_line = line_for_config_table(&source, "i18n");
    let config: Config = value.clone().try_into().map_err(|error: toml::de::Error| {
        diagnostic(
            &config_path,
            config_line,
            1,
            format!("invalid [i18n] configuration: {error}"),
        )
    })?;

    let directory = root.join("locales");
    let metadata = fs::symlink_metadata(&directory).map_err(|error| {
        diagnostic(
            &directory,
            1,
            1,
            format!("could not open the locales directory: {error}"),
        )
    })?;
    if !metadata.file_type().is_dir() {
        return Err(diagnostic(
            &directory,
            1,
            1,
            "locales must be a real directory",
        ));
    }
    let mut entries = fs::read_dir(&directory)
        .and_then(|entries| entries.collect::<Result<Vec<_>, _>>())
        .map_err(|error| diagnostic(&directory, 1, 1, error.to_string()))?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut catalogs = Vec::new();
    for entry in entries {
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| diagnostic(&path, 1, 1, error.to_string()))?;
        if kind.is_symlink() {
            return Err(diagnostic(
                &path,
                1,
                1,
                "locale directories cannot be symlinks",
            ));
        }
        if !kind.is_dir() {
            continue;
        }
        let locale = entry
            .file_name()
            .into_string()
            .map_err(|_| diagnostic(&path, 1, 1, "locale directory names must be UTF-8"))?;
        let file = path.join("messages.json");
        let json = read_catalog(&file)?;
        catalogs.push((locale, file, json));
    }
    let catalog = AxTranslationCatalog::from_json(
        &config.fallback,
        catalogs
            .iter()
            .map(|(locale, _, json)| (locale.as_str(), json.as_str())),
    )
    .map_err(|error| catalog_diagnostic(error, &catalogs, &config_path, config_line))?;

    let fallback = catalog.translator(&config.fallback).locale();
    let requested = config.locale.as_deref().unwrap_or(fallback);
    let locale = catalog.translator(requested).locale();
    if !requested.eq_ignore_ascii_case(locale) {
        return Err(diagnostic(
            &config_path,
            config_line,
            1,
            format!("[i18n] locale {requested:?} has no catalog"),
        ));
    }
    let report = CatalogReport {
        fallback: fallback.into(),
        locale: locale.into(),
        sources: catalogs
            .iter()
            .map(|(locale, _, _)| CatalogSource {
                file: format!("locales/{locale}/messages.json"),
                locale: locale.clone(),
            })
            .collect(),
    };
    let sources = catalogs
        .into_iter()
        .map(|(locale, _, json)| (locale, json))
        .collect();
    Ok(Some(CatalogInputs {
        report,
        catalog,
        sources,
    }))
}

pub(super) fn load_for_render(root: &Path) -> anyhow::Result<Option<CatalogInputs>> {
    inspect(root).map_err(|error| anyhow::anyhow!(super::format_check_diagnostic(&error)))
}

fn read_catalog(path: &Path) -> Result<String, CheckDiagnostic> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        diagnostic(
            path,
            1,
            1,
            format!("could not open message catalog: {error}"),
        )
    })?;
    if !metadata.file_type().is_file() {
        return Err(diagnostic(
            path,
            1,
            1,
            "message catalog must be a regular file",
        ));
    }
    fs::read_to_string(path).map_err(|error| {
        diagnostic(
            path,
            1,
            1,
            format!("could not read message catalog: {error}"),
        )
    })
}

fn catalog_diagnostic(
    error: AxCatalogError,
    catalogs: &[(String, PathBuf, String)],
    config_path: &Path,
    config_line: usize,
) -> CheckDiagnostic {
    let locale = match &error {
        AxCatalogError::InvalidLocale(locale) | AxCatalogError::DuplicateLocale(locale) => {
            Some(locale)
        }
        AxCatalogError::InvalidJson { locale, .. }
        | AxCatalogError::InvalidKey { locale, .. }
        | AxCatalogError::UnknownKey { locale, .. } => Some(locale),
        AxCatalogError::MissingFallback(_) => None,
    };
    let Some((_, file, _)) =
        locale.and_then(|locale| catalogs.iter().find(|item| &item.0 == locale))
    else {
        return diagnostic(config_path, config_line, 1, error.to_string());
    };
    let (line, column) = match &error {
        AxCatalogError::InvalidJson { source, .. } => {
            (source.line().max(1), source.column().max(1))
        }
        _ => (1, 1),
    };
    diagnostic(file, line, column, error.to_string())
}

fn diagnostic(
    path: &Path,
    line: usize,
    column: usize,
    message: impl Into<String>,
) -> CheckDiagnostic {
    CheckDiagnostic {
        file: display_path(path),
        line,
        column,
        severity: "error",
        code: "axonyx-i18n",
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        check_app_sources, collect_melt_report, ensure_no_melt_diagnostics_for,
        tests::make_temp_dir,
    };

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(name: &str) -> Self {
            Self(make_temp_dir(name))
        }

        fn write(&self, path: &str, contents: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn melt_lists_sources_without_serializing_messages() {
        let app = Fixture::new("i18n-melt");
        app.write("Axonyx.toml", "[i18n]\nfallback = \"EN\"\n");
        app.write(
            "locales/en/messages.json",
            r#"{"pages.save":"Catalog text only"}"#,
        );
        app.write("locales/sr-Latn/messages.json", "{}");

        assert!(check_app_sources(&app.0).unwrap().is_empty());
        let report = serde_json::to_value(collect_melt_report(&app.0).unwrap()).unwrap();
        assert_eq!(report["i18n"]["fallback"], "en");
        assert_eq!(report["i18n"]["locale"], "en");
        assert_eq!(report["i18n"]["sources"][1]["locale"], "sr-Latn");
        assert_eq!(
            report["i18n"]["sources"][1]["file"],
            "locales/sr-Latn/messages.json"
        );
        assert!(!report.to_string().contains("Catalog text only"));
    }

    #[test]
    fn static_build_renders_the_configured_language_and_protects_catalog_sources() {
        let app = Fixture::new("i18n-static");
        app.write(
            "Axonyx.toml",
            "[i18n]\nlocale = \"sr-Latn\"\nfallback = \"en\"\n",
        );
        app.write(
            "locales/en/messages.json",
            r#"{"pages.save":"Save","pages.back":"Go back"}"#,
        );
        app.write("locales/sr-Latn/messages.json", r#"{"pages.save":"Snimi"}"#);
        app.write(
            "app/page.asx",
            r#"
page Home() {
  const translate = I18n.withLocale("sr-Latn")
  const save = translate("pages.save")
  return ASX { <p>{save} {translate("pages.back")}</p> }
}
"#,
        );
        assert!(check_app_sources(&app.0).unwrap().is_empty());
        crate::build_static_site_from_app_root(&app.0, Path::new("dist"), false).unwrap();
        let html = fs::read_to_string(app.0.join("dist/index.html")).unwrap();
        assert!(html.contains(r#"<html lang="sr-Latn""#));
        assert!(html.contains("Snimi"));
        assert!(html.contains("Go back"));
        assert!(!app.0.join("dist/locales").exists());
        let error =
            crate::build_static_site_from_app_root(&app.0, Path::new("locales"), true).unwrap_err();
        assert!(error.to_string().contains("overlaps project source"));

        app.write(
            "Axonyx.toml",
            "[i18n]\nlocale = \"de\"\nfallback = \"en\"\n",
        );
        let diagnostic = inspect(&app.0).unwrap_err();
        assert!(diagnostic.file.ends_with("Axonyx.toml"));
        assert!(diagnostic.message.contains("has no catalog"));
    }

    #[test]
    fn checks_bound_translator_keys_and_scope_before_build() {
        let app = Fixture::new("i18n-bound-translator");
        app.write("Axonyx.toml", "[i18n]\nfallback = \"en\"\n");
        app.write("locales/en/messages.json", r#"{"pages.save":"Save"}"#);
        for (body, expected) in [
            (
                r#"const translate = I18n.withLocale("en")
return ASX { <p>{translate("pages.svae")}</p> }"#,
                "unknown translation key",
            ),
            (
                r#"const translate = I18n.withLocale("en")
return ASX { <p>{translate("pages.save", "en")}</p> }"#,
                "expects one literal string key",
            ),
            (
                r#"const message = translate("pages.save")
const translate = I18n.withLocale("en")
return ASX { <p>{message}</p> }"#,
                "after its declaration",
            ),
            (
                r#"const translate = I18n.withLocale("en")
return ASX { <p>{translate}</p> }"#,
                "cannot be used as a value",
            ),
        ] {
            app.write("app/page.asx", &format!("page Home() {{\n{body}\n}}"));
            let diagnostics = check_app_sources(&app.0).unwrap();
            assert!(
                diagnostics
                    .iter()
                    .any(|item| item.code == "axonyx-i18n" && item.message.contains(expected)),
                "{diagnostics:?}"
            );
        }
    }

    #[test]
    fn catalogs_are_opt_in_and_invalid_catalogs_block_build_preflight() {
        let app = Fixture::new("i18n-check");
        app.write("locales/en/messages.json", "{\n  \"pages.save\": 42\n}");
        assert!(check_app_sources(&app.0).unwrap().is_empty());
        assert!(collect_melt_report(&app.0).unwrap().i18n.is_none());

        app.write("Axonyx.toml", "[i18n]\nfallback = \"en\"\n");
        let diagnostics = check_app_sources(&app.0).unwrap();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "axonyx-i18n");
        assert!(diagnostics[0].file.ends_with("locales/en/messages.json"));
        assert_eq!(diagnostics[0].line, 2);
        let error = ensure_no_melt_diagnostics_for(&app.0, "build").unwrap_err();
        assert!(error.to_string().contains("axonyx-i18n"));

        app.write("locales/en/messages.json", r#"{"pages.save":"Save"}"#);
        app.write("locales/sr-Latn/messages.json", r#"{"pages.svae":"Snimi"}"#);
        let diagnostic = inspect(&app.0).unwrap_err();
        assert!(diagnostic.file.ends_with("locales/sr-Latn/messages.json"));
        assert!(diagnostic.message.contains("pages.svae"));
    }

    #[test]
    fn missing_fallback_and_catalog_have_actionable_locations() {
        let app = Fixture::new("i18n-missing");
        app.write("Axonyx.toml", "[i18n]\nfallback = \"en\"\n");
        app.write("locales/sr-Latn/messages.json", "{}");
        let missing = inspect(&app.0).unwrap_err();
        assert!(missing.file.ends_with("Axonyx.toml"));
        assert!(missing.message.contains("fallback catalog is missing"));
        fs::create_dir(app.0.join("locales/en")).unwrap();
        let missing = inspect(&app.0).unwrap_err();
        assert!(missing.file.ends_with("locales/en/messages.json"));
        assert!(missing.message.contains("could not open message catalog"));
    }

    #[test]
    fn reserves_only_the_callable_name_in_opted_in_asx_files() {
        let app = Fixture::new("i18n-call-name");
        let source = r#"
page Home() {
  fn t(value: String) = value
  return ASX { <p>{t("pages.save")}</p> }
}
"#;
        app.write("app/page.asx", source);
        assert!(check_app_sources(&app.0).unwrap().is_empty());
        app.write("Axonyx.toml", "[i18n]\nfallback = \"en\"\n");
        app.write("locales/en/messages.json", r#"{"pages.save":"Save"}"#);
        let diagnostics = check_app_sources(&app.0).unwrap();
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("reserved"));

        app.write(
            "app/page.asx",
            r#"
page Home() {
  const t = "ordinary variable"
  const item = { t: "ordinary member" }
  return ASX { <p title={t("pages.save")}>{t} {item.t} t("not-a-call")</p> }
}
"#,
        );
        assert!(check_app_sources(&app.0).unwrap().is_empty());
    }

    #[test]
    fn checks_calls_in_components_and_requires_literal_known_keys() {
        let app = Fixture::new("i18n-call-key");
        app.write("Axonyx.toml", "[i18n]\nfallback = \"en\"\n");
        app.write("locales/en/messages.json", r#"{"pages.save":"Save"}"#);
        app.write(
            "app/page.asx",
            r#"
page Home() {
  const key = "pages.save"
  const locale = "en"
  return ASX { <p>{t(key, locale)} {t("pages.save", locale)} {t("pages.save", locale, locale)} {t("pages.save", 42)}</p> }
}
"#,
        );
        app.write(
            "app/components/SaveButton.asx",
            r#"
component SaveButton() {
  return ASX { <button title={t("pages.svae", "en")}>{t("pages.save")}</button> }
}
"#,
        );
        let diagnostics = check_app_sources(&app.0).unwrap();
        assert_eq!(diagnostics.len(), 4, "{diagnostics:?}");
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("expects a literal string key"))
                .count(),
            2
        );
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("must be a string")));
        let unknown = diagnostics
            .iter()
            .find(|d| d.message.contains("pages.svae"))
            .unwrap();
        assert!(unknown.file.ends_with("app/components/SaveButton.asx"));
        assert_eq!(unknown.code, "axonyx-i18n");
        assert!(ensure_no_melt_diagnostics_for(&app.0, "build").is_err());
    }
}

use super::{display_path, line_for_config_table, CheckDiagnostic};
use axonyx_runtime::translation::{AxCatalogError, AxTranslationCatalog};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

mod usage;
pub(super) use usage::check_document;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    fallback: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct CatalogReport {
    pub(super) fallback: String,
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

    let report = CatalogReport {
        fallback: catalog.translator(&config.fallback).locale().into(),
        sources: catalogs
            .into_iter()
            .map(|(locale, _, _)| CatalogSource {
                file: format!("locales/{locale}/messages.json"),
                locale,
            })
            .collect(),
    };
    Ok(Some(CatalogInputs { report, catalog }))
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
        assert_eq!(report["i18n"]["sources"][1]["locale"], "sr-Latn");
        assert_eq!(
            report["i18n"]["sources"][1]["file"],
            "locales/sr-Latn/messages.json"
        );
        assert!(!report.to_string().contains("Catalog text only"));
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
  return ASX { <p>{t(key)} {t("pages.save", "extra")}</p> }
}
"#,
        );
        app.write(
            "app/components/SaveButton.asx",
            r#"
component SaveButton() {
  return ASX { <button title={t("pages.svae")}>{t("pages.save")}</button> }
}
"#,
        );
        let diagnostics = check_app_sources(&app.0).unwrap();
        assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.contains("one literal string key"))
                .count(),
            2
        );
        let unknown = diagnostics
            .iter()
            .find(|d| d.message.contains("pages.svae"))
            .unwrap();
        assert!(unknown.file.ends_with("app/components/SaveButton.asx"));
        assert_eq!(unknown.code, "axonyx-i18n");
        assert!(ensure_no_melt_diagnostics_for(&app.0, "build").is_err());
    }
}

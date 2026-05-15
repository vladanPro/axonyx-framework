use std::ffi::OsString;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use axonyx_core::ax_ast_prelude::AxImport;
use axonyx_core::ax_backend_ast_prelude::AxBackendBlock;
use axonyx_core::ax_backend_codegen_prelude::compile_backend_sources_to_module;
use axonyx_core::ax_backend_parser_prelude::{parse_backend_ax, AxBackendParseError};
use axonyx_core::ax_lowering_prelude::AxValue;
use axonyx_core::ax_parser_auto_prelude::{parse_ax_auto, AxAutoParseError, AxConvertV2Error};
use axonyx_core::ax_parser_prelude::AxParseError;
use axonyx_core::ax_parser_v2_prelude::{parse_ax_v2, AxParseV2Error};
use axonyx_core::ax_semantics_v2_prelude::AxSemanticV2Error;
use axonyx_core::ax_types_prelude::{check_document_types, AxDataContext};
use axonyx_runtime::{
    execute_preview_action_sources, execute_preview_route_sources,
    preview_ax_route_with_request_context_and_imports, AxPreviewHttpResponse, AxPreviewStore,
};
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

const DOCS_LAYOUT_AX: &str = include_str!("../templates/docs/app/docs/layout.ax.tpl");
const DOCS_HOME_AX: &str = include_str!("../templates/docs/app/docs/page.ax.tpl");
const DOCS_GETTING_STARTED_AX: &str =
    include_str!("../templates/docs/app/docs/getting-started/page.ax.tpl");
const DOCS_REFERENCE_AX: &str = include_str!("../templates/docs/app/docs/reference/page.ax.tpl");
const DOCS_EXAMPLES_AX: &str = include_str!("../templates/docs/app/docs/examples/page.ax.tpl");
const AXONYX_UI_VERSION: &str = "0.0.33";
static CARGO_PACKAGE_ROOT_CACHE: OnceLock<Mutex<std::collections::HashMap<String, PathBuf>>> =
    OnceLock::new();

#[derive(Debug, Parser)]
#[command(name = "ax")]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Add(AddArgs),
    Build(BuildArgs),
    Check(CheckArgs),
    Content(ContentArgs),
    Dev(DevArgs),
    Doctor(DoctorArgs),
    Routes(RoutesArgs),
    Run(RunArgs),
    Schema(SchemaArgs),
}

#[derive(Debug, Parser)]
struct AddArgs {
    #[arg(value_enum)]
    module: ModuleKind,
}

#[derive(Debug, Parser, Default)]
struct BuildArgs {
    /// Output directory for static HTML and public assets.
    #[arg(long, default_value = "dist")]
    out_dir: PathBuf,

    /// Remove the output directory before generating build artifacts.
    #[arg(long)]
    clean: bool,
}

#[derive(Debug, Parser)]
struct CheckArgs {
    /// Check a single .ax file instead of all app/routes/jobs sources.
    #[arg(long)]
    file: Option<PathBuf>,

    /// Output format for diagnostics.
    #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
    format: CheckFormat,
}

#[derive(Debug, Parser)]
struct ContentArgs {
    /// Output format for the content manifest.
    #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
    format: CheckFormat,
}

#[derive(Debug, Parser)]
struct DevArgs {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 3000)]
    port: u16,
}

#[derive(Debug, Parser)]
struct DoctorArgs {
    /// Output format for health checks.
    #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
    format: CheckFormat,

    /// Exit with a non-zero status when warnings are present.
    #[arg(long)]
    deny_warnings: bool,
}

#[derive(Debug, Parser)]
struct RoutesArgs {
    /// Output format for the route manifest.
    #[arg(long, value_enum, default_value_t = CheckFormat::Text)]
    format: CheckFormat,
}

#[derive(Debug, Parser)]
struct RunArgs {
    #[command(subcommand)]
    command: RunCommands,
}

#[derive(Debug, Parser)]
struct SchemaArgs {
    #[command(subcommand)]
    command: SchemaCommands,
}

#[derive(Debug, Subcommand)]
enum RunCommands {
    Dev(DevArgs),
    Start(DevArgs),
}

#[derive(Debug, Subcommand)]
enum SchemaCommands {
    Pull(SchemaPullArgs),
}

#[derive(Debug, Parser)]
struct SchemaPullArgs {
    /// JSON file path, inline JSON, or local http:// endpoint to inspect.
    source: String,

    /// Root record name for generated .ax output.
    #[arg(long, default_value = "Item")]
    name: String,

    /// Output format for the inferred schema.
    #[arg(long, value_enum, default_value_t = SchemaFormat::Ax)]
    format: SchemaFormat,

    /// Write output to a file instead of stdout.
    #[arg(long)]
    out: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CheckFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum SchemaFormat {
    Ax,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ModuleKind {
    Docs,
    Ui,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedRoute {
    request_path: String,
    request_target: String,
    page_path: PathBuf,
    layout_paths: Vec<PathBuf>,
    loader_path: Option<PathBuf>,
    actions_path: Option<PathBuf>,
    params: std::collections::BTreeMap<String, String>,
}

struct StaticAsset {
    content_type: &'static str,
    body: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum DoctorSeverity {
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct DoctorCheck {
    code: &'static str,
    severity: DoctorSeverity,
    message: String,
    hint: Option<&'static str>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct DoctorSummary {
    ok: usize,
    warn: usize,
    error: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PrerenderRoute {
    route: String,
    params: Vec<std::collections::BTreeMap<String, String>>,
}

struct DevServerState {
    root: PathBuf,
    preview_store: Mutex<AxPreviewStore>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerMode {
    Dev,
    Start,
}

impl ServerMode {
    fn inject_dev_client(self) -> bool {
        matches!(self, ServerMode::Dev)
    }

    fn label(self) -> &'static str {
        match self {
            ServerMode::Dev => "dev",
            ServerMode::Start => "start",
        }
    }
}

struct HttpRequest {
    method: String,
    target: String,
    headers: std::collections::BTreeMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug)]
enum BackendBuildStatus {
    Generated {
        source_count: usize,
        output_path: PathBuf,
    },
    NoSources {
        output_path: PathBuf,
    },
}

#[derive(Debug)]
enum StaticBuildStatus {
    Generated {
        route_count: usize,
        prerendered_count: usize,
        skipped_dynamic_count: usize,
        content_collection_count: usize,
        output_dir: PathBuf,
    },
    NoPages {
        skipped_dynamic_count: usize,
        content_collection_count: usize,
        output_dir: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize)]
struct CheckDiagnostic {
    file: String,
    line: usize,
    column: usize,
    severity: &'static str,
    code: &'static str,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct RouteManifestItem {
    kind: &'static str,
    route: String,
    method: Option<String>,
    file: String,
    layouts: Vec<String>,
    loader: Option<String>,
    actions: Option<String>,
    params: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ContentManifest {
    collections: Vec<ContentCollectionManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ContentCollectionManifest {
    name: String,
    path: String,
    extensions: Vec<String>,
    entries: Vec<ContentEntryManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ContentEntryManifest {
    path: String,
    slug: String,
    extension: String,
    content_type: String,
    bytes: u64,
    title: String,
    excerpt: String,
    word_count: usize,
    frontmatter: std::collections::BTreeMap<String, String>,
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct InferredSchema {
    root_type: String,
    records: Vec<InferredRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct InferredRecord {
    name: String,
    fields: Vec<InferredField>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct InferredField {
    name: String,
    ty: String,
    optional: bool,
}

pub fn main_entry() {
    if let Err(error) = run() {
        print_cli_error(&error);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse_from(normalized_cli_args());

    match cli.command {
        Commands::Add(args) => add_module(args.module),
        Commands::Build(args) => build_command(args),
        Commands::Check(args) => check_command(args),
        Commands::Content(args) => content_command(args),
        Commands::Dev(args) => run_dev_server(args),
        Commands::Doctor(args) => doctor_command(args),
        Commands::Routes(args) => routes_command(args),
        Commands::Run(args) => run_command(args),
        Commands::Schema(args) => schema_command(args),
    }
}

fn normalized_cli_args() -> Vec<OsString> {
    let mut args = std::env::args_os().collect::<Vec<_>>();

    if args
        .get(1)
        .and_then(|value| value.to_str())
        .is_some_and(|value| matches!(value, "ax" | "axonyx"))
    {
        args.remove(1);
    }

    args
}

fn build_command(args: BuildArgs) -> Result<()> {
    let root = app_root()?;
    ensure_no_check_diagnostics(&root)?;
    let status = compile_backend_from_app_root(&root)?;
    let static_status = build_static_site_from_app_root(&root, &args.out_dir, args.clean)?;
    print_backend_build_status(&status);
    print_static_build_status(&static_status);
    Ok(())
}

fn ensure_no_check_diagnostics(root: &Path) -> Result<()> {
    let diagnostics = check_app_sources(root)?;
    if diagnostics.is_empty() {
        return Ok(());
    }

    let mut message = String::from("Axonyx diagnostics failed before build:\n");
    for diagnostic in &diagnostics {
        message.push_str("  ");
        message.push_str(&format_check_diagnostic(diagnostic));
        message.push('\n');
    }

    bail!("{}", message.trim_end());
}

fn check_command(args: CheckArgs) -> Result<()> {
    let diagnostics = if let Some(file) = args.file {
        check_ax_file(&file)?
    } else {
        let root = app_root()?;
        check_app_sources(&root)?
    };

    match args.format {
        CheckFormat::Text => print_check_text(&diagnostics),
        CheckFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&diagnostics)?);
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

fn run_command(args: RunArgs) -> Result<()> {
    match args.command {
        RunCommands::Dev(args) => run_dev_server(args),
        RunCommands::Start(args) => run_start_server(args),
    }
}

fn doctor_command(args: DoctorArgs) -> Result<()> {
    let root = app_root()?;
    let checks = doctor_checks(&root);

    match args.format {
        CheckFormat::Text => print_doctor_text(&checks),
        CheckFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&checks)?);
        }
    }

    if doctor_should_fail(&checks, args.deny_warnings) {
        std::process::exit(1);
    }

    Ok(())
}

fn doctor_checks(root: &Path) -> Vec<DoctorCheck> {
    let mut checks = Vec::new();

    checks.push(doctor_file_check(
        root.join("Axonyx.toml").exists(),
        "axonyx-config",
        "Axonyx.toml found.",
        "Axonyx.toml is missing.",
        Some("Run this command from an Axonyx app root or create an app with create-axonyx."),
    ));
    checks.push(doctor_file_check(
        root.join("Cargo.toml").exists(),
        "cargo-manifest",
        "Cargo.toml found.",
        "Cargo.toml is missing.",
        Some("Axonyx apps need a Cargo manifest for runtime and package dependencies."),
    ));
    checks.push(doctor_file_check(
        root.join("app").exists(),
        "app-directory",
        "app/ directory found.",
        "app/ directory is missing.",
        Some("Create app/page.ax or scaffold a template with create-axonyx."),
    ));

    let cargo_source = fs::read_to_string(root.join("Cargo.toml")).ok();
    checks.push(match cargo_source.as_deref() {
        Some(source) if cargo_manifest_has_dependency(source, "axonyx-runtime") => DoctorCheck {
            code: "runtime-dependency",
            severity: DoctorSeverity::Ok,
            message: "axonyx-runtime dependency found.".to_string(),
            hint: None,
        },
        Some(_) => DoctorCheck {
            code: "runtime-dependency",
            severity: DoctorSeverity::Error,
            message: "axonyx-runtime dependency is missing.".to_string(),
            hint: Some("Add axonyx-runtime to Cargo.toml or recreate the app with create-axonyx."),
        },
        None => DoctorCheck {
            code: "runtime-dependency",
            severity: DoctorSeverity::Warn,
            message: "Could not inspect runtime dependency because Cargo.toml is missing."
                .to_string(),
            hint: None,
        },
    });

    let axonyx_source = fs::read_to_string(root.join("Axonyx.toml")).ok();
    let ui_enabled = axonyx_source
        .as_deref()
        .is_some_and(|source| source.contains("\"ui\"") || source.contains("'ui'"));
    if ui_enabled || root.join("vendor/axonyx-ui").exists() {
        checks.extend(doctor_ui_checks(root, cargo_source.as_deref()));
    }

    checks.push(doctor_ax_sources_check(root));

    checks
}

fn doctor_file_check(
    condition: bool,
    code: &'static str,
    ok_message: &str,
    error_message: &str,
    hint: Option<&'static str>,
) -> DoctorCheck {
    if condition {
        DoctorCheck {
            code,
            severity: DoctorSeverity::Ok,
            message: ok_message.to_string(),
            hint: None,
        }
    } else {
        DoctorCheck {
            code,
            severity: DoctorSeverity::Error,
            message: error_message.to_string(),
            hint,
        }
    }
}

fn doctor_ui_checks(root: &Path, cargo_source: Option<&str>) -> Vec<DoctorCheck> {
    let mut checks = Vec::new();
    let package_root = resolve_package_asset_root(root, "axonyx-ui");

    checks.push(match package_root.as_ref() {
        Some(root) => DoctorCheck {
            code: "ui-package",
            severity: DoctorSeverity::Ok,
            message: format!("axonyx-ui package resolved at '{}'.", root.display()),
            hint: None,
        },
        None => DoctorCheck {
            code: "ui-package",
            severity: DoctorSeverity::Warn,
            message: "Axonyx UI package could not be resolved.".to_string(),
            hint: Some("Run `cargo ax add ui` or add axonyx-ui to Cargo.toml."),
        },
    });

    checks.push(match cargo_source {
        Some(source) if cargo_manifest_has_dependency(source, "axonyx-ui") => DoctorCheck {
            code: "ui-cargo-dependency",
            severity: DoctorSeverity::Ok,
            message: "axonyx-ui Cargo dependency found.".to_string(),
            hint: None,
        },
        Some(_) => DoctorCheck {
            code: "ui-cargo-dependency",
            severity: DoctorSeverity::Warn,
            message: "UI module is present but axonyx-ui is not listed in Cargo.toml.".to_string(),
            hint: Some("Run `cargo ax add ui` to add the published axonyx-ui dependency."),
        },
        None => DoctorCheck {
            code: "ui-cargo-dependency",
            severity: DoctorSeverity::Warn,
            message: "Could not inspect axonyx-ui dependency because Cargo.toml is missing."
                .to_string(),
            hint: None,
        },
    });

    checks.push(
        match package_root
            .as_deref()
            .and_then(|root| cargo_package_ax_root(root, "@axonyx/ui"))
        {
            Some(_) => DoctorCheck {
                code: "ui-package-metadata",
                severity: DoctorSeverity::Ok,
                message: "Axonyx UI package metadata found.".to_string(),
                hint: None,
            },
            None => DoctorCheck {
                code: "ui-package-metadata",
                severity: DoctorSeverity::Warn,
                message: "Axonyx UI package metadata was not found or did not match @axonyx/ui."
                    .to_string(),
                hint: Some("Update axonyx-ui or rerun `cargo ax add ui`."),
            },
        },
    );

    let layout_source = fs::read_to_string(root.join("app/layout.ax")).ok();
    checks.push(match layout_source.as_deref() {
        Some(source) if source.contains("/_ax/pkg/axonyx-ui/index.css") => DoctorCheck {
            code: "ui-stylesheet",
            severity: DoctorSeverity::Ok,
            message: "Canonical Axonyx UI stylesheet link found.".to_string(),
            hint: None,
        },
        Some(source) if source.contains("/css/axonyx-ui/index.css") => DoctorCheck {
            code: "ui-stylesheet",
            severity: DoctorSeverity::Warn,
            message: "Legacy Axonyx UI stylesheet link found.".to_string(),
            hint: Some("Prefer /_ax/pkg/axonyx-ui/index.css for package-served CSS."),
        },
        Some(_) => DoctorCheck {
            code: "ui-stylesheet",
            severity: DoctorSeverity::Warn,
            message: "Axonyx UI stylesheet link is missing from app/layout.ax.".to_string(),
            hint: Some("Run `cargo ax add ui` or add /_ax/pkg/axonyx-ui/index.css to <Head>."),
        },
        None => DoctorCheck {
            code: "ui-stylesheet",
            severity: DoctorSeverity::Warn,
            message: "Could not inspect UI stylesheet because app/layout.ax is missing."
                .to_string(),
            hint: None,
        },
    });

    checks.push(
        match load_package_asset(root, "/_ax/pkg/axonyx-ui/index.css") {
            Ok(Some(_)) => DoctorCheck {
                code: "ui-package-css",
                severity: DoctorSeverity::Ok,
                message: "Axonyx UI package CSS can be served.".to_string(),
                hint: None,
            },
            Ok(None) => DoctorCheck {
                code: "ui-package-css",
                severity: DoctorSeverity::Warn,
                message: "Axonyx UI package CSS could not be found.".to_string(),
                hint: Some(
                    "Run `cargo ax add ui` or check that axonyx-ui exposes src/css/index.css.",
                ),
            },
            Err(error) => DoctorCheck {
                code: "ui-package-css",
                severity: DoctorSeverity::Warn,
                message: format!("Axonyx UI package CSS check failed: {error}"),
                hint: Some("Check the package asset path and Axonyx UI package metadata."),
            },
        },
    );

    checks
}

fn doctor_ax_sources_check(root: &Path) -> DoctorCheck {
    match check_app_sources(root) {
        Ok(diagnostics) if diagnostics.is_empty() => DoctorCheck {
            code: "ax-sources",
            severity: DoctorSeverity::Ok,
            message: "Axonyx source diagnostics passed.".to_string(),
            hint: None,
        },
        Ok(diagnostics) => DoctorCheck {
            code: "ax-sources",
            severity: DoctorSeverity::Error,
            message: format!(
                "{} Axonyx source diagnostic{} found.",
                diagnostics.len(),
                if diagnostics.len() == 1 { "" } else { "s" }
            ),
            hint: Some("Run `cargo ax check` to see file-level diagnostics."),
        },
        Err(error) => DoctorCheck {
            code: "ax-sources",
            severity: DoctorSeverity::Error,
            message: format!("Axonyx source diagnostics failed: {error}"),
            hint: Some("Run `cargo ax check` for more details."),
        },
    }
}

fn cargo_manifest_has_dependency(source: &str, dependency_name: &str) -> bool {
    source
        .parse::<toml::Value>()
        .ok()
        .and_then(|value| {
            value
                .get("dependencies")
                .and_then(toml::Value::as_table)
                .cloned()
        })
        .is_some_and(|dependencies| dependencies.contains_key(dependency_name))
}

fn print_doctor_text(checks: &[DoctorCheck]) {
    println!("Axonyx doctor");
    for check in checks {
        let label = match check.severity {
            DoctorSeverity::Ok => "ok",
            DoctorSeverity::Warn => "warn",
            DoctorSeverity::Error => "error",
        };
        println!("[{label}] {}: {}", check.code, check.message);
        if let Some(hint) = check.hint {
            println!("       hint: {hint}");
        }
    }

    let summary = doctor_summary(checks);
    println!(
        "Summary: {} ok, {} warning{}, {} error{}",
        summary.ok,
        summary.warn,
        if summary.warn == 1 { "" } else { "s" },
        summary.error,
        if summary.error == 1 { "" } else { "s" }
    );
}

fn doctor_should_fail(checks: &[DoctorCheck], deny_warnings: bool) -> bool {
    checks
        .iter()
        .any(|check| check.severity == DoctorSeverity::Error)
        || (deny_warnings
            && checks
                .iter()
                .any(|check| check.severity == DoctorSeverity::Warn))
}

fn doctor_summary(checks: &[DoctorCheck]) -> DoctorSummary {
    checks
        .iter()
        .fold(DoctorSummary::default(), |mut summary, check| {
            match check.severity {
                DoctorSeverity::Ok => summary.ok += 1,
                DoctorSeverity::Warn => summary.warn += 1,
                DoctorSeverity::Error => summary.error += 1,
            }
            summary
        })
}

fn routes_command(args: RoutesArgs) -> Result<()> {
    let root = app_root()?;
    let routes = collect_app_route_manifest(&root)?;

    match args.format {
        CheckFormat::Text => print_routes_text(&routes),
        CheckFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&routes)?);
        }
    }

    Ok(())
}

fn content_command(args: ContentArgs) -> Result<()> {
    let root = app_root()?;
    let manifest = collect_content_manifest(&root)?;

    match args.format {
        CheckFormat::Text => print_content_text(&manifest),
        CheckFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        }
    }

    Ok(())
}

fn schema_command(args: SchemaArgs) -> Result<()> {
    match args.command {
        SchemaCommands::Pull(args) => schema_pull_command(args),
    }
}

fn schema_pull_command(args: SchemaPullArgs) -> Result<()> {
    let source = read_schema_source(&args.source)?;
    let value = serde_json::from_str::<serde_json::Value>(&source)
        .with_context(|| format!("failed to parse schema source '{}' as JSON", args.source))?;
    let schema = match schema_from_typed_envelope(&args.name, &value)? {
        Some(schema) => schema,
        None => infer_schema_from_json(&args.name, &value)
            .with_context(|| "failed to infer schema from JSON source")?,
    };

    let rendered = match args.format {
        SchemaFormat::Ax => render_schema_as_ax(&schema),
        SchemaFormat::Json => serde_json::to_string_pretty(&schema)?,
    };

    if let Some(path) = args.out {
        fs::write(&path, rendered)
            .with_context(|| format!("failed to write schema output '{}'", path.display()))?;
    } else {
        println!("{rendered}");
    }

    Ok(())
}

fn read_schema_source(source: &str) -> Result<String> {
    if source.starts_with("http://") {
        return read_http_text(source);
    }

    let path = Path::new(source);
    if path.exists() {
        return fs::read_to_string(path)
            .with_context(|| format!("failed to read schema source '{}'", path.display()));
    }

    if source.trim_start().starts_with('{') || source.trim_start().starts_with('[') {
        return Ok(source.to_string());
    }

    bail!(
        "schema source '{}' is not a file, inline JSON, or http:// endpoint",
        source
    )
}

fn read_http_text(url: &str) -> Result<String> {
    let request = parse_http_url(url)?;
    let mut stream = TcpStream::connect((request.host.as_str(), request.port))
        .with_context(|| format!("failed to connect to {}", request.authority))?;
    let request_text = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        request.path, request.authority
    );
    stream
        .write_all(request_text.as_bytes())
        .with_context(|| format!("failed to send request to {url}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .with_context(|| format!("failed to read response from {url}"))?;

    let Some((head, body)) = response.split_once("\r\n\r\n") else {
        bail!("invalid HTTP response from {url}");
    };
    let status = head.lines().next().unwrap_or_default();
    if !status.contains(" 200 ") {
        bail!("schema endpoint returned non-200 status: {status}");
    }

    Ok(body.to_string())
}

struct ParsedHttpUrl {
    authority: String,
    host: String,
    port: u16,
    path: String,
}

fn parse_http_url(url: &str) -> Result<ParsedHttpUrl> {
    let Some(rest) = url.strip_prefix("http://") else {
        bail!("only http:// schema endpoints are supported in this draft")
    };
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.is_empty() {
        bail!("schema endpoint host is empty")
    }
    let (host, port) = if let Some((host, port)) = authority.rsplit_once(':') {
        (
            host.to_string(),
            port.parse::<u16>()
                .with_context(|| format!("invalid schema endpoint port '{port}'"))?,
        )
    } else {
        (authority.to_string(), 80)
    };
    if host.is_empty() {
        bail!("schema endpoint host is empty")
    }

    Ok(ParsedHttpUrl {
        authority: authority.to_string(),
        host,
        port,
        path: format!("/{path}"),
    })
}

fn infer_schema_from_json(root_name: &str, value: &serde_json::Value) -> Result<InferredSchema> {
    let mut records = Vec::new();
    let root_type = infer_value_type(root_name, value, &mut records)?;
    Ok(InferredSchema { root_type, records })
}

fn schema_from_typed_envelope(
    root_name: &str,
    value: &serde_json::Value,
) -> Result<Option<InferredSchema>> {
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    let Some(schema_value) = object.get("schema") else {
        return Ok(None);
    };
    let schema_object = schema_value
        .as_object()
        .with_context(|| "typed schema envelope field 'schema' must be an object")?;

    let mut records = Vec::new();
    for (record_name, fields_value) in schema_object {
        let fields_object = fields_value.as_object().with_context(|| {
            format!("typed schema record '{record_name}' must be an object of fields")
        })?;
        let mut fields = Vec::new();
        for (field_name, field_value) in fields_object {
            let (ty, optional) = parse_schema_field_type(field_name, field_value)?;
            fields.push(InferredField {
                name: field_name.to_string(),
                ty,
                optional,
            });
        }
        records.push(InferredRecord {
            name: sanitize_type_name(record_name),
            fields,
        });
    }

    let root_type = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            object
                .get("data")
                .and_then(|data| infer_schema_from_json(root_name, data).ok())
                .map(|schema| schema.root_type)
        })
        .unwrap_or_else(|| sanitize_type_name(root_name));

    Ok(Some(InferredSchema { root_type, records }))
}

fn parse_schema_field_type(field_name: &str, value: &serde_json::Value) -> Result<(String, bool)> {
    if let Some(ty) = value.as_str() {
        return Ok(normalize_schema_type(ty));
    }

    let Some(object) = value.as_object() else {
        bail!("typed schema field '{field_name}' must be a type string or object")
    };
    let ty = object
        .get("type")
        .and_then(serde_json::Value::as_str)
        .with_context(|| format!("typed schema field '{field_name}' is missing string 'type'"))?;
    let (ty, wrapped_optional) = normalize_schema_type(ty);
    let optional = object
        .get("optional")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(wrapped_optional);
    Ok((ty, optional))
}

fn normalize_schema_type(ty: &str) -> (String, bool) {
    let trimmed = ty.trim();
    if let Some(inner) = trimmed
        .strip_prefix("Optional<")
        .and_then(|value| value.strip_suffix('>'))
    {
        (inner.trim().to_string(), true)
    } else {
        (trimmed.to_string(), false)
    }
}

fn infer_value_type(
    name_hint: &str,
    value: &serde_json::Value,
    records: &mut Vec<InferredRecord>,
) -> Result<String> {
    Ok(match value {
        serde_json::Value::Null => "Unknown".to_string(),
        serde_json::Value::Bool(_) => "Bool".to_string(),
        serde_json::Value::Number(_) => "Number".to_string(),
        serde_json::Value::String(_) => "String".to_string(),
        serde_json::Value::Array(items) => {
            let item_name = singular_type_name(name_hint);
            let item_type = infer_array_item_type(&item_name, items, records)?;
            format!("List<{item_type}>")
        }
        serde_json::Value::Object(map) => {
            let record_name = sanitize_type_name(name_hint);
            let objects = vec![map];
            let record = infer_record(&record_name, &objects, records)?;
            records.push(record);
            record_name
        }
    })
}

fn infer_array_item_type(
    name_hint: &str,
    items: &[serde_json::Value],
    records: &mut Vec<InferredRecord>,
) -> Result<String> {
    let objects = items
        .iter()
        .filter_map(serde_json::Value::as_object)
        .collect::<Vec<_>>();
    if !objects.is_empty() && objects.len() == items.len() {
        let record_name = sanitize_type_name(name_hint);
        let record = infer_record(&record_name, &objects, records)?;
        records.push(record);
        return Ok(record_name);
    }

    let mut ty = None;
    let mut optional = false;
    for item in items {
        if item.is_null() {
            optional = true;
            continue;
        }
        let next = infer_value_type(name_hint, item, records)?;
        ty = Some(match ty {
            None => next,
            Some(current) if current == next => current,
            Some(_) => "Unknown".to_string(),
        });
    }

    let ty = ty.unwrap_or_else(|| "Unknown".to_string());
    Ok(if optional {
        format!("Optional<{ty}>")
    } else {
        ty
    })
}

fn infer_record(
    record_name: &str,
    objects: &[&serde_json::Map<String, serde_json::Value>],
    records: &mut Vec<InferredRecord>,
) -> Result<InferredRecord> {
    let mut keys = std::collections::BTreeSet::new();
    for object in objects {
        keys.extend(object.keys().cloned());
    }

    let mut fields = Vec::new();
    for key in keys {
        let mut values = Vec::new();
        let mut optional = false;
        for object in objects {
            match object.get(&key) {
                Some(value) if value.is_null() => optional = true,
                Some(value) => values.push(value),
                None => optional = true,
            }
        }

        let field_type = infer_field_type(record_name, &key, &values, records)?;
        fields.push(InferredField {
            name: key,
            ty: field_type,
            optional,
        });
    }

    Ok(InferredRecord {
        name: record_name.to_string(),
        fields,
    })
}

fn infer_field_type(
    record_name: &str,
    field_name: &str,
    values: &[&serde_json::Value],
    records: &mut Vec<InferredRecord>,
) -> Result<String> {
    if values.is_empty() {
        return Ok("Unknown".to_string());
    }

    let nested_name = format!("{record_name}{}", sanitize_type_name(field_name));
    let mut ty = None;
    for value in values {
        let next = infer_value_type(&nested_name, value, records)?;
        ty = Some(match ty {
            None => next,
            Some(current) if current == next => current,
            Some(_) => "Unknown".to_string(),
        });
    }
    Ok(ty.unwrap_or_else(|| "Unknown".to_string()))
}

fn render_schema_as_ax(schema: &InferredSchema) -> String {
    let mut out = String::new();
    for record in &schema.records {
        out.push_str(&format!("type {} {{\n", record.name));
        for field in &record.fields {
            if field.optional {
                out.push_str(&format!("  {}?: {}\n", field.name, field.ty));
            } else {
                out.push_str(&format!("  {}: {}\n", field.name, field.ty));
            }
        }
        out.push_str("}\n\n");
    }
    out.push_str(&format!("// root: {}\n", schema.root_type));
    out.trim_end().to_string()
}

fn sanitize_type_name(input: &str) -> String {
    let mut out = String::new();
    let mut capitalize_next = true;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if capitalize_next {
                out.push(ch.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                out.push(ch);
            }
        } else {
            capitalize_next = true;
        }
    }
    if out.is_empty() {
        "Item".to_string()
    } else if out
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit())
    {
        format!("T{out}")
    } else {
        out
    }
}

fn singular_type_name(input: &str) -> String {
    let name = sanitize_type_name(input);
    name.strip_suffix('s').unwrap_or(&name).to_string()
}

fn print_content_text(manifest: &ContentManifest) {
    if manifest.collections.is_empty() {
        println!("No content collections configured.");
        println!("Add [content.collections.<name>] entries to Axonyx.toml.");
        return;
    }

    println!("Content collections:");
    for collection in &manifest.collections {
        println!(
            "  {:<18} path={} entries={} extensions={}",
            collection.name,
            collection.path,
            collection.entries.len(),
            collection.extensions.join(",")
        );

        for entry in &collection.entries {
            println!(
                "    {:<32} slug={} title=\"{}\" words={} bytes={}",
                entry.path, entry.slug, entry.title, entry.word_count, entry.bytes
            );
            if !entry.excerpt.is_empty() {
                println!("      {}", entry.excerpt);
            }
        }
    }
}

fn collect_content_manifest(root: &Path) -> Result<ContentManifest> {
    let configs = load_content_collection_configs(root)?;
    let mut collections = Vec::new();

    for config in configs {
        let entries = collect_content_entries(root, &config)?;
        collections.push(ContentCollectionManifest {
            name: config.name,
            path: display_relative_path(root, &config.path),
            extensions: config.extensions,
            entries,
        });
    }

    Ok(ContentManifest { collections })
}

fn preview_store_from_content(root: &Path) -> Result<AxPreviewStore> {
    let manifest = collect_content_manifest(root)?;
    let mut store = AxPreviewStore::default();

    for collection in manifest.collections {
        let items = collection
            .entries
            .into_iter()
            .map(content_entry_to_record)
            .collect();
        store = store.with_collection(collection.name, items);
    }

    Ok(store)
}

fn content_entry_to_record(entry: ContentEntryManifest) -> AxValue {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("path".to_string(), AxValue::from(entry.path));
    fields.insert("slug".to_string(), AxValue::from(entry.slug));
    fields.insert("extension".to_string(), AxValue::from(entry.extension));
    fields.insert(
        "content_type".to_string(),
        AxValue::from(entry.content_type),
    );
    fields.insert("bytes".to_string(), AxValue::from(entry.bytes as i64));
    fields.insert("title".to_string(), AxValue::from(entry.title));
    fields.insert("excerpt".to_string(), AxValue::from(entry.excerpt));
    fields.insert(
        "word_count".to_string(),
        AxValue::from(entry.word_count as i64),
    );
    fields.insert("body".to_string(), AxValue::from(entry.body));
    for (key, value) in entry.frontmatter {
        fields.insert(key, AxValue::from(value));
    }
    AxValue::Record(fields)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContentCollectionConfig {
    name: String,
    path: PathBuf,
    extensions: Vec<String>,
}

fn load_content_collection_configs(root: &Path) -> Result<Vec<ContentCollectionConfig>> {
    let Some(collections_value) = axonyx_config_value(root, "content", "collections") else {
        return Ok(Vec::new());
    };

    let collections = collections_value
        .as_table()
        .ok_or_else(|| anyhow::anyhow!("[content].collections must be a TOML table"))?;
    let mut out = Vec::new();

    for (name, value) in collections {
        let table = value
            .as_table()
            .ok_or_else(|| anyhow::anyhow!("[content.collections.{name}] must be a TOML table"))?;
        let path = table
            .get("path")
            .and_then(toml::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("[content.collections.{name}] is missing path"))?;
        let path = resolve_content_collection_path(root, path)?;
        let extensions = table
            .get("extensions")
            .map(content_extensions_from_value)
            .transpose()?
            .unwrap_or_else(|| vec!["md".to_string(), "mdx".to_string()]);

        out.push(ContentCollectionConfig {
            name: name.to_string(),
            path,
            extensions,
        });
    }

    out.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(out)
}

fn content_extensions_from_value(value: &toml::Value) -> Result<Vec<String>> {
    let values = value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("content collection extensions must be an array"))?;
    let mut extensions = Vec::new();

    for value in values {
        let extension = value
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("content collection extension must be a string"))?
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase();
        if extension.is_empty()
            || extension.contains('/')
            || extension.contains('\\')
            || extension == "."
            || extension == ".."
        {
            bail!("invalid content collection extension '{extension}'");
        }
        if !extensions.contains(&extension) {
            extensions.push(extension);
        }
    }

    Ok(extensions)
}

fn resolve_content_collection_path(root: &Path, path: &str) -> Result<PathBuf> {
    if path.contains('\0') {
        bail!("content collection path contains an invalid null byte");
    }

    let path = PathBuf::from(path);
    let resolved = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };

    let normalized = normalize_content_path(&resolved)?;
    let root = normalize_content_path(root)?;
    if !normalized.starts_with(&root) {
        bail!(
            "content collection path '{}' must stay inside app root '{}'",
            normalized.display(),
            root.display()
        );
    }

    Ok(normalized)
}

fn normalize_content_path(path: &Path) -> Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            std::path::Component::RootDir => normalized.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    bail!("content collection path cannot escape app root");
                }
            }
            std::path::Component::Normal(segment) => normalized.push(segment),
        }
    }
    Ok(normalized)
}

fn collect_content_entries(
    root: &Path,
    config: &ContentCollectionConfig,
) -> Result<Vec<ContentEntryManifest>> {
    if !config.path.exists() {
        return Ok(Vec::new());
    }
    if !config.path.is_dir() {
        bail!(
            "content collection '{}' path '{}' is not a directory",
            config.name,
            config.path.display()
        );
    }

    let mut entries = Vec::new();
    collect_content_entries_in_dir(root, config, &config.path, &mut entries)?;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn collect_content_entries_in_dir(
    root: &Path,
    config: &ContentCollectionConfig,
    dir: &Path,
    out: &mut Vec<ContentEntryManifest>,
) -> Result<()> {
    let mut children = fs::read_dir(dir)
        .with_context(|| format!("failed to read content directory '{}'", dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to read content directory '{}'", dir.display()))?;

    children.sort_by_key(|entry| entry.path());

    for entry in children {
        let path = entry.path();
        let file_name = entry.file_name();
        if file_name.to_string_lossy().starts_with('.') {
            continue;
        }

        if path.is_dir() {
            collect_content_entries_in_dir(root, config, &path, out)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .unwrap_or_default();
        if !config.extensions.contains(&extension) {
            continue;
        }

        let metadata = fs::metadata(&path)
            .with_context(|| format!("failed to inspect content file '{}'", path.display()))?;
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read content file '{}'", path.display()))?;
        let (frontmatter, body) = parse_content_frontmatter(&source);
        let title = content_title(&frontmatter, &body, &path);
        let excerpt = content_excerpt(&frontmatter, &body);
        out.push(ContentEntryManifest {
            path: display_relative_path(root, &path),
            slug: content_slug(&config.path, &path),
            content_type: content_type_for_extension(&extension).to_string(),
            extension,
            bytes: metadata.len(),
            title,
            excerpt,
            word_count: content_word_count(&body),
            frontmatter,
            body,
        });
    }

    Ok(())
}

fn parse_content_frontmatter(source: &str) -> (std::collections::BTreeMap<String, String>, String) {
    let Some(rest) = source
        .strip_prefix("---\n")
        .or_else(|| source.strip_prefix("---\r\n"))
    else {
        return (std::collections::BTreeMap::new(), source.to_string());
    };

    let Some((frontmatter_source, body)) = split_frontmatter_body(rest) else {
        return (std::collections::BTreeMap::new(), source.to_string());
    };

    let mut frontmatter = std::collections::BTreeMap::new();
    for line in frontmatter_source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !is_content_frontmatter_key(key) {
            continue;
        }
        frontmatter.insert(key.to_string(), trim_frontmatter_value(value));
    }

    (frontmatter, body.to_string())
}

fn split_frontmatter_body(source: &str) -> Option<(&str, &str)> {
    if let Some((frontmatter, body)) = source.split_once("\n---\n") {
        return Some((frontmatter, body));
    }
    if let Some((frontmatter, body)) = source.split_once("\r\n---\r\n") {
        return Some((frontmatter, body));
    }
    if let Some((frontmatter, body)) = source.split_once("\n---\r\n") {
        return Some((frontmatter, body));
    }
    source
        .split_once("\r\n---\n")
        .map(|(frontmatter, body)| (frontmatter, body))
}

fn is_content_frontmatter_key(key: &str) -> bool {
    key.chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn trim_frontmatter_value(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len().saturating_sub(1)].to_string()
    } else {
        value.to_string()
    }
}

fn content_type_for_extension(extension: &str) -> &'static str {
    match extension {
        "md" | "mdx" => "markdown",
        "html" | "htm" => "html",
        "json" => "json",
        _ => "text",
    }
}

fn content_title(
    frontmatter: &std::collections::BTreeMap<String, String>,
    body: &str,
    path: &Path,
) -> String {
    if let Some(title) = frontmatter
        .get("title")
        .filter(|value| !value.trim().is_empty())
    {
        return title.trim().to_string();
    }

    if let Some(heading) = first_markdown_heading(body) {
        return heading;
    }

    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(humanize_slug)
        .unwrap_or_default()
}

fn content_excerpt(frontmatter: &std::collections::BTreeMap<String, String>, body: &str) -> String {
    for key in ["excerpt", "summary", "description"] {
        if let Some(value) = frontmatter
            .get(key)
            .filter(|value| !value.trim().is_empty())
        {
            return collapse_ws(value);
        }
    }

    body.lines()
        .map(strip_markdown_line)
        .map(|line| collapse_ws(&line))
        .find(|line| !line.is_empty())
        .unwrap_or_default()
}

fn content_word_count(body: &str) -> usize {
    strip_markdown_line(body)
        .split_whitespace()
        .filter(|word| !word.trim().is_empty())
        .count()
}

fn first_markdown_heading(body: &str) -> Option<String> {
    body.lines().find_map(|line| {
        let line = line.trim();
        let heading = line.strip_prefix("# ")?;
        let heading = collapse_ws(heading);
        (!heading.is_empty()).then_some(heading)
    })
}

fn strip_markdown_line(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_html_tag = false;
    for ch in value.chars() {
        if in_html_tag {
            if ch == '>' {
                in_html_tag = false;
                out.push(' ');
            }
            continue;
        }
        match ch {
            '#' | '*' | '_' | '`' | '>' | '[' | ']' | '(' | ')' => out.push(' '),
            '<' => in_html_tag = true,
            _ => out.push(ch),
        }
    }
    out
}

fn collapse_ws(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn humanize_slug(value: &str) -> String {
    value
        .replace(['-', '_'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let mut out = first.to_uppercase().collect::<String>();
                    out.push_str(chars.as_str());
                    out
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn content_slug(collection_root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(collection_root).unwrap_or(path);
    let mut segments = relative
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(segment) => Some(segment.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>();

    if let Some(last) = segments.last_mut() {
        if let Some((stem, _)) = last.rsplit_once('.') {
            *last = stem.to_string();
        }
    }

    segments.join("/")
}

fn check_app_sources(root: &Path) -> Result<Vec<CheckDiagnostic>> {
    let mut files = Vec::new();
    collect_ax_files(&root.join("app"), &mut files)?;
    collect_ax_files(&root.join("routes"), &mut files)?;
    collect_ax_files(&root.join("jobs"), &mut files)?;

    let mut diagnostics = Vec::new();
    for file in files {
        diagnostics.extend(check_ax_file_with_root(&file, Some(root))?);
    }
    diagnostics.extend(check_route_manifest(root)?);

    Ok(diagnostics)
}

fn check_route_manifest(root: &Path) -> Result<Vec<CheckDiagnostic>> {
    let routes = collect_app_route_manifest(root)?;
    let mut seen = std::collections::BTreeMap::<String, RouteManifestItem>::new();
    let mut diagnostics = Vec::new();

    for route in routes {
        let key = route_conflict_key(&route);
        if let Some(existing) = seen.get(&key) {
            diagnostics.push(CheckDiagnostic {
                file: display_path(&root.join(&route.file)),
                line: 1,
                column: 1,
                severity: "error",
                code: "axonyx-route-duplicate",
                message: format!(
                    "duplicate {} route `{}` also defined in '{}'",
                    route.kind,
                    route_display_name(&route),
                    existing.file
                ),
            });
        } else {
            seen.insert(key, route);
        }
    }

    Ok(diagnostics)
}

fn route_conflict_key(route: &RouteManifestItem) -> String {
    let canonical_route = canonical_route_conflict_pattern(&route.route);
    match route.kind {
        "api" => format!(
            "api:{}:{}",
            route.method.as_deref().unwrap_or("*"),
            canonical_route
        ),
        _ => format!("page:{canonical_route}"),
    }
}

fn canonical_route_conflict_pattern(route: &str) -> String {
    route
        .split('/')
        .map(|segment| {
            if segment.starts_with(':') && segment.len() > 1 {
                ":param"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn route_display_name(route: &RouteManifestItem) -> String {
    route
        .method
        .as_ref()
        .map(|method| format!("{method} {}", route.route))
        .unwrap_or_else(|| route.route.clone())
}

fn collect_ax_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir).with_context(|| format!("failed to read '{}'", dir.display()))? {
        let entry =
            entry.with_context(|| format!("failed to read entry in '{}'", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect '{}'", path.display()))?;

        if file_type.is_dir() {
            collect_ax_files(&path, out)?;
            continue;
        }

        if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("ax") {
            out.push(path);
        }
    }

    Ok(())
}

fn check_ax_file(path: &Path) -> Result<Vec<CheckDiagnostic>> {
    let root = find_app_root_for_path(path);
    check_ax_file_with_root(path, root.as_deref())
}

fn check_ax_file_with_root(path: &Path, root: Option<&Path>) -> Result<Vec<CheckDiagnostic>> {
    let source = fs::read_to_string(path)
        .with_context(|| format!("failed to read .ax file '{}'", path.display()))?;
    Ok(check_ax_source_with_root(path, &source, root))
}

fn check_ax_source_with_root(
    path: &Path,
    source: &str,
    root: Option<&Path>,
) -> Vec<CheckDiagnostic> {
    if looks_like_backend_ax(source) {
        return match parse_backend_ax(source) {
            Ok(_) => Vec::new(),
            Err(error) => vec![diagnostic_from_parse_error(
                path,
                CheckParseError::Backend(error),
            )],
        };
    }

    let document = match parse_ax_auto(source) {
        Ok(document) => document,
        Err(error) => {
            return vec![diagnostic_from_parse_error(
                path,
                CheckParseError::Page(error),
            )]
        }
    };

    let mut diagnostics = Vec::new();
    diagnostics.extend(check_type_annotations(path, source, &document));
    if let Some(root) = root {
        diagnostics.extend(check_imports(root, path, source, &document.imports));
    }
    diagnostics
}

fn check_type_annotations(
    path: &Path,
    source: &str,
    document: &axonyx_core::ax_ast_prelude::AxDocument,
) -> Vec<CheckDiagnostic> {
    let Ok(file) = parse_ax_v2(source) else {
        return Vec::new();
    };
    if file.types.is_empty() && file.lets.iter().all(|binding| binding.ty.is_none()) {
        return Vec::new();
    }

    let context = match AxDataContext::from_v2_let_types(&file) {
        Ok(context) => context,
        Err(error) => {
            return vec![CheckDiagnostic {
                file: display_path(path),
                line: typed_let_line(source).unwrap_or(1),
                column: 1,
                severity: "error",
                code: "axonyx-type",
                message: error.to_string(),
            }];
        }
    };

    check_document_types(document, &context)
        .errors
        .into_iter()
        .map(|error| CheckDiagnostic {
            file: display_path(path),
            line: type_error_line(source, error.expression.as_deref())
                .or_else(|| typed_let_line(source))
                .unwrap_or(1),
            column: 1,
            severity: "error",
            code: "axonyx-type",
            message: match error.expression {
                Some(expression) => format!("`{expression}`: {}", error.message),
                None => format!("{}: {}", error.location, error.message),
            },
        })
        .collect()
}

fn type_error_line(source: &str, expression: Option<&str>) -> Option<usize> {
    let expression = expression?;
    source
        .lines()
        .enumerate()
        .find(|(_, line)| line.contains(expression))
        .map(|(index, _)| index + 1)
}

fn typed_let_line(source: &str) -> Option<usize> {
    source
        .lines()
        .enumerate()
        .find(|(_, line)| {
            let trimmed = line.trim_start();
            trimmed.starts_with("let ") && trimmed.contains(':')
        })
        .map(|(index, _)| index + 1)
}

fn find_app_root_for_path(path: &Path) -> Option<PathBuf> {
    let mut current = path.parent()?;

    loop {
        if current.join("Axonyx.toml").exists() {
            return Some(current.to_path_buf());
        }

        current = current.parent()?;
    }
}

fn check_imports(
    root: &Path,
    path: &Path,
    source: &str,
    imports: &[AxImport],
) -> Vec<CheckDiagnostic> {
    imports
        .iter()
        .filter_map(|import_decl| {
            let resolved = resolve_preview_import_path(root, &import_decl.source);
            let line = import_source_line(source, &import_decl.source);

            if let Some(import_path) = resolved.as_ref().filter(|path| path.exists()) {
                return validate_import_target(root, path, line, &import_decl.source, import_path);
            }

            let detail = resolved
                .as_ref()
                .map(|path| format!(" expected '{}'", display_path(path)))
                .unwrap_or_default();

            Some(CheckDiagnostic {
                file: display_path(path),
                line,
                column: 1,
                severity: "error",
                code: "axonyx-import",
                message: format!(
                    "unable to resolve import `{}`{}",
                    import_decl.source, detail
                ),
            })
        })
        .collect()
}

fn validate_import_target(
    root: &Path,
    importing_path: &Path,
    import_line: usize,
    import_source: &str,
    import_path: &Path,
) -> Option<CheckDiagnostic> {
    let mut stack = vec![canonical_path(importing_path)];
    let error = validate_import_path_recursive(root, import_path, &mut stack).err()?;

    let (code, message) = match error {
        ImportValidationError::Parse {
            path,
            line,
            message,
        } if same_path(&path, import_path) => (
            "axonyx-import-parse",
            format!(
                "import `{}` resolved to '{}' but that file is not valid .ax (line {}: {})",
                import_source,
                display_path(import_path),
                line,
                message
            ),
        ),
        ImportValidationError::Parse {
            path,
            line,
            message,
        } => (
            "axonyx-import-chain",
            format!(
                "import `{}` resolved to '{}' but its import chain is broken: '{}' is not valid .ax (line {}: {})",
                import_source,
                display_path(import_path),
                display_path(&path),
                line,
                message
            ),
        ),
        ImportValidationError::Missing {
            from_path,
            import_source: nested_source,
            expected,
        } => {
            let detail = expected
                .as_ref()
                .map(|path| format!(" expected '{}'", display_path(path)))
                .unwrap_or_default();
            (
                "axonyx-import-chain",
                format!(
                    "import `{}` resolved to '{}' but its import chain is broken: '{}' imports `{}`{}",
                    import_source,
                    display_path(import_path),
                    display_path(&from_path),
                    nested_source,
                    detail
                ),
            )
        }
        ImportValidationError::Cycle { chain } => (
            "axonyx-import-cycle",
            format!(
                "import `{}` resolved to '{}' but its import chain contains a cycle: {}",
                import_source,
                display_path(import_path),
                chain
                    .iter()
                    .map(|path| format!("'{}'", display_path(path)))
                    .collect::<Vec<_>>()
                    .join(" -> ")
            ),
        ),
    };

    Some(CheckDiagnostic {
        file: display_path(importing_path),
        line: import_line,
        column: 1,
        severity: "error",
        code,
        message,
    })
}

#[derive(Debug)]
enum ImportValidationError {
    Missing {
        from_path: PathBuf,
        import_source: String,
        expected: Option<PathBuf>,
    },
    Parse {
        path: PathBuf,
        line: usize,
        message: String,
    },
    Cycle {
        chain: Vec<PathBuf>,
    },
}

fn validate_import_path_recursive(
    root: &Path,
    path: &Path,
    stack: &mut Vec<PathBuf>,
) -> Result<(), ImportValidationError> {
    let canonical = canonical_path(path);
    if let Some(index) = stack.iter().position(|entry| same_path(entry, &canonical)) {
        let mut chain = stack[index..].to_vec();
        chain.push(canonical);
        return Err(ImportValidationError::Cycle { chain });
    }

    let source = fs::read_to_string(path).map_err(|_| ImportValidationError::Missing {
        from_path: path.to_path_buf(),
        import_source: String::new(),
        expected: Some(path.to_path_buf()),
    })?;
    let document = parse_ax_auto(&source).map_err(|error| ImportValidationError::Parse {
        path: path.to_path_buf(),
        line: line_from_auto_parse_error(&error).unwrap_or(1),
        message: message_from_auto_parse_error(&error),
    })?;

    stack.push(canonical);

    for import_decl in document.imports {
        let resolved = resolve_preview_import_path(root, &import_decl.source);
        let Some(import_path) = resolved.as_ref().filter(|path| path.exists()) else {
            stack.pop();
            return Err(ImportValidationError::Missing {
                from_path: path.to_path_buf(),
                import_source: import_decl.source,
                expected: resolved,
            });
        };

        if let Err(error) = validate_import_path_recursive(root, import_path, stack) {
            stack.pop();
            return Err(error);
        }
    }

    stack.pop();
    Ok(())
}

fn canonical_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn same_path(left: &Path, right: &Path) -> bool {
    canonical_path(left) == canonical_path(right)
}

fn import_source_line(source: &str, import_source: &str) -> usize {
    let double_quoted = format!("\"{import_source}\"");
    let single_quoted = format!("'{import_source}'");

    source
        .lines()
        .position(|line| line.contains(&double_quoted) || line.contains(&single_quoted))
        .map(|index| index + 1)
        .unwrap_or(1)
}

fn looks_like_backend_ax(source: &str) -> bool {
    source.lines().map(str::trim_start).any(|line| {
        line.starts_with("route ")
            || line.starts_with("loader ")
            || line.starts_with("action ")
            || line.starts_with("job ")
    })
}

enum CheckParseError {
    Page(AxAutoParseError),
    Backend(AxBackendParseError),
}

fn diagnostic_from_parse_error(path: &Path, error: CheckParseError) -> CheckDiagnostic {
    let (line, code, message) = match error {
        CheckParseError::Page(error) => (
            line_from_auto_parse_error(&error).unwrap_or(1),
            "axonyx-parse",
            message_from_auto_parse_error(&error),
        ),
        CheckParseError::Backend(error) => (
            line_from_backend_parse_error(&error).unwrap_or(1),
            "axonyx-backend-parse",
            error.to_string(),
        ),
    };

    CheckDiagnostic {
        file: display_path(path),
        line,
        column: 1,
        severity: "error",
        code,
        message,
    }
}

fn message_from_auto_parse_error(error: &AxAutoParseError) -> String {
    match error {
        AxAutoParseError::V1(error) => error.to_string(),
        AxAutoParseError::V2(error) => error.to_string(),
        AxAutoParseError::Semantic(error) => error.to_string(),
        AxAutoParseError::Convert(error) => error.to_string(),
    }
}

fn line_from_auto_parse_error(error: &AxAutoParseError) -> Option<usize> {
    match error {
        AxAutoParseError::V1(error) => line_from_ax_parse_error(error),
        AxAutoParseError::V2(error) => line_from_ax_parse_v2_error(error),
        AxAutoParseError::Semantic(error) => line_from_semantic_error(error),
        AxAutoParseError::Convert(error) => line_from_convert_error(error),
    }
}

fn line_from_ax_parse_error(error: &AxParseError) -> Option<usize> {
    match error {
        AxParseError::EmptyDocument => Some(1),
        AxParseError::TabsNotSupported { line }
        | AxParseError::InvalidIndentation { line }
        | AxParseError::InvalidPage { line }
        | AxParseError::UnexpectedIndentation { line }
        | AxParseError::InvalidDataBinding { line }
        | AxParseError::InvalidEach { line }
        | AxParseError::InvalidPipelineStage { line }
        | AxParseError::InvalidComponent { line }
        | AxParseError::InvalidTitle { line }
        | AxParseError::InvalidTheme { line }
        | AxParseError::InvalidHeadTag { line, .. }
        | AxParseError::InvalidExpression { line, .. } => Some(*line),
    }
}

fn line_from_ax_parse_v2_error(error: &AxParseV2Error) -> Option<usize> {
    match error {
        AxParseV2Error::EmptyDocument | AxParseV2Error::MissingPage => Some(1),
        AxParseV2Error::InvalidImport { line }
        | AxParseV2Error::MissingImportFrom { line }
        | AxParseV2Error::EmptyImportList { line }
        | AxParseV2Error::InvalidPage { line }
        | AxParseV2Error::InvalidType { line }
        | AxParseV2Error::InvalidLet { line }
        | AxParseV2Error::InvalidFunction { line }
        | AxParseV2Error::InvalidComponent { line }
        | AxParseV2Error::DuplicatePage { line }
        | AxParseV2Error::InvalidTag { line }
        | AxParseV2Error::UnterminatedTag { line }
        | AxParseV2Error::UnterminatedString { line }
        | AxParseV2Error::UnterminatedExpression { line }
        | AxParseV2Error::UnexpectedClosingTag { line, .. }
        | AxParseV2Error::MismatchedClosingTag { line, .. }
        | AxParseV2Error::MissingAttributeValue { line, .. } => Some(*line),
    }
}

fn line_from_convert_error(error: &AxConvertV2Error) -> Option<usize> {
    match error {
        AxConvertV2Error::InvalidExpression { error, .. } => line_from_ax_parse_error(error),
        AxConvertV2Error::MissingControlAttr { .. }
        | AxConvertV2Error::InvalidBindingAttr { .. }
        | AxConvertV2Error::ControlBranchAttrsNotSupported { .. }
        | AxConvertV2Error::DuplicateControlBranch { .. }
        | AxConvertV2Error::ControlBranchMustBeLast { .. }
        | AxConvertV2Error::UnexpectedControlBranch { .. }
        | AxConvertV2Error::InvalidHeadChild
        | AxConvertV2Error::UnsupportedHeadTag { .. }
        | AxConvertV2Error::HeadValueAttrsNotSupported { .. }
        | AxConvertV2Error::HeadValueRequiresSingleChild { .. }
        | AxConvertV2Error::HeadValueInvalidChild { .. }
        | AxConvertV2Error::HeadTagChildrenNotSupported { .. } => Some(1),
    }
}

fn line_from_semantic_error(error: &AxSemanticV2Error) -> Option<usize> {
    match error {
        AxSemanticV2Error::ReservedImportName { .. }
        | AxSemanticV2Error::ReservedComponentName { .. }
        | AxSemanticV2Error::DuplicateComponentName { .. }
        | AxSemanticV2Error::ComponentNameConflictsWithImport { .. }
        | AxSemanticV2Error::HeadTagOutsideHead { .. }
        | AxSemanticV2Error::HeadOutsideTopLevel => Some(1),
    }
}

fn line_from_backend_parse_error(error: &AxBackendParseError) -> Option<usize> {
    match error {
        AxBackendParseError::EmptyDocument => Some(1),
        AxBackendParseError::TabsNotSupported { line }
        | AxBackendParseError::InvalidIndentation { line }
        | AxBackendParseError::UnexpectedIndentation { line }
        | AxBackendParseError::InvalidBlock { line }
        | AxBackendParseError::InvalidDataBinding { line }
        | AxBackendParseError::InvalidInputSection { line }
        | AxBackendParseError::InvalidField { line }
        | AxBackendParseError::InvalidMutation { line }
        | AxBackendParseError::InvalidAssignment { line }
        | AxBackendParseError::InvalidReturn { line }
        | AxBackendParseError::InvalidSend { line }
        | AxBackendParseError::InvalidQuerySource { line }
        | AxBackendParseError::InvalidQueryClause { line }
        | AxBackendParseError::InvalidQueryNumber { line }
        | AxBackendParseError::InvalidExpression { line, .. } => Some(*line),
    }
}

fn print_check_text(diagnostics: &[CheckDiagnostic]) {
    if diagnostics.is_empty() {
        println!("Axonyx check passed.");
        return;
    }

    for diagnostic in diagnostics {
        println!("{}", format_check_diagnostic(diagnostic));
    }
}

fn format_check_diagnostic(diagnostic: &CheckDiagnostic) -> String {
    format!(
        "{}:{}:{}: {}[{}]: {}",
        diagnostic.file,
        diagnostic.line,
        diagnostic.column,
        diagnostic.severity,
        diagnostic.code,
        diagnostic.message
    )
}

fn print_cli_error(error: &anyhow::Error) {
    let message = error.to_string();

    eprintln!("Axonyx could not finish this command.");
    eprintln!();
    eprintln!("Problem:");
    eprintln!("  {}", translate_error_message(&message));

    if let Some(hint) = hint_for_error(error) {
        eprintln!();
        eprintln!("Hint:");
        eprintln!("  {hint}");
    }

    let mut chain = error.chain();
    let _ = chain.next();
    let details = chain.map(ToString::to_string).collect::<Vec<_>>();
    if !details.is_empty() {
        eprintln!();
        eprintln!("Details:");
        for detail in details {
            eprintln!("  - {}", translate_error_message(&detail));
        }
    }
}

fn translate_error_message(message: &str) -> String {
    message
        .replace("preview", "Axonyx")
        .replace("AxPreview", "Axonyx")
}

fn hint_for_error(error: &anyhow::Error) -> Option<&'static str> {
    let combined = error
        .chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    if combined.contains("Axonyx.toml was not found") {
        return Some("Run the command from an Axonyx app root, or create one with create-axonyx.");
    }

    if combined.contains("Axonyx diagnostics failed") {
        return Some(
            "Run `cargo ax check` to see the same file-level diagnostics before building.",
        );
    }

    if combined.contains("unable to resolve import") || combined.contains("failed to import") {
        return Some(
            "Check the import path, package_overrides, and whether the target .ax file exists.",
        );
    }

    if combined.contains("[prerender]")
        || combined.contains("prerender route")
        || combined.contains("missing prerender param")
    {
        return Some(
            "Check the [prerender] routes in Axonyx.toml. Dynamic params must be strings.",
        );
    }

    if combined.contains("--clean refuses") {
        return Some("Choose an output directory inside the app root, for example `cargo ax build --out-dir dist --clean`.");
    }

    if combined.contains("failed to render route") {
        return Some("Run `cargo ax check`, then inspect the route's page.ax, layout.ax, loader.ax, and imports.");
    }

    None
}

fn display_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized
        .strip_prefix("//?/")
        .unwrap_or(&normalized)
        .to_string()
}

fn app_root() -> Result<PathBuf> {
    let root = std::env::current_dir().context("unable to resolve current directory")?;
    let axonyx_toml = root.join("Axonyx.toml");

    if !axonyx_toml.exists() {
        bail!(
            "Axonyx.toml was not found in '{}'; run this command from an Axonyx app root",
            root.display()
        );
    }

    Ok(root)
}

fn add_module(module: ModuleKind) -> Result<()> {
    let root = app_root()?;
    let axonyx_toml = root.join("Axonyx.toml");

    match module {
        ModuleKind::Docs => {
            scaffold_docs_module(&root)?;
            enable_module(&axonyx_toml, "docs")?;
            println!("Added docs module.");
        }
        ModuleKind::Ui => {
            add_ui_module(&root, &axonyx_toml)?;
            println!("Added ui module.");
        }
    }

    Ok(())
}

fn run_dev_server(args: DevArgs) -> Result<()> {
    run_http_server(args, ServerMode::Dev)
}

fn run_start_server(args: DevArgs) -> Result<()> {
    run_http_server(args, ServerMode::Start)
}

fn run_http_server(args: DevArgs, mode: ServerMode) -> Result<()> {
    let root = app_root()?;
    let backend_status = compile_backend_from_app_root(&root)?;

    let bind = format!("{}:{}", args.host, args.port);
    let listener = TcpListener::bind(&bind)
        .with_context(|| format!("failed to bind Axonyx server at {bind}"))?;
    let preview_store = preview_store_from_content(&root)?;
    let shared_state = Arc::new(DevServerState {
        root,
        preview_store: Mutex::new(preview_store),
    });

    print_backend_build_status(&backend_status);
    println!("Axonyx {} server listening at http://{bind}", mode.label());
    println!(
        "Routes come from app/**/page.ax with nested layouts, route-local loader.ax, actions.ax POST handling, and routes/**/*.ax API endpoints."
    );
    if mode == ServerMode::Dev {
        println!("Live reload polling is enabled.");
    }
    println!("Press Ctrl+C to stop.");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_connection(stream, &shared_state, mode) {
                    eprintln!("Axonyx {} server error: {error:#}", mode.label());
                }
            }
            Err(error) => eprintln!("Axonyx {} server connection error: {error}", mode.label()),
        }
    }

    Ok(())
}

fn compile_backend_from_app_root(root: &Path) -> Result<BackendBuildStatus> {
    let mut sources = Vec::new();
    collect_backend_sources(root, &mut sources)?;

    let output_path = root.join("src").join("generated").join("backend.rs");

    if sources.is_empty() {
        return Ok(BackendBuildStatus::NoSources { output_path });
    }

    let source_refs = sources
        .iter()
        .map(|(name, source)| (name.as_str(), source.as_str()))
        .collect::<Vec<_>>();

    let module = compile_backend_sources_to_module(&source_refs)
        .with_context(|| "failed to compile backend .ax sources")?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create generated backend directory '{}'",
                parent.display()
            )
        })?;
    }

    fs::write(&output_path, module).with_context(|| {
        format!(
            "failed to write generated backend module '{}'",
            output_path.display()
        )
    })?;

    Ok(BackendBuildStatus::Generated {
        source_count: source_refs.len(),
        output_path,
    })
}

fn print_backend_build_status(status: &BackendBuildStatus) {
    match status {
        BackendBuildStatus::Generated {
            source_count,
            output_path,
        } => {
            println!(
                "Generated backend from {source_count} .ax source(s) into {}",
                output_path.display()
            );
        }
        BackendBuildStatus::NoSources { output_path } => {
            println!(
                "No backend .ax sources found; leaving generated backend at {}",
                output_path.display()
            );
        }
    }
}

fn build_static_site_from_app_root(
    root: &Path,
    out_dir: &Path,
    clean: bool,
) -> Result<StaticBuildStatus> {
    let output_dir = resolve_output_dir(root, out_dir);
    let routes = collect_page_route_manifest(root)?;
    let static_routes = routes
        .iter()
        .filter(|route| route.params.is_empty())
        .collect::<Vec<_>>();
    let dynamic_routes = routes
        .iter()
        .filter(|route| !route.params.is_empty())
        .collect::<Vec<_>>();
    let prerender_routes = load_prerender_routes(root)?;

    if clean {
        clean_output_dir(root, &output_dir)?;
    }

    fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create '{}'", output_dir.display()))?;

    copy_public_assets_to_dist(root, &output_dir)?;
    copy_package_assets_to_dist(root, &output_dir)?;
    let content_collection_count = write_content_manifest_to_dist(root, &output_dir)?;

    if static_routes.is_empty() && prerender_routes.is_empty() {
        return Ok(StaticBuildStatus::NoPages {
            skipped_dynamic_count: dynamic_routes.len(),
            content_collection_count,
            output_dir,
        });
    }

    let state = DevServerState {
        root: root.to_path_buf(),
        preview_store: Mutex::new(preview_store_from_content(root)?),
    };

    for route in &static_routes {
        let resolved = resolve_route(root, &route.route)?
            .ok_or_else(|| anyhow::anyhow!("failed to resolve route '{}'", route.route))?;
        let html = render_route_html(&state, &resolved)?;
        let output_path = static_route_output_path(&output_dir, &route.route)?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create '{}'", parent.display()))?;
        }

        fs::write(&output_path, html)
            .with_context(|| format!("failed to write '{}'", output_path.display()))?;
    }

    let prerendered_count = build_prerendered_routes(
        root,
        &output_dir,
        &state,
        &dynamic_routes,
        &prerender_routes,
    )?;
    let skipped_dynamic_count = dynamic_routes.len().saturating_sub(
        prerender_routes
            .iter()
            .filter(|entry| {
                dynamic_routes
                    .iter()
                    .any(|route| route.route == entry.route)
            })
            .count(),
    );

    Ok(StaticBuildStatus::Generated {
        route_count: static_routes.len(),
        prerendered_count,
        skipped_dynamic_count,
        content_collection_count,
        output_dir,
    })
}

fn clean_output_dir(root: &Path, output_dir: &Path) -> Result<()> {
    if !output_dir.exists() {
        return Ok(());
    }

    let root = root
        .canonicalize()
        .with_context(|| format!("failed to resolve app root '{}'", root.display()))?;
    let output_dir = output_dir
        .canonicalize()
        .with_context(|| format!("failed to resolve output dir '{}'", output_dir.display()))?;

    if output_dir == root || !output_dir.starts_with(&root) {
        bail!(
            "--clean refuses to remove '{}' because it is not a child of app root '{}'",
            output_dir.display(),
            root.display()
        );
    }

    fs::remove_dir_all(&output_dir)
        .with_context(|| format!("failed to clean '{}'", output_dir.display()))?;
    Ok(())
}

fn build_prerendered_routes(
    root: &Path,
    output_dir: &Path,
    state: &DevServerState,
    dynamic_routes: &[&RouteManifestItem],
    prerender_routes: &[PrerenderRoute],
) -> Result<usize> {
    let mut count = 0;

    for entry in prerender_routes {
        let Some(route) = dynamic_routes
            .iter()
            .find(|route| route.route == entry.route)
        else {
            bail!(
                "prerender route '{}' does not match a dynamic page route",
                entry.route
            );
        };

        for params in &entry.params {
            let concrete_route = concrete_route_from_params(&route.route, params)?;
            let resolved = resolve_route(root, &concrete_route)?.ok_or_else(|| {
                anyhow::anyhow!("failed to resolve prerender route '{}'", concrete_route)
            })?;
            let html = render_route_html(state, &resolved)?;
            let output_path = static_route_output_path(output_dir, &concrete_route)?;

            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create '{}'", parent.display()))?;
            }

            fs::write(&output_path, html)
                .with_context(|| format!("failed to write '{}'", output_path.display()))?;
            count += 1;
        }
    }

    Ok(count)
}

fn concrete_route_from_params(
    route: &str,
    params: &std::collections::BTreeMap<String, String>,
) -> Result<String> {
    let mut segments = Vec::new();

    for segment in route.trim_start_matches('/').split('/') {
        if let Some(name) = segment.strip_prefix(':').filter(|name| !name.is_empty()) {
            let value = params
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("missing prerender param '{name}' for '{route}'"))?;
            let value = value.trim_matches('/');
            if value.is_empty() || value.contains('/') || value == "." || value == ".." {
                bail!("invalid prerender param '{name}' value '{value}' for '{route}'");
            }
            segments.push(value.to_string());
        } else if !segment.is_empty() {
            segments.push(segment.to_string());
        }
    }

    if segments.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", segments.join("/")))
    }
}

fn load_prerender_routes(root: &Path) -> Result<Vec<PrerenderRoute>> {
    let Some(routes_value) = axonyx_config_value(root, "prerender", "routes") else {
        return Ok(Vec::new());
    };

    let routes = routes_value
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("[prerender].routes must be an array"))?;
    let mut out = Vec::new();

    for route_value in routes {
        let table = route_value
            .as_table()
            .ok_or_else(|| anyhow::anyhow!("each [prerender].routes item must be a table"))?;
        let route = table
            .get("route")
            .and_then(toml::Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("prerender route item is missing route"))?
            .trim()
            .to_string();
        let params_value = table
            .get("params")
            .ok_or_else(|| anyhow::anyhow!("prerender route '{route}' is missing params"))?;
        let params_array = params_value
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("prerender route '{route}' params must be an array"))?;
        let mut params = Vec::new();

        for params_value in params_array {
            let params_table = params_value.as_table().ok_or_else(|| {
                anyhow::anyhow!("prerender route '{route}' params entries must be tables")
            })?;
            let mut params_map = std::collections::BTreeMap::new();

            for (name, value) in params_table {
                let value = value.as_str().ok_or_else(|| {
                    anyhow::anyhow!("prerender route '{route}' param '{name}' must be a string")
                })?;
                params_map.insert(name.to_string(), value.to_string());
            }

            params.push(params_map);
        }

        out.push(PrerenderRoute { route, params });
    }

    Ok(out)
}

fn resolve_output_dir(root: &Path, out_dir: &Path) -> PathBuf {
    if out_dir.is_absolute() {
        out_dir.to_path_buf()
    } else {
        root.join(out_dir)
    }
}

fn copy_public_assets_to_dist(root: &Path, output_dir: &Path) -> Result<()> {
    let public_dir = root.join("public");
    if !public_dir.exists() {
        return Ok(());
    }

    copy_dir_all_filtered(&public_dir, output_dir, |_| false)
}

fn copy_package_assets_to_dist(root: &Path, output_dir: &Path) -> Result<()> {
    let Some(package_root) = resolve_package_asset_root(root, "axonyx-ui") else {
        return Ok(());
    };

    let css_root = package_css_root(&package_root);
    if !css_root.exists() {
        return Ok(());
    }

    let target = output_dir.join("_ax").join("pkg").join("axonyx-ui");
    copy_dir_all_filtered(&css_root, &target, |_| false)
}

fn write_content_manifest_to_dist(root: &Path, output_dir: &Path) -> Result<usize> {
    let manifest = collect_content_manifest(root)?;
    let count = manifest.collections.len();
    if count == 0 {
        return Ok(0);
    }

    let target = output_dir.join("_ax").join("content").join("manifest.json");
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create '{}'", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(&manifest)
        .context("failed to render content manifest as JSON")?;
    fs::write(&target, json)
        .with_context(|| format!("failed to write content manifest to '{}'", target.display()))?;

    Ok(count)
}

fn static_route_output_path(output_dir: &Path, route: &str) -> Result<PathBuf> {
    let normalized = normalize_request_path(route)?;
    let segments = path_segments(&normalized);

    if segments.is_empty() {
        return Ok(output_dir.join("index.html"));
    }

    Ok(segments
        .iter()
        .fold(output_dir.to_path_buf(), |current, segment| {
            current.join(segment)
        })
        .join("index.html"))
}

fn print_static_build_status(status: &StaticBuildStatus) {
    match status {
        StaticBuildStatus::Generated {
            route_count,
            prerendered_count,
            skipped_dynamic_count,
            content_collection_count,
            output_dir,
        } => {
            println!(
                "Generated static site from {route_count} page route(s) into {}",
                output_dir.display()
            );
            if *prerendered_count > 0 {
                println!("Prerendered {prerendered_count} dynamic page variant(s).");
            }
            if *skipped_dynamic_count > 0 {
                println!(
                    "Skipped {skipped_dynamic_count} dynamic page route(s); provide params through a future prerender config."
                );
            }
            if *content_collection_count > 0 {
                println!(
                    "Wrote content manifest for {content_collection_count} collection(s) into {}/_ax/content/manifest.json",
                    output_dir.display()
                );
            }
        }
        StaticBuildStatus::NoPages {
            skipped_dynamic_count,
            content_collection_count,
            output_dir,
        } => {
            println!(
                "No static page routes found; copied public assets into {}",
                output_dir.display()
            );
            if *skipped_dynamic_count > 0 {
                println!(
                    "Skipped {skipped_dynamic_count} dynamic page route(s); provide params through a future prerender config."
                );
            }
            if *content_collection_count > 0 {
                println!(
                    "Wrote content manifest for {content_collection_count} collection(s) into {}/_ax/content/manifest.json",
                    output_dir.display()
                );
            }
        }
    }
}

fn collect_app_route_manifest(root: &Path) -> Result<Vec<RouteManifestItem>> {
    let mut routes = collect_page_route_manifest(root)?;
    routes.extend(collect_backend_route_manifest(root)?);
    routes.sort_by(|left, right| {
        left.route
            .cmp(&right.route)
            .then_with(|| left.kind.cmp(right.kind))
            .then_with(|| left.method.cmp(&right.method))
    });
    Ok(routes)
}

fn collect_page_route_manifest(root: &Path) -> Result<Vec<RouteManifestItem>> {
    let app_root = root.join("app");
    if !app_root.exists() {
        return Ok(Vec::new());
    }

    let mut routes = Vec::new();
    collect_app_route_manifest_from(root, &app_root, &app_root, &mut routes)?;
    Ok(routes)
}

fn collect_app_route_manifest_from(
    root: &Path,
    app_root: &Path,
    dir: &Path,
    out: &mut Vec<RouteManifestItem>,
) -> Result<()> {
    let page_path = dir.join("page.ax");
    if page_path.exists() {
        out.push(app_route_manifest_item(root, app_root, dir, &page_path)?);
    }

    for entry in fs::read_dir(dir).with_context(|| format!("failed to read '{}'", dir.display()))? {
        let entry =
            entry.with_context(|| format!("failed to read entry in '{}'", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect '{}'", path.display()))?;

        if file_type.is_dir() {
            collect_app_route_manifest_from(root, app_root, &path, out)?;
        }
    }

    Ok(())
}

fn app_route_manifest_item(
    root: &Path,
    app_root: &Path,
    page_dir: &Path,
    page_path: &Path,
) -> Result<RouteManifestItem> {
    let relative_dir = page_dir.strip_prefix(app_root).unwrap_or(page_dir);
    let segments = relative_dir
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let route = route_pattern_from_segments(&segments);
    let params = segments
        .iter()
        .filter_map(|segment| parse_dynamic_app_segment(segment).map(str::to_string))
        .collect::<Vec<_>>();

    let mut layouts = Vec::new();
    let root_layout = app_root.join("layout.ax");
    if root_layout.exists() {
        layouts.push(display_relative_path(root, &root_layout));
    }

    let mut current = app_root.to_path_buf();
    for segment in &segments {
        current = current.join(segment);
        let layout_path = current.join("layout.ax");
        if layout_path.exists() {
            layouts.push(display_relative_path(root, &layout_path));
        }
    }

    let loader_path = page_dir.join("loader.ax");
    let actions_path = page_dir.join("actions.ax");

    Ok(RouteManifestItem {
        kind: "page",
        route,
        method: None,
        file: display_relative_path(root, page_path),
        layouts,
        loader: loader_path
            .exists()
            .then(|| display_relative_path(root, &loader_path)),
        actions: actions_path
            .exists()
            .then(|| display_relative_path(root, &actions_path)),
        params,
    })
}

fn collect_backend_route_manifest(root: &Path) -> Result<Vec<RouteManifestItem>> {
    let routes_root = root.join("routes");
    if !routes_root.exists() {
        return Ok(Vec::new());
    }

    let mut sources = Vec::new();
    collect_backend_sources_in_dir(&routes_root, &routes_root, &mut sources, true)?;

    let mut routes = Vec::new();
    for (relative_path, source) in sources {
        let document = parse_backend_ax(&source).with_context(|| {
            format!(
                "failed to parse backend route source '{}'",
                routes_root.join(&relative_path).display()
            )
        })?;

        for block in document.blocks {
            let AxBackendBlock::Route(route) = block else {
                continue;
            };

            routes.push(RouteManifestItem {
                kind: "api",
                route: route.path.clone(),
                method: Some(route.method),
                file: format!("routes/{relative_path}"),
                layouts: Vec::new(),
                loader: None,
                actions: None,
                params: route_params_from_pattern(&route.path),
            });
        }
    }

    Ok(routes)
}

fn route_pattern_from_segments(segments: &[&str]) -> String {
    if segments.is_empty() {
        return "/".to_string();
    }

    let route = segments
        .iter()
        .map(|segment| {
            parse_dynamic_app_segment(segment)
                .map(|param| format!(":{param}"))
                .unwrap_or_else(|| (*segment).to_string())
        })
        .collect::<Vec<_>>()
        .join("/");
    format!("/{route}")
}

fn route_params_from_pattern(pattern: &str) -> Vec<String> {
    pattern
        .split('/')
        .filter_map(|segment| {
            segment
                .strip_prefix(':')
                .filter(|name| !name.is_empty())
                .or_else(|| parse_dynamic_app_segment(segment))
                .map(str::to_string)
        })
        .collect()
}

fn display_relative_path(root: &Path, path: &Path) -> String {
    display_path(path.strip_prefix(root).unwrap_or(path))
}

fn print_routes_text(routes: &[RouteManifestItem]) {
    if routes.is_empty() {
        println!("No routes found in app/**/page.ax or routes/**/*.ax.");
        return;
    }

    println!("Routes:");
    for route in routes {
        let mut details = vec![format!("kind={}", route.kind)];
        if let Some(method) = &route.method {
            details.push(format!("method={method}"));
        }
        details.push(format!("file={}", route.file));
        if !route.layouts.is_empty() {
            details.push(format!("layouts={}", route.layouts.len()));
        }
        if route.loader.is_some() {
            details.push("loader".to_string());
        }
        if route.actions.is_some() {
            details.push("actions".to_string());
        }
        if !route.params.is_empty() {
            details.push(format!("params={}", route.params.join(",")));
        }

        println!("  {:<28} {}", route.route, details.join(" "));
    }
}

fn collect_backend_sources(root: &Path, out: &mut Vec<(String, String)>) -> Result<()> {
    let routes_root = root.join("routes");
    let jobs_root = root.join("jobs");
    let app_root = root.join("app");

    collect_backend_sources_in_dir(&routes_root, &routes_root, out, true)?;
    collect_backend_sources_in_dir(&jobs_root, &jobs_root, out, true)?;
    collect_named_backend_files(&app_root, &app_root, out, &["loader.ax", "actions.ax"])?;
    Ok(())
}

fn collect_backend_sources_in_dir(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, String)>,
    recurse: bool,
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)
        .with_context(|| format!("failed to read directory '{}'", dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in '{}'", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect '{}'", path.display()))?;

        if file_type.is_dir() && recurse {
            collect_backend_sources_in_dir(root, &path, out, true)?;
            continue;
        }

        if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("ax") {
            let source = fs::read_to_string(&path)
                .with_context(|| format!("failed to read backend source '{}'", path.display()))?;
            let name = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((name, source));
        }
    }

    Ok(())
}

fn collect_named_backend_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, String)>,
    names: &[&str],
) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)
        .with_context(|| format!("failed to read directory '{}'", dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in '{}'", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect '{}'", path.display()))?;

        if file_type.is_dir() {
            collect_named_backend_files(root, &path, out, names)?;
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if !names.contains(&file_name) {
            continue;
        }

        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read backend source '{}'", path.display()))?;
        let name = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        out.push((name, source));
    }

    Ok(())
}

fn scaffold_docs_module(root: &Path) -> Result<()> {
    write_if_missing(root, "app/docs/layout.ax", DOCS_LAYOUT_AX)?;
    write_if_missing(root, "app/docs/page.ax", DOCS_HOME_AX)?;
    write_if_missing(
        root,
        "app/docs/getting-started/page.ax",
        DOCS_GETTING_STARTED_AX,
    )?;
    write_if_missing(root, "app/docs/reference/page.ax", DOCS_REFERENCE_AX)?;
    write_if_missing(root, "app/docs/examples/page.ax", DOCS_EXAMPLES_AX)?;
    Ok(())
}

fn add_ui_module(root: &Path, axonyx_toml: &Path) -> Result<()> {
    ensure_ui_cargo_dependency(root)?;
    ensure_ui_layout_setup(root)?;
    enable_module(&axonyx_toml.to_path_buf(), "ui")?;

    println!("Ensured Cargo dependency: axonyx-ui = \"{AXONYX_UI_VERSION}\".");
    println!("Updated app/layout.ax with silver theme and stylesheet link when needed.");
    println!("You can now import components such as:");
    println!("  import {{ SectionCard }} from \"@axonyx/ui/foundry/SectionCard.ax\"");
    Ok(())
}

fn ensure_ui_cargo_dependency(root: &Path) -> Result<()> {
    let cargo_toml = root.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Ok(());
    }

    ensure_cargo_dependency_version(&cargo_toml, "axonyx-ui", AXONYX_UI_VERSION)
}

fn ensure_cargo_dependency_version(
    cargo_toml: &Path,
    dependency_name: &str,
    dependency_version: &str,
) -> Result<()> {
    let source = fs::read_to_string(cargo_toml)
        .with_context(|| format!("failed to read '{}'", cargo_toml.display()))?;
    let mut value = source
        .parse::<toml::Value>()
        .with_context(|| format!("failed to parse '{}'", cargo_toml.display()))?;
    let root_table = value
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("Cargo.toml root must be a TOML table"))?;
    let dependencies = root_table
        .entry("dependencies")
        .or_insert_with(|| toml::Value::Table(Default::default()));
    let dependencies_table = dependencies
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("[dependencies] must be a TOML table"))?;

    if dependencies_table.contains_key(dependency_name) {
        return Ok(());
    }

    dependencies_table.insert(
        dependency_name.to_string(),
        toml::Value::String(dependency_version.to_string()),
    );

    let rendered = toml::to_string_pretty(&value).context("failed to render Cargo.toml")?;
    fs::write(cargo_toml, rendered)
        .with_context(|| format!("failed to write '{}'", cargo_toml.display()))?;
    Ok(())
}

fn copy_dir_all_filtered(
    source: &Path,
    destination: &Path,
    skip: impl Fn(&Path) -> bool + Copy,
) -> Result<()> {
    if skip(source) {
        return Ok(());
    }

    if source.is_dir() {
        fs::create_dir_all(destination)
            .with_context(|| format!("failed to create '{}'", destination.display()))?;

        for entry in fs::read_dir(source)
            .with_context(|| format!("failed to read '{}'", source.display()))?
        {
            let entry =
                entry.with_context(|| format!("failed to read entry in '{}'", source.display()))?;
            let path = entry.path();
            let target = destination.join(entry.file_name());
            copy_dir_all_filtered(&path, &target, skip)?;
        }
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create '{}'", parent.display()))?;
    }

    fs::copy(source, destination).with_context(|| {
        format!(
            "failed to copy '{}' to '{}'",
            source.display(),
            destination.display()
        )
    })?;

    Ok(())
}

fn ensure_ui_layout_setup(root: &Path) -> Result<()> {
    let layout_path = root.join("app").join("layout.ax");
    if !layout_path.exists() {
        return Ok(());
    }

    let source = fs::read_to_string(&layout_path)
        .with_context(|| format!("failed to read '{}'", layout_path.display()))?;
    let updated = if source.contains("<Head>") {
        ensure_ui_layout_setup_jsx(&source)
    } else {
        ensure_ui_layout_setup_v1(&source)
    };

    if updated != source {
        fs::write(&layout_path, updated)
            .with_context(|| format!("failed to write '{}'", layout_path.display()))?;
    }

    Ok(())
}

fn ensure_ui_layout_setup_jsx(source: &str) -> String {
    const THEME_TAG: &str = "<Theme>silver</Theme>";
    const STYLESHEET_HREF: &str = "/_ax/pkg/axonyx-ui/index.css";
    const LEGACY_STYLESHEET_HREF: &str = "/css/axonyx-ui/index.css";
    const STYLESHEET_TAG: &str = r#"<Link rel="stylesheet" href="/_ax/pkg/axonyx-ui/index.css" />"#;

    let mut updated = source.to_string();

    if updated.contains("<Head>") {
        if !updated.contains(THEME_TAG) {
            updated = updated.replacen("<Head>", &format!("<Head>\n  {THEME_TAG}"), 1);
        }

        if !updated.contains(STYLESHEET_HREF) && !updated.contains(LEGACY_STYLESHEET_HREF) {
            updated = updated.replacen("</Head>", &format!("  {STYLESHEET_TAG}\n</Head>"), 1);
        }

        return updated;
    }

    let mut lines = source.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let page_index = lines
        .iter()
        .position(|line| line.trim_start().starts_with("page "))
        .unwrap_or(0);

    let mut head_block = vec![
        String::new(),
        "<Head>".to_string(),
        format!("  {THEME_TAG}"),
        format!("  {STYLESHEET_TAG}"),
        "</Head>".to_string(),
    ];

    lines.splice(page_index + 1..page_index + 1, head_block.drain(..));
    lines.join("\n")
}

fn ensure_ui_layout_setup_v1(source: &str) -> String {
    let mut lines = source.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    let Some(page_index) = lines
        .iter()
        .position(|line| line.trim_start().starts_with("page "))
    else {
        return source.to_string();
    };

    let has_theme = lines
        .iter()
        .any(|line| line.trim() == "theme \"silver\"" || line.trim_start().starts_with("theme "));
    let has_stylesheet = lines.iter().any(|line| {
        line.contains("/_ax/pkg/axonyx-ui/index.css") || line.contains("/css/axonyx-ui/index.css")
    });

    if has_theme && has_stylesheet {
        return source.to_string();
    }

    let mut insert_at = page_index + 1;
    while insert_at < lines.len() {
        let trimmed = lines[insert_at].trim_start();
        if lines[insert_at].starts_with("  ")
            && matches!(
                trimmed.split_whitespace().next(),
                Some("title" | "meta" | "link" | "script" | "theme")
            )
        {
            insert_at += 1;
            continue;
        }
        break;
    }

    let mut to_insert = Vec::new();
    if !has_theme {
        to_insert.push("  theme \"silver\"".to_string());
    }
    if !has_stylesheet {
        to_insert
            .push("  link rel: \"stylesheet\", href: \"/_ax/pkg/axonyx-ui/index.css\"".to_string());
    }

    lines.splice(insert_at..insert_at, to_insert);
    lines.join("\n")
}

fn write_if_missing(root: &Path, relative: &str, contents: &str) -> Result<()> {
    let path = root.join(relative);
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory '{}'", parent.display()))?;
    }

    fs::write(&path, contents)
        .with_context(|| format!("failed to write module file '{}'", path.display()))?;
    Ok(())
}

fn enable_module(axonyx_toml: &PathBuf, module_name: &str) -> Result<()> {
    update_axonyx_toml(axonyx_toml, |root_table| {
        let modules = root_table
            .entry("modules")
            .or_insert_with(|| toml::Value::Table(Default::default()));

        let modules_table = modules
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("[modules] must be a TOML table"))?;

        let enabled = modules_table
            .entry("enabled")
            .or_insert_with(|| toml::Value::Array(Vec::new()));

        let enabled_array = enabled
            .as_array_mut()
            .ok_or_else(|| anyhow::anyhow!("[modules].enabled must be an array"))?;

        if !enabled_array
            .iter()
            .any(|item| item.as_str() == Some(module_name))
        {
            enabled_array.push(toml::Value::String(module_name.to_string()));
        }

        Ok(())
    })
}

fn update_axonyx_toml(
    axonyx_toml: &PathBuf,
    update: impl FnOnce(&mut toml::map::Map<String, toml::Value>) -> Result<()>,
) -> Result<()> {
    let source = fs::read_to_string(axonyx_toml)
        .with_context(|| format!("failed to read '{}'", axonyx_toml.display()))?;
    let mut value = source
        .parse::<toml::Value>()
        .with_context(|| format!("failed to parse '{}'", axonyx_toml.display()))?;

    let root_table = value
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("Axonyx.toml root must be a TOML table"))?;
    update(root_table)?;

    let rendered = toml::to_string_pretty(&value).context("failed to render Axonyx.toml")?;
    fs::write(axonyx_toml, rendered)
        .with_context(|| format!("failed to write '{}'", axonyx_toml.display()))?;
    Ok(())
}

fn handle_connection(
    mut stream: TcpStream,
    state: &DevServerState,
    mode: ServerMode,
) -> Result<()> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .context("failed to set read timeout")?;

    let Some(request) = read_http_request(&mut stream)? else {
        return Ok(());
    };

    if request.method == "GET" {
        if let Some(asset) = load_package_asset(&state.root, &request.target)? {
            write_response(&mut stream, "200 OK", asset.content_type, &asset.body)?;
            return Ok(());
        }

        if let Some(asset) = load_public_asset(&state.root, &request.target)? {
            write_response(&mut stream, "200 OK", asset.content_type, &asset.body)?;
            return Ok(());
        }
    }

    if request.method == "POST" && request.target.starts_with("/__axonyx/action") {
        handle_action_request(&mut stream, state, &request)?;
        return Ok(());
    }

    if request.method == "GET" && request.target == "/favicon.ico" {
        write_response(
            &mut stream,
            "204 No Content",
            "text/plain; charset=utf-8",
            b"",
        )?;
        return Ok(());
    }

    if mode == ServerMode::Dev
        && request.method == "GET"
        && request.target.starts_with("/__axonyx/version")
    {
        let request_path = extract_version_path(&request.target).unwrap_or_else(|| "/".to_string());
        let Some(route) = resolve_route(&state.root, &request_path)? else {
            write_response(
                &mut stream,
                "404 Not Found",
                "text/plain; charset=utf-8",
                b"route not found",
            )?;
            return Ok(());
        };

        let version = route_version(&state.root, &route)?;
        write_response(
            &mut stream,
            "200 OK",
            "text/plain; charset=utf-8",
            version.as_bytes(),
        )?;
        return Ok(());
    }

    if let Some(response) = execute_backend_route_request(state, &request)? {
        let status = http_status_text(response.status);
        write_response(&mut stream, &status, &response.content_type, &response.body)?;
        return Ok(());
    }

    if request.method != "GET" {
        write_response(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            b"Method Not Allowed",
        )?;
        return Ok(());
    }

    let Some(route) = resolve_route(&state.root, &request.target)? else {
        if looks_like_asset_request(&request.target) {
            write_response(
                &mut stream,
                "404 Not Found",
                "text/plain; charset=utf-8",
                b"asset not found",
            )?;
            return Ok(());
        }

        let html = format!(
            "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Axonyx 404</title></head><body><h1>Route not found</h1><p>No <code>page.ax</code> matched <code>{}</code>.</p></body></html>",
            html_escape(&request.target)
        );
        write_response(
            &mut stream,
            "404 Not Found",
            "text/html; charset=utf-8",
            html.as_bytes(),
        )?;
        return Ok(());
    };

    let mut html = render_route_html(state, &route)?;
    if mode.inject_dev_client() {
        html = inject_dev_client(&html, &route.request_path);
    }
    write_response(
        &mut stream,
        "200 OK",
        "text/html; charset=utf-8",
        html.as_bytes(),
    )?;
    Ok(())
}

fn execute_backend_route_request(
    state: &DevServerState,
    request: &HttpRequest,
) -> Result<Option<AxPreviewHttpResponse>> {
    let mut sources = Vec::new();
    let routes_root = state.root.join("routes");
    collect_backend_sources_in_dir(&routes_root, &routes_root, &mut sources, true)?;

    if sources.is_empty() {
        return Ok(None);
    }

    let source_refs = sources
        .iter()
        .map(|(_, source)| source.as_str())
        .collect::<Vec<_>>();
    let mut store = state
        .preview_store
        .lock()
        .map_err(|_| anyhow::anyhow!("preview store lock was poisoned"))?;

    execute_preview_route_sources(&source_refs, &request.method, &request.target, &mut store)
        .with_context(|| {
            format!(
                "failed to execute backend route {} {}",
                request.method, request.target
            )
        })
}

fn read_http_request(stream: &mut TcpStream) -> Result<Option<HttpRequest>> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let mut header_end = None;

    loop {
        let read = stream
            .read(&mut chunk)
            .context("failed to read request from dev client")?;
        if read == 0 {
            if buffer.is_empty() {
                return Ok(None);
            }
            break;
        }

        buffer.extend_from_slice(&chunk[..read]);

        if header_end.is_none() {
            header_end = find_header_end(&buffer);
        }

        if let Some(end) = header_end {
            let header_text = String::from_utf8_lossy(&buffer[..end]);
            let content_length = parse_content_length(&header_text);
            let total = end + 4 + content_length;
            if buffer.len() >= total {
                break;
            }
        }
    }

    let Some(header_end) = find_header_end(&buffer) else {
        return Ok(None);
    };

    let header_text = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = header_text.lines();
    let Some(request_line) = lines.next() else {
        return Ok(None);
    };

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let target = parts.next().unwrap_or("/").to_string();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect::<std::collections::BTreeMap<_, _>>();
    let body_start = header_end + 4;
    let body = buffer[body_start..].to_vec();

    Ok(Some(HttpRequest {
        method,
        target,
        headers,
        body,
    }))
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.trim().eq_ignore_ascii_case("content-length") {
                return value.trim().parse::<usize>().ok();
            }
            None
        })
        .unwrap_or(0)
}

fn handle_action_request(
    stream: &mut TcpStream,
    state: &DevServerState,
    request: &HttpRequest,
) -> Result<()> {
    let content_type = request
        .headers
        .get("content-type")
        .map(String::as_str)
        .unwrap_or("");
    if !content_type.starts_with("application/x-www-form-urlencoded") {
        write_response(
            stream,
            "415 Unsupported Media Type",
            "text/plain; charset=utf-8",
            b"expected application/x-www-form-urlencoded",
        )?;
        return Ok(());
    }

    let request_path =
        extract_action_query_param(&request.target, "path").unwrap_or_else(|| "/".to_string());
    let action_name = extract_action_query_param(&request.target, "name").unwrap_or_default();
    if action_name.is_empty() {
        write_response(
            stream,
            "400 Bad Request",
            "text/plain; charset=utf-8",
            b"missing action name",
        )?;
        return Ok(());
    }

    let Some(route) = resolve_route(&state.root, &request_path)? else {
        write_response(
            stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"route not found",
        )?;
        return Ok(());
    };

    let Some(actions_path) = &route.actions_path else {
        write_response(
            stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"actions.ax not found for route",
        )?;
        return Ok(());
    };

    let action_source = fs::read_to_string(actions_path)
        .with_context(|| format!("failed to read '{}'", actions_path.display()))?;
    let input_fields = parse_form_body(&request.body);
    let mut store = state
        .preview_store
        .lock()
        .map_err(|_| anyhow::anyhow!("preview store lock was poisoned"))?;
    let result = execute_preview_action_sources(
        &[action_source.as_str()],
        &action_name,
        &input_fields,
        &mut store,
    )
    .with_context(|| {
        format!(
            "failed to execute action '{}' from '{}'",
            action_name,
            actions_path.display()
        )
    })?;

    let redirect_to = result.redirect_to.unwrap_or(route.request_path);
    write_redirect_response(stream, "303 See Other", &redirect_to)?;
    Ok(())
}

fn parse_form_body(body: &[u8]) -> std::collections::BTreeMap<String, String> {
    let text = String::from_utf8_lossy(body);
    let mut fields = std::collections::BTreeMap::new();

    for pair in text.split('&') {
        if pair.is_empty() {
            continue;
        }

        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        fields.insert(url_decode(key), url_decode(value));
    }

    fields
}

fn http_status_text(status: u16) -> String {
    match status {
        200 => "200 OK".to_string(),
        204 => "204 No Content".to_string(),
        400 => "400 Bad Request".to_string(),
        404 => "404 Not Found".to_string(),
        405 => "405 Method Not Allowed".to_string(),
        415 => "415 Unsupported Media Type".to_string(),
        500 => "500 Internal Server Error".to_string(),
        _ => format!("{status} OK"),
    }
}

fn write_redirect_response(stream: &mut TcpStream, status: &str, location: &str) -> Result<()> {
    let header = format!(
        "HTTP/1.1 {status}\r\nLocation: {location}\r\nContent-Length: 0\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
    );

    stream
        .write_all(header.as_bytes())
        .context("failed to write redirect response")?;
    stream
        .flush()
        .context("failed to flush redirect response")?;
    Ok(())
}

fn load_public_asset(root: &Path, request_path: &str) -> Result<Option<StaticAsset>> {
    let normalized = normalize_request_path(request_path)?;
    let segments = path_segments(&normalized);
    if segments.is_empty() {
        return Ok(None);
    }

    let asset_path = segments
        .iter()
        .fold(root.join("public"), |current, segment| {
            current.join(segment)
        });

    if !asset_path.exists() || !asset_path.is_file() {
        return Ok(None);
    }

    let body = fs::read(&asset_path)
        .with_context(|| format!("failed to read asset '{}'", asset_path.display()))?;

    Ok(Some(StaticAsset {
        content_type: content_type_for(&asset_path),
        body,
    }))
}

fn load_package_asset(root: &Path, request_path: &str) -> Result<Option<StaticAsset>> {
    let normalized = normalize_request_path(request_path)?;
    let segments = path_segments(&normalized);
    if segments.len() < 4 || segments[0] != "_ax" || segments[1] != "pkg" {
        return Ok(None);
    }

    let package_name = &segments[2];
    let relative = segments[3..].join("/");
    let Some(package_root) = resolve_package_asset_root(root, package_name) else {
        return Ok(None);
    };
    let Some(asset_path) = package_asset_path(&package_root, &relative) else {
        return Ok(None);
    };

    if !asset_path.exists() || !asset_path.is_file() {
        return Ok(None);
    }

    let body = fs::read(&asset_path)
        .with_context(|| format!("failed to read package asset '{}'", asset_path.display()))?;

    Ok(Some(StaticAsset {
        content_type: content_type_for(&asset_path),
        body,
    }))
}

fn resolve_package_asset_root(root: &Path, package_name: &str) -> Option<PathBuf> {
    if package_name == "axonyx-ui" {
        let app_vendor = root.join("vendor").join("axonyx-ui");
        if app_vendor.exists() {
            return Some(app_vendor);
        }

        return cargo_package_root(root, package_name)
            .or_else(|| axonyx_ui_workspace_package_root(root));
    }

    cargo_package_root(root, package_name)
}

fn axonyx_ui_workspace_package_root(root: &Path) -> Option<PathBuf> {
    let workspace_root = root.parent()?;
    [
        workspace_root
            .join("axonyx-framework")
            .join("vendor")
            .join("axonyx-ui"),
        workspace_root.join("axonyx-ui"),
    ]
    .into_iter()
    .find(|package_root| package_root.exists())
}

fn axonyx_ui_workspace_import_bases(root: &Path) -> Vec<PathBuf> {
    let Some(workspace_root) = root.parent() else {
        return Vec::new();
    };

    vec![
        workspace_root
            .join("axonyx-framework")
            .join("vendor")
            .join("axonyx-ui")
            .join("src")
            .join("ax"),
        workspace_root
            .join("axonyx-framework")
            .join("vendor")
            .join("axonyx-ui")
            .join("src"),
        workspace_root.join("axonyx-ui").join("src").join("ax"),
        workspace_root.join("axonyx-ui").join("src"),
    ]
}

fn axonyx_ui_app_import_bases(root: &Path) -> Vec<PathBuf> {
    vec![
        root.join("vendor").join("axonyx-ui").join("src").join("ax"),
        root.join("vendor").join("axonyx-ui").join("src"),
    ]
}

fn package_asset_path(package_root: &Path, relative: &str) -> Option<PathBuf> {
    let relative_path = safe_relative_path(relative)?;
    let css_root = package_css_root(package_root);
    let css_entry = package_css_entry(package_root);

    if relative_path.components().count() == 1
        && css_entry
            .file_name()
            .is_some_and(|file_name| file_name == relative_path.as_os_str())
    {
        return Some(css_entry);
    }

    Some(css_root.join(relative_path))
}

fn package_css_root(package_root: &Path) -> PathBuf {
    package_metadata_export(package_root, "css_root")
        .map(|path| package_root.join(path))
        .unwrap_or_else(|| package_root.join("src").join("css"))
}

fn package_css_entry(package_root: &Path) -> PathBuf {
    package_metadata_export(package_root, "css_entry")
        .map(|path| package_root.join(path))
        .unwrap_or_else(|| package_root.join("src").join("css").join("index.css"))
}

fn package_metadata_export(package_root: &Path, key: &str) -> Option<String> {
    let source = fs::read_to_string(package_root.join("Axonyx.package.toml")).ok()?;
    let value = source.parse::<toml::Value>().ok()?;
    value
        .get("exports")
        .and_then(|exports| exports.get(key))
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

fn safe_relative_path(relative: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for segment in relative.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return None;
        }
        path.push(segment);
    }
    Some(path)
}

fn resolve_route(root: &Path, request_path: &str) -> Result<Option<ResolvedRoute>> {
    let normalized = normalize_request_path(request_path)?;
    let segments = path_segments(&normalized);
    let app_root = root.join("app");
    let Some((page_dir, matched_dirs, params)) = resolve_app_route_dir(&app_root, &segments)?
    else {
        return Ok(None);
    };
    let page_path = page_dir.join("page.ax");

    let mut layout_paths = Vec::new();
    let root_layout = app_root.join("layout.ax");
    if root_layout.exists() {
        layout_paths.push(root_layout);
    }

    let mut current = app_root;
    for segment in &matched_dirs {
        current = current.join(segment);
        let layout_path = current.join("layout.ax");
        if layout_path.exists() {
            layout_paths.push(layout_path);
        }
    }

    let loader_path = page_path
        .parent()
        .map(|parent| parent.join("loader.ax"))
        .filter(|path| path.exists());
    let actions_path = page_path
        .parent()
        .map(|parent| parent.join("actions.ax"))
        .filter(|path| path.exists());

    Ok(Some(ResolvedRoute {
        request_path: normalized,
        request_target: request_path.to_string(),
        page_path,
        layout_paths,
        loader_path,
        actions_path,
        params,
    }))
}

fn resolve_app_route_dir(
    app_root: &Path,
    segments: &[String],
) -> Result<
    Option<(
        PathBuf,
        Vec<String>,
        std::collections::BTreeMap<String, String>,
    )>,
> {
    resolve_app_route_dir_from(
        app_root,
        segments,
        Vec::new(),
        std::collections::BTreeMap::new(),
    )
}

fn resolve_app_route_dir_from(
    current: &Path,
    segments: &[String],
    matched_dirs: Vec<String>,
    params: std::collections::BTreeMap<String, String>,
) -> Result<
    Option<(
        PathBuf,
        Vec<String>,
        std::collections::BTreeMap<String, String>,
    )>,
> {
    if segments.is_empty() {
        if current.join("page.ax").exists() {
            return Ok(Some((current.to_path_buf(), matched_dirs, params)));
        }
        return Ok(None);
    }

    let segment = &segments[0];
    let exact_dir = current.join(segment);
    if exact_dir.is_dir() {
        let mut exact_dirs = matched_dirs.clone();
        exact_dirs.push(segment.clone());
        if let Some(found) =
            resolve_app_route_dir_from(&exact_dir, &segments[1..], exact_dirs, params.clone())?
        {
            return Ok(Some(found));
        }
    }

    let mut dynamic_dirs = fs::read_dir(current)
        .with_context(|| format!("failed to read directory '{}'", current.display()))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let param_name = parse_dynamic_app_segment(&name)?.to_string();
            Some((name, param_name, path))
        })
        .collect::<Vec<_>>();
    dynamic_dirs.sort_by(|left, right| left.0.cmp(&right.0));

    for (dir_name, param_name, dir_path) in dynamic_dirs {
        let mut next_dirs = matched_dirs.clone();
        next_dirs.push(dir_name);
        let mut next_params = params.clone();
        next_params.insert(param_name, segment.clone());
        if let Some(found) =
            resolve_app_route_dir_from(&dir_path, &segments[1..], next_dirs, next_params)?
        {
            return Ok(Some(found));
        }
    }

    Ok(None)
}

fn parse_dynamic_app_segment(segment: &str) -> Option<&str> {
    segment
        .strip_prefix('[')?
        .strip_suffix(']')
        .filter(|name| !name.is_empty())
}

fn render_route_html(state: &DevServerState, route: &ResolvedRoute) -> Result<String> {
    let page_source = fs::read_to_string(&route.page_path)
        .with_context(|| format!("failed to read '{}'", route.page_path.display()))?;
    let layout_sources = route
        .layout_paths
        .iter()
        .map(|path| {
            fs::read_to_string(path).with_context(|| format!("failed to read '{}'", path.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    let layout_refs = layout_sources
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let loader_sources = route
        .loader_path
        .iter()
        .map(|path| {
            fs::read_to_string(path).with_context(|| format!("failed to read '{}'", path.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    let loader_refs = loader_sources
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let action_sources = route
        .actions_path
        .iter()
        .map(|path| {
            fs::read_to_string(path).with_context(|| format!("failed to read '{}'", path.display()))
        })
        .collect::<Result<Vec<_>>>()?;
    let action_refs = action_sources
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let store = state
        .preview_store
        .lock()
        .map_err(|_| anyhow::anyhow!("preview store lock was poisoned"))?;
    let import_resolver = |source: &str| load_preview_import_source(&state.root, source);

    let html = preview_ax_route_with_request_context_and_imports(
        &layout_refs,
        &loader_refs,
        &action_refs,
        &page_source,
        &route.request_target,
        &route.params,
        &store,
        &import_resolver,
    )
    .with_context(|| {
        format!(
            "failed to render route '{}' from '{}'",
            route.request_path,
            route.page_path.display()
        )
    })?;

    Ok(apply_theme_config(&state.root, html))
}

fn route_version(root: &Path, route: &ResolvedRoute) -> Result<String> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut visited = std::collections::BTreeSet::new();
    route.request_path.hash(&mut hasher);
    let config_path = root.join("Axonyx.toml");
    if config_path.exists() {
        hash_file(&config_path, &mut hasher)?;
    }

    hash_ax_file_with_imports(root, &route.page_path, &mut hasher, &mut visited)?;
    for path in &route.layout_paths {
        hash_ax_file_with_imports(root, path, &mut hasher, &mut visited)?;
    }
    if let Some(path) = &route.loader_path {
        hash_file(path, &mut hasher)?;
    }
    if let Some(path) = &route.actions_path {
        hash_file(path, &mut hasher)?;
    }

    Ok(format!("{:x}", hasher.finish()))
}

fn apply_theme_config(root: &Path, html: String) -> String {
    let Some(theme_table) = axonyx_config_table(root, "theme") else {
        return html;
    };

    let mut html = html;

    if let Some(active) = theme_table
        .get("active")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        html = ensure_html_theme_attr(&html, active.trim());
    }

    if let Some(stylesheet) = theme_table
        .get("stylesheet")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        html = ensure_head_stylesheet(&html, stylesheet.trim());
    }

    html
}

fn ensure_html_theme_attr(html: &str, theme: &str) -> String {
    if html.contains("data-theme=") {
        return html.to_string();
    }

    html.replacen(
        "<html",
        &format!("<html data-theme=\"{}\"", html_escape(theme)),
        1,
    )
}

fn ensure_head_stylesheet(html: &str, stylesheet: &str) -> String {
    if html.contains(stylesheet) {
        return html.to_string();
    }

    let tag = format!(
        "<link rel=\"stylesheet\" href=\"{}\">",
        html_escape(stylesheet)
    );

    if html.contains("</head>") {
        return html.replacen("</head>", &format!("{tag}</head>"), 1);
    }

    html.to_string()
}

fn hash_file(path: &Path, hasher: &mut impl Hasher) -> Result<()> {
    path.to_string_lossy().hash(hasher);
    let contents = fs::read(path)
        .with_context(|| format!("failed to read '{}' for hashing", path.display()))?;
    contents.hash(hasher);
    Ok(())
}

fn hash_ax_file_with_imports(
    root: &Path,
    path: &Path,
    hasher: &mut impl Hasher,
    visited: &mut std::collections::BTreeSet<PathBuf>,
) -> Result<()> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if !visited.insert(canonical) {
        return Ok(());
    }

    hash_file(path, hasher)?;

    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read '{}'", path.display()))?;
    let Ok(document) = parse_ax_auto(&source) else {
        return Ok(());
    };

    for import_decl in document.imports {
        if let Some(import_path) = resolve_preview_import_path(root, &import_decl.source) {
            if import_path.exists() {
                hash_ax_file_with_imports(root, &import_path, hasher, visited)?;
            }
        }
    }

    Ok(())
}

fn load_preview_import_source(root: &Path, source: &str) -> Option<String> {
    let path = resolve_preview_import_path(root, source)?;
    fs::read_to_string(path).ok()
}

fn resolve_preview_import_path(root: &Path, source: &str) -> Option<PathBuf> {
    resolve_component_override_import_path(root, source)
        .or_else(|| resolve_package_override_import_path(root, source))
        .or_else(|| resolve_app_import_path(root, source))
        .or_else(|| resolve_axonyx_ui_app_import_path(root, source))
        .or_else(|| resolve_cargo_package_import_path(root, source))
        .or_else(|| resolve_axonyx_ui_workspace_import_path(root, source))
}

fn resolve_app_import_path(root: &Path, source: &str) -> Option<PathBuf> {
    let relative = source.strip_prefix("@/")?;
    let mut path = root.join("app");

    for segment in relative.split('/') {
        if !segment.is_empty() {
            path.push(segment);
        }
    }

    if path.extension().is_none() {
        path.set_extension("ax");
    }

    Some(path)
}

fn resolve_component_override_import_path(root: &Path, source: &str) -> Option<PathBuf> {
    let target = axonyx_config_string(root, "component_overrides", source)?;
    resolve_config_path(root, &target)
}

fn resolve_package_override_import_path(root: &Path, source: &str) -> Option<PathBuf> {
    let overrides = axonyx_config_table(root, "package_overrides")?;
    let mut matches = overrides
        .iter()
        .filter_map(|(package, value)| {
            let target = value.as_str()?;
            let relative = source
                .strip_prefix(package)
                .and_then(|rest| rest.strip_prefix('/'))?;
            Some((package.len(), target, relative))
        })
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| right.0.cmp(&left.0));
    let (_, target, relative) = matches.into_iter().next()?;

    let mut base = resolve_config_base_path(root, target)?;
    let ax_root = base.join("src").join("ax");
    let src_root = base.join("src");
    if ax_root.exists() {
        base = ax_root;
    } else if src_root.exists() {
        base = src_root;
    }

    Some(join_import_relative(base, relative))
}

fn resolve_cargo_package_import_path(root: &Path, source: &str) -> Option<PathBuf> {
    let (namespace, relative) = split_package_import(source)?;
    let package_name = cargo_package_name_for_namespace(root, namespace)?;
    let package_root = cargo_package_root(root, &package_name)?;
    let ax_root = cargo_package_ax_root(&package_root, namespace)
        .unwrap_or_else(|| default_package_ax_root(&package_root));

    Some(join_import_relative(ax_root, relative))
}

fn split_package_import(source: &str) -> Option<(&str, &str)> {
    let mut parts = source.splitn(3, '/');
    let scope = parts.next()?;
    if !scope.starts_with('@') {
        return None;
    }

    let package = parts.next()?;
    let relative = parts.next()?;
    let namespace_len = scope.len() + 1 + package.len();

    Some((&source[..namespace_len], relative))
}

fn cargo_package_name_for_namespace(root: &Path, namespace: &str) -> Option<String> {
    if let Some(value) = axonyx_config_value(root, "packages", namespace) {
        if let Some(package_name) = value.as_str() {
            return Some(package_name.to_string());
        }

        if let Some(package_name) = value
            .as_table()
            .and_then(|table| table.get("crate"))
            .and_then(|value| value.as_str())
        {
            return Some(package_name.to_string());
        }
    }

    match namespace {
        "@axonyx/ui" => Some("axonyx-ui".to_string()),
        _ => None,
    }
}

fn cargo_package_root(root: &Path, package_name: &str) -> Option<PathBuf> {
    let manifest_path = root.join("Cargo.toml");
    if !manifest_path.exists() {
        return None;
    }

    let cache_key = format!("{}::{package_name}", manifest_path.to_string_lossy());
    let cache =
        CARGO_PACKAGE_ROOT_CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    if let Ok(cache) = cache.lock() {
        if let Some(package_root) = cache.get(&cache_key) {
            return Some(package_root.clone());
        }
    }

    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .arg("--manifest-path")
        .arg(&manifest_path)
        .current_dir(root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let metadata = serde_json::from_slice::<serde_json::Value>(&output.stdout).ok()?;
    let packages = metadata.get("packages")?.as_array()?;
    let manifest = packages
        .iter()
        .find(|package| package.get("name").and_then(|name| name.as_str()) == Some(package_name))?
        .get("manifest_path")?
        .as_str()?;

    let package_root = PathBuf::from(manifest).parent().map(Path::to_path_buf)?;
    if let Ok(mut cache) = cache.lock() {
        cache.insert(cache_key, package_root.clone());
    }

    Some(package_root)
}

fn cargo_package_ax_root(package_root: &Path, namespace: &str) -> Option<PathBuf> {
    let source = fs::read_to_string(package_root.join("Axonyx.package.toml")).ok()?;
    let value = source.parse::<toml::Value>().ok()?;
    let metadata_namespace = value
        .get("package")
        .and_then(|package| package.get("namespace"))
        .and_then(|namespace| namespace.as_str())?;

    if metadata_namespace != namespace {
        return None;
    }

    let ax_root = value
        .get("exports")
        .and_then(|exports| exports.get("ax_root"))
        .and_then(|ax_root| ax_root.as_str())?;

    Some(package_root.join(ax_root))
}

fn default_package_ax_root(package_root: &Path) -> PathBuf {
    let ax_root = package_root.join("src").join("ax");
    if ax_root.exists() {
        return ax_root;
    }

    package_root.join("src")
}

fn resolve_axonyx_ui_workspace_import_path(root: &Path, source: &str) -> Option<PathBuf> {
    let relative = source.strip_prefix("@axonyx/ui/")?;
    let mut fallback = None;

    for base in axonyx_ui_workspace_import_bases(root) {
        let path = join_import_relative(base, relative);
        fallback.get_or_insert_with(|| path.clone());

        if path.exists() {
            return Some(path);
        }
    }

    fallback
}

fn resolve_axonyx_ui_app_import_path(root: &Path, source: &str) -> Option<PathBuf> {
    let relative = source.strip_prefix("@axonyx/ui/")?;

    axonyx_ui_app_import_bases(root)
        .into_iter()
        .map(|base| join_import_relative(base, relative))
        .find(|path| path.exists())
}

fn join_import_relative(mut base: PathBuf, relative: &str) -> PathBuf {
    for segment in relative.split('/') {
        if !segment.is_empty() {
            base.push(segment);
        }
    }

    if base.extension().is_none() {
        base.set_extension("ax");
    }

    base
}

fn resolve_config_path(root: &Path, target: &str) -> Option<PathBuf> {
    if target.starts_with("@/") {
        return resolve_app_import_path(root, target);
    }

    if target.starts_with("@axonyx/ui/") {
        return resolve_axonyx_ui_app_import_path(root, target)
            .or_else(|| resolve_cargo_package_import_path(root, target))
            .or_else(|| resolve_axonyx_ui_workspace_import_path(root, target));
    }

    let path = PathBuf::from(target);
    let mut path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };

    if path.extension().is_none() {
        path.set_extension("ax");
    }

    Some(path)
}

fn resolve_config_base_path(root: &Path, target: &str) -> Option<PathBuf> {
    let path = PathBuf::from(target);
    Some(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

fn axonyx_config_string(root: &Path, table: &str, key: &str) -> Option<String> {
    axonyx_config_table(root, table)?
        .get(key)?
        .as_str()
        .map(ToOwned::to_owned)
}

fn axonyx_config_value(root: &Path, table: &str, key: &str) -> Option<toml::Value> {
    axonyx_config_table(root, table)?.get(key).cloned()
}

fn axonyx_config_table(root: &Path, table: &str) -> Option<toml::map::Map<String, toml::Value>> {
    let source = fs::read_to_string(root.join("Axonyx.toml")).ok()?;
    let value = source.parse::<toml::Value>().ok()?;
    value.get(table)?.as_table().cloned()
}

fn inject_dev_client(html: &str, request_path: &str) -> String {
    let route_literal = format!("{request_path:?}");
    let script = format!(
        r#"<script>
(() => {{
  const route = {route_literal};
  let version = null;
  const poll = async () => {{
    try {{
      const response = await fetch(`/__axonyx/version?path=${{encodeURIComponent(route)}}`, {{ cache: "no-store" }});
      if (!response.ok) return;
      const next = (await response.text()).trim();
      if (version === null) {{
        version = next;
        return;
      }}
      if (next && next !== version) {{
        window.location.reload();
      }}
    }} catch (_error) {{
      // Dev-only polling can stay quiet while the server restarts.
    }}
  }};
  setInterval(poll, 900);
}})();
</script>"#
    );

    if let Some(index) = html.rfind("</body>") {
        let mut with_client = String::with_capacity(html.len() + script.len());
        with_client.push_str(&html[..index]);
        with_client.push_str(&script);
        with_client.push_str(&html[index..]);
        with_client
    } else {
        let mut with_client = html.to_string();
        with_client.push_str(&script);
        with_client
    }
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );

    stream
        .write_all(header.as_bytes())
        .context("failed to write response headers")?;
    stream
        .write_all(body)
        .context("failed to write response body")?;
    stream.flush().context("failed to flush response")?;
    Ok(())
}

fn extract_version_path(target: &str) -> Option<String> {
    let query = target.split_once('?')?.1;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=')?;
        if key == "path" {
            return Some(url_decode(value));
        }
    }
    None
}

fn extract_action_query_param(target: &str, needle: &str) -> Option<String> {
    let query = target.split_once('?')?.1;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if key == needle {
            return Some(url_decode(value));
        }
    }
    None
}

fn normalize_request_path(request_path: &str) -> Result<String> {
    let raw_path = request_path.split(['?', '#']).next().unwrap_or("/").trim();
    let raw_path = if raw_path.is_empty() { "/" } else { raw_path };
    let mut segments = Vec::new();

    for segment in raw_path.trim_start_matches('/').split('/') {
        if segment.is_empty() {
            continue;
        }
        if segment == "." || segment == ".." || segment.contains('\\') {
            bail!("invalid route path '{request_path}'");
        }
        segments.push(segment.to_string());
    }

    if segments.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", segments.join("/")))
    }
}

fn path_segments(path: &str) -> Vec<String> {
    path.trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn url_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = String::with_capacity(value.len());
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &value[index + 1..index + 3];
                if let Ok(decoded) = u8::from_str_radix(hex, 16) {
                    out.push(decoded as char);
                    index += 3;
                } else {
                    out.push('%');
                    index += 1;
                }
            }
            byte => {
                out.push(byte as char);
                index += 1;
            }
        }
    }

    out
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn looks_like_asset_request(request_path: &str) -> bool {
    request_path
        .split(['?', '#'])
        .next()
        .and_then(|path| path.rsplit('/').next())
        .is_some_and(|segment| segment.contains('.'))
}

fn content_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_temp_dir(name: &str) -> PathBuf {
        let unique = format!(
            "axonyx-cargo-test-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).expect("temp dir should create");
        path
    }

    fn write_test_axonyx_ui_package(root: &Path, card_title: &str, css: &str) {
        fs::create_dir_all(root.join("src/foundry")).expect("ui foundry dir should exist");
        fs::create_dir_all(root.join("src/css")).expect("ui css dir should exist");
        fs::write(
            root.join("Cargo.toml"),
            r#"
[package]
name = "axonyx-ui"
version = "0.0.0"
edition = "2021"

[lib]
path = "src/lib.rs"
"#,
        )
        .expect("ui cargo manifest should write");
        fs::write(root.join("src/lib.rs"), "").expect("ui lib should write");
        fs::write(
            root.join("Axonyx.package.toml"),
            r#"
[package]
name = "axonyx-ui"
namespace = "@axonyx/ui"

[exports]
ax_root = "src"
css_root = "src/css"
css_entry = "src/css/index.css"
"#,
        )
        .expect("ui package metadata should write");
        fs::write(
            root.join("src/foundry/SectionCard.ax"),
            format!(
                r#"
page SectionCard
<Card title="{card_title}" />
"#
            ),
        )
        .expect("ui component should write");
        fs::write(root.join("src/css/index.css"), css).expect("ui css should write");
    }

    #[test]
    fn resolve_route_collects_nested_layouts() {
        let root = make_temp_dir("route");
        fs::create_dir_all(root.join("app/docs")).expect("app/docs should exist");
        fs::write(root.join("app/layout.ax"), "page RootLayout\n  Slot\n")
            .expect("root layout should write");
        fs::write(root.join("app/docs/layout.ax"), "page DocsLayout\n  Slot\n")
            .expect("nested layout should write");
        fs::write(
            root.join("app/docs/loader.ax"),
            "loader DocsList\n  data docs = Db.Stream(\"docs\")\n  return docs\n",
        )
        .expect("loader should write");
        fs::write(
            root.join("app/docs/actions.ax"),
            "action CreateDoc\n  input:\n    title: string\n\n  return ok\n",
        )
        .expect("actions should write");
        fs::write(
            root.join("app/docs/page.ax"),
            "page DocsHome\n  Copy -> \"Docs\"\n",
        )
        .expect("page should write");

        let route = resolve_route(&root, "/docs")
            .expect("route resolution should work")
            .expect("route should exist");

        assert_eq!(route.request_path, "/docs");
        assert_eq!(route.request_target, "/docs");
        assert_eq!(route.layout_paths.len(), 2);
        assert!(route
            .loader_path
            .as_ref()
            .is_some_and(|path| path.ends_with(Path::new("app").join("docs").join("loader.ax"))));
        assert!(route
            .actions_path
            .as_ref()
            .is_some_and(|path| path.ends_with(Path::new("app").join("docs").join("actions.ax"))));
        assert!(route
            .page_path
            .ends_with(Path::new("app").join("docs").join("page.ax")));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn resolve_route_supports_dynamic_app_segments() {
        let root = make_temp_dir("dynamic-route");
        fs::create_dir_all(root.join("app/posts/[slug]")).expect("dynamic app dir should exist");
        fs::write(
            root.join("app/posts/[slug]/page.ax"),
            "page Post\n  Copy -> params.slug\n",
        )
        .expect("page should write");

        let route = resolve_route(&root, "/posts/hello-axonyx")
            .expect("route resolution should work")
            .expect("route should exist");

        assert_eq!(route.request_path, "/posts/hello-axonyx");
        assert_eq!(
            route.params.get("slug").map(String::as_str),
            Some("hello-axonyx")
        );
        assert!(route.page_path.ends_with(
            Path::new("app")
                .join("posts")
                .join("[slug]")
                .join("page.ax")
        ));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn resolve_route_prefers_static_app_segment_over_dynamic_one() {
        let root = make_temp_dir("dynamic-static-route");
        fs::create_dir_all(root.join("app/posts/[slug]")).expect("dynamic app dir should exist");
        fs::create_dir_all(root.join("app/posts/featured")).expect("static app dir should exist");
        fs::write(
            root.join("app/posts/[slug]/page.ax"),
            "page DynamicPost\n  Copy -> params.slug\n",
        )
        .expect("dynamic page should write");
        fs::write(
            root.join("app/posts/featured/page.ax"),
            "page FeaturedPost\n  Copy -> \"Featured\"\n",
        )
        .expect("static page should write");

        let route = resolve_route(&root, "/posts/featured")
            .expect("route resolution should work")
            .expect("route should exist");

        assert!(route.page_path.ends_with(
            Path::new("app")
                .join("posts")
                .join("featured")
                .join("page.ax")
        ));
        assert!(route.params.is_empty());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn collects_app_route_manifest_for_cli_output() {
        let root = make_temp_dir("route-manifest");
        fs::create_dir_all(root.join("app/posts/[slug]")).expect("dynamic app dir should exist");
        fs::write(root.join("app/layout.ax"), "page RootLayout\n<Slot />\n")
            .expect("root layout should write");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy>Home</Copy>\n")
            .expect("home page should write");
        fs::write(
            root.join("app/posts/[slug]/layout.ax"),
            "page PostLayout\n<Slot />\n",
        )
        .expect("post layout should write");
        fs::write(
            root.join("app/posts/[slug]/page.ax"),
            "page Post\n<Copy>{params.slug}</Copy>\n",
        )
        .expect("post page should write");
        fs::write(
            root.join("app/posts/[slug]/loader.ax"),
            "loader Post\n  return ok\n",
        )
        .expect("loader should write");
        fs::write(
            root.join("app/posts/[slug]/actions.ax"),
            "action Save\n  return ok\n",
        )
        .expect("actions should write");

        let routes = collect_app_route_manifest(&root).expect("manifest should collect");

        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].kind, "page");
        assert_eq!(routes[0].route, "/");
        assert_eq!(routes[0].method, None);
        assert_eq!(routes[0].file, "app/page.ax");
        assert_eq!(routes[0].layouts, vec!["app/layout.ax"]);

        assert_eq!(routes[1].kind, "page");
        assert_eq!(routes[1].route, "/posts/:slug");
        assert_eq!(routes[1].params, vec!["slug"]);
        assert_eq!(routes[1].layouts.len(), 2);
        assert_eq!(
            routes[1].loader.as_deref(),
            Some("app/posts/[slug]/loader.ax")
        );
        assert_eq!(
            routes[1].actions.as_deref(),
            Some("app/posts/[slug]/actions.ax")
        );

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn route_manifest_includes_backend_api_routes() {
        let root = make_temp_dir("backend-route-manifest");
        fs::create_dir_all(root.join("routes/api")).expect("api routes dir should exist");
        fs::write(
            root.join("routes/api/posts.ax"),
            r#"
route GET "/api/posts"
  return ok

route POST "/api/posts/:slug"
  return ok
"#,
        )
        .expect("route source should write");

        let routes = collect_app_route_manifest(&root).expect("manifest should collect");

        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].kind, "api");
        assert_eq!(routes[0].method.as_deref(), Some("GET"));
        assert_eq!(routes[0].route, "/api/posts");
        assert_eq!(routes[0].file, "routes/api/posts.ax");

        assert_eq!(routes[1].kind, "api");
        assert_eq!(routes[1].method.as_deref(), Some("POST"));
        assert_eq!(routes[1].route, "/api/posts/:slug");
        assert_eq!(routes[1].params, vec!["slug"]);

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_app_sources_reports_duplicate_page_route_patterns() {
        let root = make_temp_dir("duplicate-page-routes");
        fs::create_dir_all(root.join("app/posts/[slug]")).expect("slug route should exist");
        fs::create_dir_all(root.join("app/posts/[id]")).expect("id route should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/posts/[slug]/page.ax"),
            "page PostBySlug\n<Copy>{params.slug}</Copy>\n",
        )
        .expect("slug page should write");
        fs::write(
            root.join("app/posts/[id]/page.ax"),
            "page PostById\n<Copy>{params.id}</Copy>\n",
        )
        .expect("id page should write");

        let diagnostics = check_app_sources(&root).expect("check should run");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "axonyx-route-duplicate");
        assert!(diagnostics[0].message.contains("/posts/:"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_app_sources_reports_duplicate_backend_api_routes() {
        let root = make_temp_dir("duplicate-api-routes");
        fs::create_dir_all(root.join("routes/api")).expect("api dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("routes/api/posts.ax"),
            r#"
route GET "/api/posts"
  return ok
"#,
        )
        .expect("first route should write");
        fs::write(
            root.join("routes/api/posts-copy.ax"),
            r#"
route GET "/api/posts"
  return ok
"#,
        )
        .expect("second route should write");

        let diagnostics = check_app_sources(&root).expect("check should run");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "axonyx-route-duplicate");
        assert!(diagnostics[0].message.contains("GET /api/posts"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn normalize_request_path_rejects_parent_segments() {
        assert!(normalize_request_path("/../secret").is_err());
    }

    #[test]
    fn inject_dev_client_adds_reload_script() {
        let html = inject_dev_client("<html><body><main>Hello</main></body></html>", "/docs");

        assert!(html.contains("/__axonyx/version"));
        assert!(html.contains("window.location.reload"));
    }

    #[test]
    fn loads_public_asset_from_public_directory() {
        let root = make_temp_dir("public");
        fs::create_dir_all(root.join("public")).expect("public dir should exist");
        fs::write(root.join("public/logo.svg"), "<svg></svg>").expect("asset should write");

        let asset = load_public_asset(&root, "/logo.svg")
            .expect("asset lookup should work")
            .expect("asset should exist");

        assert_eq!(asset.content_type, "image/svg+xml");
        assert_eq!(asset.body, b"<svg></svg>");

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn missing_public_asset_returns_none() {
        let root = make_temp_dir("missing-public");

        let asset = load_public_asset(&root, "/missing.svg").expect("asset lookup should work");

        assert!(asset.is_none());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn loads_package_asset_from_cargo_dependency() {
        let workspace = make_temp_dir("package-asset-cargo");
        let root = workspace.join("axonyx-site");
        let ui_root = workspace.join("axonyx-ui");
        let ui_path = ui_root.to_string_lossy().replace('\\', "\\\\");

        fs::create_dir_all(&root).expect("app dir should exist");
        write_test_axonyx_ui_package(&ui_root, "Cargo UI", "body { color: silver; }");
        fs::write(
            root.join("Cargo.toml"),
            format!(
                r#"
[package]
name = "axonyx-site"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-ui = {{ path = "{ui_path}" }}
"#
            ),
        )
        .expect("app cargo manifest should write");

        let asset = load_package_asset(&root, "/_ax/pkg/axonyx-ui/index.css")
            .expect("package asset lookup should work")
            .expect("package asset should exist");

        assert_eq!(asset.content_type, "text/css; charset=utf-8");
        assert_eq!(asset.body, b"body { color: silver; }");

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn package_asset_rejects_parent_segments() {
        let root = make_temp_dir("package-asset-parent");

        assert!(load_package_asset(&root, "/_ax/pkg/axonyx-ui/../secret.css").is_err());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_command_generates_backend_module_from_app_sources() {
        let root = make_temp_dir("build");
        fs::create_dir_all(root.join("routes").join("api")).expect("routes dir should exist");
        fs::create_dir_all(root.join("src").join("generated")).expect("generated dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("routes").join("api").join("posts.ax"),
            "route GET \"/api/posts\"\n  data posts = Db.Stream(\"posts\")\n  return posts\n",
        )
        .expect("route should write");

        let status = compile_backend_from_app_root(&root).expect("build should succeed");

        match status {
            BackendBuildStatus::Generated {
                source_count,
                output_path,
            } => {
                assert_eq!(source_count, 1);
                let module =
                    fs::read_to_string(output_path).expect("generated backend should be readable");
                assert!(module.contains("pub fn route_get_api_posts"));
            }
            BackendBuildStatus::NoSources { .. } => panic!("backend sources should be found"),
        }

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_preflight_reports_file_level_diagnostics() {
        let root = make_temp_dir("build-preflight-diagnostics");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { MissingCard } from "@/components/MissingCard.ax"

page Home
<MissingCard />
"#,
        )
        .expect("page should write");

        let error = ensure_no_check_diagnostics(&root).expect_err("diagnostics should fail");
        let message = error.to_string();

        assert!(message.contains("Axonyx diagnostics failed before build"));
        assert!(message.contains("app/page.ax"));
        assert!(message.contains("axonyx-import"));
        assert!(message.contains("@/components/MissingCard.ax"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn cli_error_hint_detects_prerender_config_errors() {
        let error = anyhow::anyhow!("prerender route '/blog/:slug' is missing params");

        assert_eq!(
            hint_for_error(&error),
            Some("Check the [prerender] routes in Axonyx.toml. Dynamic params must be strings.")
        );
    }

    #[test]
    fn build_static_site_generates_html_and_copies_public_assets() {
        let root = make_temp_dir("static-build");
        fs::create_dir_all(root.join("app/docs")).expect("docs dir should exist");
        fs::create_dir_all(root.join("public/css")).expect("public css dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/layout.ax"),
            r#"
page RootLayout
<Head>
  <Title>Demo</Title>
</Head>
<Slot />
"#,
        )
        .expect("layout should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home
<Copy>Home page</Copy>
"#,
        )
        .expect("home page should write");
        fs::write(
            root.join("app/docs/page.ax"),
            r#"
page Docs
<Copy>Docs page</Copy>
"#,
        )
        .expect("docs page should write");
        fs::write(root.join("public/css/site.css"), "body { color: red; }")
            .expect("asset should write");

        let status = build_static_site_from_app_root(&root, Path::new("dist"), false)
            .expect("static build works");

        match status {
            StaticBuildStatus::Generated {
                route_count,
                prerendered_count,
                skipped_dynamic_count,
                content_collection_count,
                output_dir,
            } => {
                assert_eq!(route_count, 2);
                assert_eq!(prerendered_count, 0);
                assert_eq!(skipped_dynamic_count, 0);
                assert_eq!(content_collection_count, 0);
                assert_eq!(output_dir, root.join("dist"));
            }
            StaticBuildStatus::NoPages { .. } => panic!("static pages should be found"),
        }

        let home =
            fs::read_to_string(root.join("dist/index.html")).expect("home html should exist");
        let docs =
            fs::read_to_string(root.join("dist/docs/index.html")).expect("docs html should exist");
        assert!(home.contains("Home page"));
        assert!(docs.contains("Docs page"));
        assert!(root.join("dist/css/site.css").exists());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_static_site_copies_package_css_assets() {
        let root = make_temp_dir("static-build-package-css");
        let ui_root = root.join("vendor/axonyx-ui");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        write_test_axonyx_ui_package(&ui_root, "Vendored UI", "body { color: gold; }");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home
<Head>
  <Link rel="stylesheet" href="/_ax/pkg/axonyx-ui/index.css" />
</Head>
<Container>
  <Copy>Static package CSS</Copy>
</Container>
"#,
        )
        .expect("page should write");

        let status = build_static_site_from_app_root(&root, Path::new("dist"), true)
            .expect("static site should build");

        assert!(matches!(status, StaticBuildStatus::Generated { .. }));
        assert_eq!(
            fs::read_to_string(root.join("dist/_ax/pkg/axonyx-ui/index.css"))
                .expect("package css should copy"),
            "body { color: gold; }"
        );

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_static_site_skips_dynamic_routes_until_prerender_config_exists() {
        let root = make_temp_dir("static-build-dynamic");
        fs::create_dir_all(root.join("app/blog/[slug]")).expect("blog dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/blog/[slug]/page.ax"),
            r#"
page BlogPost
<Copy>Dynamic post</Copy>
"#,
        )
        .expect("dynamic page should write");

        let status = build_static_site_from_app_root(&root, Path::new("dist"), false)
            .expect("static build works");

        match status {
            StaticBuildStatus::NoPages {
                skipped_dynamic_count,
                content_collection_count,
                output_dir,
            } => {
                assert_eq!(output_dir, root.join("dist"));
                assert_eq!(skipped_dynamic_count, 1);
                assert_eq!(content_collection_count, 0);
            }
            StaticBuildStatus::Generated {
                route_count,
                prerendered_count,
                skipped_dynamic_count,
                content_collection_count,
                ..
            } => {
                assert_eq!(route_count, 0);
                assert_eq!(prerendered_count, 0);
                assert_eq!(skipped_dynamic_count, 1);
                assert_eq!(content_collection_count, 0);
            }
        }
        assert!(!root.join("dist/blog/[slug]/index.html").exists());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_static_site_prerenders_dynamic_routes_from_config() {
        let root = make_temp_dir("static-build-prerender");
        fs::create_dir_all(root.join("app/blog/[slug]")).expect("blog dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[prerender]
routes = [
  { route = "/blog/:slug", params = [{ slug = "hello-axonyx" }, { slug = "foundry-ui" }] },
]
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("app/blog/[slug]/page.ax"),
            r#"
page BlogPost
<Copy>Post slug: {params.slug}</Copy>
"#,
        )
        .expect("dynamic page should write");

        let status = build_static_site_from_app_root(&root, Path::new("dist"), false)
            .expect("static build works");

        match status {
            StaticBuildStatus::Generated {
                route_count,
                prerendered_count,
                skipped_dynamic_count,
                content_collection_count,
                output_dir,
            } => {
                assert_eq!(route_count, 0);
                assert_eq!(prerendered_count, 2);
                assert_eq!(skipped_dynamic_count, 0);
                assert_eq!(content_collection_count, 0);
                assert_eq!(output_dir, root.join("dist"));
            }
            StaticBuildStatus::NoPages { .. } => panic!("prerender pages should be generated"),
        }

        let hello = fs::read_to_string(root.join("dist/blog/hello-axonyx/index.html"))
            .expect("hello page should exist");
        let foundry = fs::read_to_string(root.join("dist/blog/foundry-ui/index.html"))
            .expect("foundry page should exist");
        assert!(hello.contains("hello-axonyx"));
        assert!(foundry.contains("foundry-ui"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_static_site_clean_removes_previous_output() {
        let root = make_temp_dir("static-build-clean");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(root.join("dist/stale")).expect("stale dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(root.join("dist/stale/file.txt"), "old").expect("stale file should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home
<Copy>Fresh build</Copy>
"#,
        )
        .expect("page should write");

        build_static_site_from_app_root(&root, Path::new("dist"), true)
            .expect("static build should clean");

        assert!(!root.join("dist/stale/file.txt").exists());
        assert!(root.join("dist/index.html").exists());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_static_site_clean_refuses_to_remove_app_root() {
        let root = make_temp_dir("static-build-clean-root");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");

        let error = build_static_site_from_app_root(&root, Path::new("."), true)
            .expect_err("clean should refuse app root");

        assert!(error.to_string().contains("--clean refuses to remove"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn strips_cargo_subcommand_prefix_for_ax() {
        let args = vec![
            OsString::from("cargo-ax.exe"),
            OsString::from("ax"),
            OsString::from("run"),
            OsString::from("dev"),
        ];

        let normalized = {
            let mut args = args;
            if args
                .get(1)
                .and_then(|value| value.to_str())
                .is_some_and(|value| matches!(value, "ax" | "axonyx"))
            {
                args.remove(1);
            }
            args
        };

        let cli = Cli::try_parse_from(normalized).expect("cargo ax args should parse");

        assert!(matches!(cli.command, Commands::Run(_)));
    }

    #[test]
    fn add_ui_module_adds_registry_dependency_and_updates_layout() {
        let workspace = make_temp_dir("add-ui");
        let app_root = workspace.join("demo-app");

        fs::create_dir_all(app_root.join("app")).expect("app dir should exist");
        fs::write(
            app_root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[modules]\nenabled = []\n",
        )
        .expect("config should write");
        fs::write(
            app_root.join("Cargo.toml"),
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = "0.1.0"
"#,
        )
        .expect("cargo manifest should write");
        fs::write(
            app_root.join("app/layout.ax"),
            "page RootLayout\n  title \"Demo\"\n  Slot\n",
        )
        .expect("layout should write");

        add_ui_module(&app_root, &app_root.join("Axonyx.toml")).expect("ui module should add");

        assert!(!app_root.join("vendor/axonyx-ui").exists());
        assert!(!app_root.join("public/css/axonyx-ui").exists());

        let axonyx_toml =
            fs::read_to_string(app_root.join("Axonyx.toml")).expect("config should read back");
        assert!(axonyx_toml.contains("\"ui\""));
        assert!(!axonyx_toml.contains("[package_overrides]"));

        let layout =
            fs::read_to_string(app_root.join("app/layout.ax")).expect("layout should read back");
        assert!(layout.contains("theme \"silver\""));
        assert!(layout.contains("/_ax/pkg/axonyx-ui/index.css"));

        let cargo_toml =
            fs::read_to_string(app_root.join("Cargo.toml")).expect("cargo manifest should read");
        assert!(cargo_toml.contains("axonyx-ui = \"0.0.33\""));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_reports_healthy_ui_package_setup() {
        let workspace = make_temp_dir("doctor-healthy-ui");
        let app_root = workspace.join("demo-app");
        let ui_root = app_root.join("vendor/axonyx-ui");

        fs::create_dir_all(app_root.join("app")).expect("app dir should exist");
        write_test_axonyx_ui_package(&ui_root, "Doctor UI", "body { color: silver; }");
        fs::write(
            app_root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[modules]\nenabled = [\"ui\"]\n",
        )
        .expect("config should write");
        fs::write(
            app_root.join("Cargo.toml"),
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = "0.1.0"

[dependencies.axonyx-ui]
path = "vendor/axonyx-ui"
"#,
        )
        .expect("cargo manifest should write");
        fs::write(
            app_root.join("app/layout.ax"),
            r#"
page RootLayout
<Head>
  <Link rel="stylesheet" href="/_ax/pkg/axonyx-ui/index.css" />
</Head>
<Slot />
"#,
        )
        .expect("layout should write");

        let checks = doctor_checks(&app_root);

        assert!(checks
            .iter()
            .all(|check| check.severity == DoctorSeverity::Ok));
        assert!(checks.iter().any(|check| check.code == "ui-package-css"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_warns_when_ui_dependency_is_missing() {
        let workspace = make_temp_dir("doctor-missing-ui-dependency");
        let app_root = workspace.join("demo-app");
        let ui_root = app_root.join("vendor/axonyx-ui");

        fs::create_dir_all(app_root.join("app")).expect("app dir should exist");
        write_test_axonyx_ui_package(&ui_root, "Doctor UI", "body { color: silver; }");
        fs::write(
            app_root.join("Axonyx.toml"),
            "[app]\nname = \"demo\"\n\n[modules]\nenabled = [\"ui\"]\n",
        )
        .expect("config should write");
        fs::write(
            app_root.join("Cargo.toml"),
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = "0.1.0"
"#,
        )
        .expect("cargo manifest should write");
        fs::write(
            app_root.join("app/layout.ax"),
            r#"
page RootLayout
<Head>
  <Link rel="stylesheet" href="/_ax/pkg/axonyx-ui/index.css" />
</Head>
<Slot />
"#,
        )
        .expect("layout should write");

        let checks = doctor_checks(&app_root);
        let ui_dependency = checks
            .iter()
            .find(|check| check.code == "ui-cargo-dependency")
            .expect("ui dependency check should exist");

        assert_eq!(ui_dependency.severity, DoctorSeverity::Warn);
        assert!(ui_dependency.message.contains("axonyx-ui"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn doctor_summary_counts_severities_and_deny_warnings_can_fail() {
        let checks = vec![
            DoctorCheck {
                code: "ok-check",
                severity: DoctorSeverity::Ok,
                message: "ok".to_string(),
                hint: None,
            },
            DoctorCheck {
                code: "warn-check",
                severity: DoctorSeverity::Warn,
                message: "warn".to_string(),
                hint: None,
            },
        ];

        let summary = doctor_summary(&checks);

        assert_eq!(summary.ok, 1);
        assert_eq!(summary.warn, 1);
        assert_eq!(summary.error, 0);
        assert!(!doctor_should_fail(&checks, false));
        assert!(doctor_should_fail(&checks, true));
    }

    #[test]
    fn doctor_reports_ax_source_diagnostics() {
        let root = make_temp_dir("doctor-ax-diagnostics");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("Cargo.toml"),
            r#"
[package]
name = "demo-app"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-runtime = "0.1.0"
"#,
        )
        .expect("cargo manifest should write");
        fs::write(root.join("app/page.ax"), "page Home\n<Copy></Card>\n")
            .expect("page should write");

        let checks = doctor_checks(&root);
        let ax_sources = checks
            .iter()
            .find(|check| check.code == "ax-sources")
            .expect("ax source check should exist");

        assert_eq!(ax_sources.severity, DoctorSeverity::Error);
        assert!(ax_sources.message.contains("diagnostic"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn jsx_layout_setup_adds_theme_and_stylesheet_inside_head() {
        let source = r#"page SiteLayout

<Head>
  <Title>Demo</Title>
</Head>

<Container max="xl">
  <Slot />
</Container>"#;

        let updated = ensure_ui_layout_setup_jsx(source);

        assert!(updated.contains("<Theme>silver</Theme>"));
        assert!(
            updated.contains(r#"<Link rel="stylesheet" href="/_ax/pkg/axonyx-ui/index.css" />"#)
        );
        assert!(updated.contains("<Title>Demo</Title>"));
    }

    #[test]
    fn parse_form_body_decodes_urlencoded_pairs() {
        let fields = parse_form_body(b"title=Hello+Axonyx&excerpt=Fast%20forms");

        assert_eq!(
            fields.get("title").map(String::as_str),
            Some("Hello Axonyx")
        );
        assert_eq!(
            fields.get("excerpt").map(String::as_str),
            Some("Fast forms")
        );
    }

    #[test]
    fn executes_backend_route_request_from_routes_directory() {
        let root = make_temp_dir("api-route");
        fs::create_dir_all(root.join("routes").join("api")).expect("routes dir should exist");
        fs::write(
            root.join("routes").join("api").join("posts.ax"),
            "route GET \"/api/posts\"\n  data posts = Db.Stream(\"posts\")\n    where status = \"published\"\n    limit 2\n  return posts\n",
        )
        .expect("route should write");

        let state = DevServerState {
            root: root.clone(),
            preview_store: Mutex::new(AxPreviewStore::default()),
        };
        let request = HttpRequest {
            method: "GET".to_string(),
            target: "/api/posts".to_string(),
            headers: std::collections::BTreeMap::new(),
            body: Vec::new(),
        };

        let response = execute_backend_route_request(&state, &request)
            .expect("backend route request should succeed")
            .expect("backend route should match");

        assert_eq!(response.status, 200);
        assert_eq!(response.content_type, "application/json; charset=utf-8");
        let body = String::from_utf8(response.body).expect("json response should be utf-8");
        assert!(body.contains("Hello Axonyx"));
        assert!(body.contains("Docs Without Bloat"));
        assert!(!body.contains("Draft Preview"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn executes_dynamic_backend_route_request_with_query_context() {
        let root = make_temp_dir("api-route-dynamic");
        fs::create_dir_all(root.join("routes").join("api").join("posts"))
            .expect("routes dir should exist");
        fs::write(
            root.join("routes")
                .join("api")
                .join("posts")
                .join("show.ax"),
            "route GET \"/api/posts/:slug\"\n  data posts = Db.Stream(\"posts\")\n    where slug = params.slug\n    where status = query.status\n  return posts\n",
        )
        .expect("route should write");

        let state = DevServerState {
            root: root.clone(),
            preview_store: Mutex::new(AxPreviewStore::default()),
        };
        let request = HttpRequest {
            method: "GET".to_string(),
            target: "/api/posts/draft-preview?status=draft".to_string(),
            headers: std::collections::BTreeMap::new(),
            body: Vec::new(),
        };

        let response = execute_backend_route_request(&state, &request)
            .expect("backend route request should succeed")
            .expect("backend route should match");

        assert_eq!(response.status, 200);
        let body = String::from_utf8(response.body).expect("json response should be utf-8");
        assert!(body.contains("Draft Preview"));
        assert!(!body.contains("Hello Axonyx"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn renders_dynamic_page_route_with_loader_params_and_query_context() {
        let root = make_temp_dir("dynamic-page-render");
        fs::create_dir_all(root.join("app/posts/[slug]")).expect("dynamic app dir should exist");
        fs::write(
            root.join("app/posts/[slug]/loader.ax"),
            "loader PostDetail\n  data posts = Db.Stream(\"posts\")\n    where slug = params.slug\n    where status = query.status\n  return posts\n",
        )
        .expect("loader should write");
        fs::write(
            root.join("app/posts/[slug]/page.ax"),
            "page Post\n  data posts = load PostDetail\n  each post in posts\n    Copy -> post.title\n",
        )
        .expect("page should write");

        let route = resolve_route(&root, "/posts/draft-preview?status=draft")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = DevServerState {
            root: root.clone(),
            preview_store: Mutex::new(AxPreviewStore::default()),
        };
        let html = render_route_html(&state, &route).expect("dynamic route should render");

        assert!(html.contains("Draft Preview"));
        assert!(!html.contains("Hello Axonyx"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn renders_page_with_imported_app_component() {
        let root = make_temp_dir("imported-app-component");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(
            root.join("app/components/site-card.ax"),
            r#"
page SiteCard
<Card title={title}>
  <Slot />
</Card>
"#,
        )
        .expect("component should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { SiteCard } from "@/components/site-card.ax"

page Home
<SiteCard title="Hello from import">
  <Copy>Inner body</Copy>
</SiteCard>
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = DevServerState {
            root: root.clone(),
            preview_store: Mutex::new(AxPreviewStore::default()),
        };
        let html =
            render_route_html(&state, &route).expect("imported component route should render");

        assert!(html.contains("Hello from import"));
        assert!(html.contains("Inner body"));
        assert!(!html.contains("data-import-source"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn render_route_applies_theme_config_when_head_has_no_theme() {
        let root = make_temp_dir("theme-config-render");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[theme]
active = "silver"
stylesheet = "/css/axonyx-ui/index.css"
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home
<Copy>Hello theme</Copy>
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = DevServerState {
            root: root.clone(),
            preview_store: Mutex::new(AxPreviewStore::default()),
        };
        let html = render_route_html(&state, &route).expect("route should render");

        assert!(html.contains(r#"<html data-theme="silver" lang="en">"#));
        assert!(html.contains(r#"<link rel="stylesheet" href="/css/axonyx-ui/index.css">"#));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn render_route_keeps_explicit_head_theme_over_config_theme() {
        let root = make_temp_dir("theme-config-explicit");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[theme]
active = "silver"
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home

<Head>
  <Theme>gold</Theme>
</Head>

<Copy>Hello explicit theme</Copy>
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = DevServerState {
            root: root.clone(),
            preview_store: Mutex::new(AxPreviewStore::default()),
        };
        let html = render_route_html(&state, &route).expect("route should render");

        assert!(html.contains(r#"data-theme="gold""#));
        assert!(!html.contains(r#"data-theme="silver""#));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn route_version_changes_when_imported_component_changes() {
        let root = make_temp_dir("route-version-imports");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(
            root.join("app/components/site-card.ax"),
            r#"
page SiteCard
<Card title="Initial" />
"#,
        )
        .expect("component should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { SiteCard } from "@/components/site-card.ax"

page Home
<SiteCard />
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let before = route_version(&root, &route).expect("initial version should hash");

        fs::write(
            root.join("app/components/site-card.ax"),
            r#"
page SiteCard
<Card title="Updated" />
"#,
        )
        .expect("component should update");

        let after = route_version(&root, &route).expect("updated version should hash");
        assert_ne!(before, after);

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn renders_page_with_imported_axonyx_ui_component() {
        let workspace = make_temp_dir("ui-package-workspace");
        let root = workspace.join("axonyx-site");
        let ui_root = workspace.join("axonyx-ui");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(ui_root.join("src/ax/foundry")).expect("ui ax dir should exist");
        fs::write(
            ui_root.join("src/ax/foundry/SectionCard.ax"),
            r#"
page SectionCard
<Card title={title}>
  <Slot />
</Card>
"#,
        )
        .expect("ui component should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home

<SectionCard title="Imported from UI">
  <Copy>Silver contract</Copy>
</SectionCard>
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = DevServerState {
            root: root.clone(),
            preview_store: Mutex::new(AxPreviewStore::default()),
        };
        let html =
            render_route_html(&state, &route).expect("package component route should render");

        assert!(html.contains("Imported from UI"));
        assert!(html.contains("Silver contract"));
        assert!(!html.contains("data-import-source"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn renders_page_with_imported_axonyx_ui_component_from_src_foundry_layout() {
        let workspace = make_temp_dir("ui-package-src-foundry");
        let root = workspace.join("axonyx-site");
        let ui_root = workspace.join("axonyx-ui");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(ui_root.join("src/foundry")).expect("ui foundry dir should exist");
        fs::write(
            ui_root.join("src/foundry/SectionCard.ax"),
            r#"
page SectionCard
<Card title={title}>
  <Slot />
</Card>
"#,
        )
        .expect("ui component should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home

<SectionCard title="Imported from src/foundry">
  <Copy>Modern package layout</Copy>
</SectionCard>
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = DevServerState {
            root: root.clone(),
            preview_store: Mutex::new(AxPreviewStore::default()),
        };
        let html =
            render_route_html(&state, &route).expect("src/foundry package route should render");

        assert!(html.contains("Imported from src/foundry"));
        assert!(html.contains("Modern package layout"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn renders_page_with_imported_axonyx_ui_component_from_cargo_dependency() {
        let workspace = make_temp_dir("ui-package-cargo-dependency");
        let root = workspace.join("axonyx-site");
        let ui_root = workspace.join("axonyx-ui");
        let ui_path = ui_root.to_string_lossy().replace('\\', "\\\\");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(ui_root.join("src/foundry")).expect("ui foundry dir should exist");
        fs::write(
            root.join("Cargo.toml"),
            format!(
                r#"
[package]
name = "axonyx-site"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-ui = {{ path = "{ui_path}" }}
"#
            ),
        )
        .expect("app cargo manifest should write");
        fs::write(
            ui_root.join("Cargo.toml"),
            r#"
[package]
name = "axonyx-ui"
version = "0.0.0"
edition = "2021"

[lib]
path = "src/lib.rs"
"#,
        )
        .expect("ui cargo manifest should write");
        fs::write(ui_root.join("src/lib.rs"), "").expect("ui lib should write");
        fs::write(
            ui_root.join("Axonyx.package.toml"),
            r#"
[package]
name = "axonyx-ui"
namespace = "@axonyx/ui"

[exports]
ax_root = "src"
css_root = "src/css"
css_entry = "src/css/index.css"
"#,
        )
        .expect("ui package metadata should write");
        fs::write(
            ui_root.join("src/foundry/SectionCard.ax"),
            r#"
page SectionCard
<Card title={title}>
  <Slot />
</Card>
"#,
        )
        .expect("ui component should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home

<SectionCard title="Imported through Cargo">
  <Copy>No package override needed</Copy>
</SectionCard>
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = DevServerState {
            root: root.clone(),
            preview_store: Mutex::new(AxPreviewStore::default()),
        };
        let html =
            render_route_html(&state, &route).expect("cargo package component route should render");

        assert!(html.contains("Imported through Cargo"));
        assert!(html.contains("No package override needed"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn vendored_axonyx_ui_component_wins_over_cargo_dependency() {
        let workspace = make_temp_dir("ui-package-vendor-before-cargo");
        let root = workspace.join("axonyx-site");
        let cargo_ui_root = workspace.join("axonyx-ui");
        let vendor_ui_root = root.join("vendor/axonyx-ui");
        let cargo_ui_path = cargo_ui_root.to_string_lossy().replace('\\', "\\\\");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(cargo_ui_root.join("src/foundry")).expect("cargo ui dir should exist");
        fs::create_dir_all(vendor_ui_root.join("src/foundry")).expect("vendor ui dir should exist");
        fs::write(
            root.join("Cargo.toml"),
            format!(
                r#"
[package]
name = "axonyx-site"
version = "0.1.0"
edition = "2021"

[dependencies]
axonyx-ui = {{ path = "{cargo_ui_path}" }}
"#
            ),
        )
        .expect("app cargo manifest should write");
        fs::write(
            cargo_ui_root.join("Cargo.toml"),
            r#"
[package]
name = "axonyx-ui"
version = "0.0.0"
edition = "2021"

[lib]
path = "src/lib.rs"
"#,
        )
        .expect("ui cargo manifest should write");
        fs::write(cargo_ui_root.join("src/lib.rs"), "").expect("ui lib should write");
        fs::write(
            cargo_ui_root.join("Axonyx.package.toml"),
            r#"
[package]
name = "axonyx-ui"
namespace = "@axonyx/ui"

[exports]
ax_root = "src"
"#,
        )
        .expect("ui package metadata should write");
        fs::write(
            cargo_ui_root.join("src/foundry/SectionCard.ax"),
            r#"
page SectionCard
<Card title="Cargo package" />
"#,
        )
        .expect("cargo ui component should write");
        fs::write(
            vendor_ui_root.join("src/foundry/SectionCard.ax"),
            r#"
page SectionCard
<Card title="Vendored package" />
"#,
        )
        .expect("vendor ui component should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home
<SectionCard />
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let state = DevServerState {
            root: root.clone(),
            preview_store: Mutex::new(AxPreviewStore::default()),
        };
        let html = render_route_html(&state, &route).expect("vendored package route should render");

        assert!(html.contains("Vendored package"));
        assert!(!html.contains("Cargo package"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn route_version_changes_when_imported_axonyx_ui_component_changes() {
        let workspace = make_temp_dir("ui-package-version");
        let root = workspace.join("axonyx-site");
        let ui_root = workspace.join("axonyx-ui");

        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(ui_root.join("src/ax/foundry")).expect("ui ax dir should exist");
        fs::write(
            ui_root.join("src/ax/foundry/SectionCard.ax"),
            r#"
page SectionCard
<Card title="Version A" />
"#,
        )
        .expect("ui component should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home
<SectionCard />
"#,
        )
        .expect("page should write");

        let route = resolve_route(&root, "/")
            .expect("route resolution should work")
            .expect("route should exist");
        let before = route_version(&root, &route).expect("initial version should hash");

        fs::write(
            ui_root.join("src/ax/foundry/SectionCard.ax"),
            r#"
page SectionCard
<Card title="Version B" />
"#,
        )
        .expect("ui component should update");

        let after = route_version(&root, &route).expect("updated version should hash");
        assert_ne!(before, after);

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn check_ax_source_reports_page_parse_error_line() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/app/page.ax");
        let diagnostics = check_ax_source_with_root(&path, "page Home\n<Copy></Card>\n", None);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].code, "axonyx-parse");
    }

    #[test]
    fn check_ax_source_reports_reserved_import_name() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/app/page.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
import { Link } from "@axonyx/ui/foundry/Link.ax"

page Home
<Link href="/">Home</Link>
"#,
            None,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[0].code, "axonyx-parse");
        assert!(diagnostics[0].message.contains("reserved"));
        assert!(diagnostics[0].message.contains("Link"));
    }

    #[test]
    fn check_ax_source_reports_invalid_type_annotation() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/app/page.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
page Blog

let posts: List<Post>> = load PostsList

<Copy>Body</Copy>
"#,
            None,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 4);
        assert_eq!(diagnostics[0].code, "axonyx-type");
    }

    #[test]
    fn check_ax_source_reports_unknown_typed_each_member() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/app/page.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
page Blog

type Post {
  title: String
}

let posts: List<Post> = load PostsList

<Each items={posts} as="post">
  <Card title={post.summary} />
</Each>
"#,
            None,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "axonyx-type");
        assert_eq!(diagnostics[0].line, 11);
        assert!(diagnostics[0].message.contains("post.summary"));
        assert!(diagnostics[0].message.contains("summary"));
        assert!(diagnostics[0].message.contains("unknown field"));
    }

    #[test]
    fn check_ax_source_allows_optional_typed_each_member() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/app/page.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
page Blog

type Post {
  title: String
}

let posts: List<Post> = load PostsList

<Each items={posts} as="post">
  <Card title={post?.summary} />
</Each>
"#,
            None,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn check_ax_source_allows_optional_type_field() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/app/page.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
page Blog

type Post {
  title: String
  summary?: String
}

let posts: List<Post> = load PostsList

<Each items={posts} as="post">
  <Card title={post.summary} />
</Each>
"#,
            None,
        );

        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    }

    #[test]
    fn check_ax_source_reports_duplicate_type_field() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/app/page.ax");
        let diagnostics = check_ax_source_with_root(
            &path,
            r#"
page Blog

type Post {
  title: String
  title: String
}

<Copy>Body</Copy>
"#,
            None,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "axonyx-type");
        assert!(diagnostics[0].message.contains("duplicate field"));
        assert!(diagnostics[0].message.contains("title"));
    }

    #[test]
    fn check_ax_source_reports_backend_parse_error_line() {
        let path = PathBuf::from("H:/CODE/axonyx/demo/routes/api/posts.ax");
        let diagnostics =
            check_ax_source_with_root(&path, "route GET \"/api/posts\"\n    return posts\n", None);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].code, "axonyx-backend-parse");
    }

    #[test]
    fn check_ax_source_reports_missing_app_import() {
        let root = make_temp_dir("check-missing-app-import");
        let page_path = root.join("app/page.ax");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");

        let diagnostics = check_ax_source_with_root(
            &page_path,
            r#"
import { SiteCard } from "@/components/SiteCard.ax"

page Home
<SiteCard />
"#,
            Some(&root),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].code, "axonyx-import");
        assert!(diagnostics[0].message.contains("@/components/SiteCard.ax"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_ax_source_accepts_existing_app_import() {
        let root = make_temp_dir("check-existing-app-import");
        let page_path = root.join("app/page.ax");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/components/SiteCard.ax"),
            "page SiteCard\n<Card />\n",
        )
        .expect("component should write");

        let diagnostics = check_ax_source_with_root(
            &page_path,
            r#"
import { SiteCard } from "@/components/SiteCard.ax"

page Home
<SiteCard />
"#,
            Some(&root),
        );

        assert!(diagnostics.is_empty());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn component_override_resolves_exact_import_source() {
        let root = make_temp_dir("component-override");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[component_overrides]
"@axonyx/ui/foundry/SectionCard.ax" = "@/components/SiteCard.ax"
"#,
        )
        .expect("config should write");

        let resolved =
            resolve_preview_import_path(root.as_path(), "@axonyx/ui/foundry/SectionCard.ax")
                .expect("override should resolve");

        assert_eq!(resolved, root.join("app/components/SiteCard.ax"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn package_override_resolves_package_namespace_import() {
        let root = make_temp_dir("package-override");
        fs::create_dir_all(root.join("vendor/custom-ui/src/ax/foundry"))
            .expect("custom ui dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[package_overrides]
"@axonyx/ui" = "./vendor/custom-ui"
"#,
        )
        .expect("config should write");

        let resolved =
            resolve_preview_import_path(root.as_path(), "@axonyx/ui/foundry/SectionCard.ax")
                .expect("package override should resolve");

        assert_eq!(
            resolved,
            root.join("vendor/custom-ui/src/ax/foundry/SectionCard.ax")
        );

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn package_override_resolves_package_namespace_import_from_src_layout() {
        let root = make_temp_dir("package-override-src-layout");
        fs::create_dir_all(root.join("vendor/custom-ui/src/foundry"))
            .expect("custom ui dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[package_overrides]
"@axonyx/ui" = "./vendor/custom-ui"
"#,
        )
        .expect("config should write");

        let resolved =
            resolve_preview_import_path(root.as_path(), "@axonyx/ui/foundry/SectionCard.ax")
                .expect("package override should resolve");

        assert_eq!(
            resolved,
            root.join("vendor/custom-ui/src/foundry/SectionCard.ax")
        );

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_ax_source_accepts_component_override_import() {
        let root = make_temp_dir("check-component-override");
        let page_path = root.join("app/page.ax");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[component_overrides]
"@axonyx/ui/foundry/SectionCard.ax" = "@/components/SiteCard.ax"
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("app/components/SiteCard.ax"),
            "page SectionCard\n<Card />\n",
        )
        .expect("override component should write");

        let diagnostics = check_ax_source_with_root(
            &page_path,
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home
<SectionCard />
"#,
            Some(&root),
        );

        assert!(diagnostics.is_empty());

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_ax_source_reports_invalid_component_override_target() {
        let root = make_temp_dir("check-invalid-component-override");
        let page_path = root.join("app/page.ax");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[component_overrides]
"@axonyx/ui/foundry/SectionCard.ax" = "@/components/SiteCard.ax"
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("app/components/SiteCard.ax"),
            "page SectionCard\n<Copy></Card>\n",
        )
        .expect("override component should write");

        let diagnostics = check_ax_source_with_root(
            &page_path,
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home
<SectionCard />
"#,
            Some(&root),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].code, "axonyx-import-parse");
        assert!(diagnostics[0].message.contains("SiteCard.ax"));
        assert!(diagnostics[0].message.contains("line 2"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_ax_source_reports_invalid_package_override_target() {
        let root = make_temp_dir("check-invalid-package-override");
        let page_path = root.join("app/page.ax");
        fs::create_dir_all(root.join("vendor/custom-ui/src/ax/foundry"))
            .expect("custom ui dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[package_overrides]
"@axonyx/ui" = "./vendor/custom-ui"
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("vendor/custom-ui/src/ax/foundry/SectionCard.ax"),
            "page SectionCard\n<Copy></Card>\n",
        )
        .expect("override component should write");

        let diagnostics = check_ax_source_with_root(
            &page_path,
            r#"
import { SectionCard } from "@axonyx/ui/foundry/SectionCard.ax"

page Home
<SectionCard />
"#,
            Some(&root),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].code, "axonyx-import-parse");
        assert!(diagnostics[0]
            .message
            .contains("vendor/custom-ui/src/ax/foundry/SectionCard.ax"));
        assert!(diagnostics[0].message.contains("line 2"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_ax_source_reports_nested_missing_import_chain() {
        let root = make_temp_dir("check-nested-missing-import-chain");
        let page_path = root.join("app/page.ax");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/components/SiteCard.ax"),
            r#"
import { InnerCard } from "@/components/InnerCard.ax"

page SiteCard
<InnerCard />
"#,
        )
        .expect("component should write");

        let diagnostics = check_ax_source_with_root(
            &page_path,
            r#"
import { SiteCard } from "@/components/SiteCard.ax"

page Home
<SiteCard />
"#,
            Some(&root),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].code, "axonyx-import-chain");
        assert!(diagnostics[0].message.contains("SiteCard.ax"));
        assert!(diagnostics[0].message.contains("InnerCard.ax"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn check_ax_source_reports_import_cycle() {
        let root = make_temp_dir("check-import-cycle");
        let page_path = root.join("app/page.ax");
        fs::create_dir_all(root.join("app/components")).expect("components dir should exist");
        fs::write(root.join("Axonyx.toml"), "[app]\nname = \"demo\"\n")
            .expect("config should write");
        fs::write(
            root.join("app/components/SiteCard.ax"),
            r#"
import { InnerCard } from "@/components/InnerCard.ax"

page SiteCard
<InnerCard />
"#,
        )
        .expect("site card should write");
        fs::write(
            root.join("app/components/InnerCard.ax"),
            r#"
import { SiteCard } from "@/components/SiteCard.ax"

page InnerCard
<SiteCard />
"#,
        )
        .expect("inner card should write");

        let diagnostics = check_ax_source_with_root(
            &page_path,
            r#"
import { SiteCard } from "@/components/SiteCard.ax"

page Home
<SiteCard />
"#,
            Some(&root),
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, 2);
        assert_eq!(diagnostics[0].code, "axonyx-import-cycle");
        assert!(diagnostics[0].message.contains("SiteCard.ax"));
        assert!(diagnostics[0].message.contains("InnerCard.ax"));
        assert!(diagnostics[0].message.contains("cycle"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn content_manifest_indexes_configured_collections() {
        let root = make_temp_dir("content-manifest");
        fs::create_dir_all(root.join("content/docs/nested")).expect("content dirs should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[content.collections.docs]
path = "content/docs"
extensions = ["md", "mdx"]
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("content/docs/intro.md"),
            "---\ntitle: Intro\ndescription: Start here\n---\n# Intro\n",
        )
        .expect("intro should write");
        fs::write(root.join("content/docs/nested/setup.mdx"), "# Setup\n")
            .expect("setup should write");
        fs::write(root.join("content/docs/ignored.txt"), "skip").expect("ignored should write");

        let manifest = collect_content_manifest(&root).expect("manifest should collect");

        assert_eq!(manifest.collections.len(), 1);
        let collection = &manifest.collections[0];
        assert_eq!(collection.name, "docs");
        assert_eq!(collection.path, "content/docs");
        assert_eq!(collection.extensions, vec!["md", "mdx"]);
        assert_eq!(collection.entries.len(), 2);
        assert_eq!(collection.entries[0].path, "content/docs/intro.md");
        assert_eq!(collection.entries[0].slug, "intro");
        assert_eq!(collection.entries[0].content_type, "markdown");
        assert_eq!(collection.entries[0].title, "Intro");
        assert_eq!(collection.entries[0].excerpt, "Start here");
        assert_eq!(collection.entries[0].word_count, 1);
        assert_eq!(
            collection.entries[0].frontmatter.get("title"),
            Some(&"Intro".to_string())
        );
        assert_eq!(collection.entries[0].body, "# Intro\n");
        assert_eq!(collection.entries[1].path, "content/docs/nested/setup.mdx");
        assert_eq!(collection.entries[1].slug, "nested/setup");
        assert_eq!(collection.entries[1].title, "Setup");
        assert_eq!(collection.entries[1].excerpt, "Setup");

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn preview_store_loads_configured_content_collections() {
        let root = make_temp_dir("content-preview-store");
        fs::create_dir_all(root.join("content/docs")).expect("content dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[content.collections.docs]
path = "content/docs"
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("content/docs/getting-started.md"),
            "---\ntitle: Getting Started\ndescription: Build your first page\n---\n# Start\n",
        )
        .expect("doc should write");

        let store = preview_store_from_content(&root).expect("preview store should load content");
        let items = store.collection_items("docs");

        assert_eq!(items.len(), 1);
        let AxValue::Record(fields) = &items[0] else {
            panic!("expected content item record");
        };
        assert_eq!(fields.get("slug"), Some(&AxValue::from("getting-started")));
        assert_eq!(fields.get("extension"), Some(&AxValue::from("md")));
        assert_eq!(fields.get("content_type"), Some(&AxValue::from("markdown")));
        assert_eq!(fields.get("title"), Some(&AxValue::from("Getting Started")));
        assert_eq!(
            fields.get("excerpt"),
            Some(&AxValue::from("Build your first page"))
        );
        assert_eq!(fields.get("word_count"), Some(&AxValue::from(1i64)));
        assert_eq!(
            fields.get("description"),
            Some(&AxValue::from("Build your first page"))
        );
        assert_eq!(fields.get("body"), Some(&AxValue::from("# Start\n")));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn build_static_site_writes_content_manifest() {
        let root = make_temp_dir("static-build-content-manifest");
        fs::create_dir_all(root.join("app")).expect("app dir should exist");
        fs::create_dir_all(root.join("content/docs")).expect("content dir should exist");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[content.collections.docs]
path = "content/docs"
extensions = ["md"]
"#,
        )
        .expect("config should write");
        fs::write(
            root.join("app/page.ax"),
            r#"
page Home
<Copy>Home page</Copy>
"#,
        )
        .expect("page should write");
        fs::write(root.join("content/docs/intro.md"), "# Intro\n").expect("intro should write");

        let status = build_static_site_from_app_root(&root, Path::new("dist"), true)
            .expect("static build works");

        match status {
            StaticBuildStatus::Generated {
                content_collection_count,
                ..
            } => assert_eq!(content_collection_count, 1),
            StaticBuildStatus::NoPages { .. } => panic!("static pages should be found"),
        }

        let manifest = fs::read_to_string(root.join("dist/_ax/content/manifest.json"))
            .expect("content manifest should exist");
        assert!(manifest.contains("\"name\": \"docs\""));
        assert!(manifest.contains("\"path\": \"content/docs/intro.md\""));
        assert!(manifest.contains("\"slug\": \"intro\""));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn content_collection_path_must_stay_inside_app_root() {
        let root = make_temp_dir("content-path-safety");
        fs::write(
            root.join("Axonyx.toml"),
            r#"
[app]
name = "demo"

[content.collections.docs]
path = "../outside"
"#,
        )
        .expect("config should write");

        let error = load_content_collection_configs(&root).expect_err("path should fail");
        assert!(error.to_string().contains("must stay inside app root"));

        fs::remove_dir_all(root).expect("temp dir should clean up");
    }

    #[test]
    fn schema_inference_generates_ax_type_from_json_list() {
        let value = serde_json::json!([
            {
                "title": "Hello",
                "slug": "hello",
                "summary": null
            },
            {
                "title": "Second",
                "slug": "second"
            }
        ]);

        let schema = infer_schema_from_json("Post", &value).expect("schema should infer");
        let ax = render_schema_as_ax(&schema);

        assert!(ax.contains("type Post {"));
        assert!(ax.contains("  title: String"));
        assert!(ax.contains("  slug: String"));
        assert!(ax.contains("  summary?: Unknown"));
        assert!(ax.contains("// root: List<Post>"));
    }

    #[test]
    fn schema_inference_marks_missing_list_fields_optional() {
        let value = serde_json::json!([
            {
                "title": "Hello",
                "views": 3
            },
            {
                "title": "Second",
                "summary": "Short"
            }
        ]);

        let schema = infer_schema_from_json("Post", &value).expect("schema should infer");
        let ax = render_schema_as_ax(&schema);

        assert!(ax.contains("  summary?: String"));
        assert!(ax.contains("  views?: Number"));
    }

    #[test]
    fn schema_pull_prefers_typed_envelope_schema() {
        let value = serde_json::json!({
            "type": "List<Post>",
            "schemaHash": "sha256:test",
            "schema": {
                "Post": {
                    "title": "String",
                    "slug": "String",
                    "summary": "Optional<String>"
                }
            },
            "data": [
                {
                    "title": "Hello",
                    "slug": "hello",
                    "summary": null
                }
            ]
        });

        let schema = schema_from_typed_envelope("Post", &value)
            .expect("schema envelope should parse")
            .expect("schema envelope should be detected");
        let ax = render_schema_as_ax(&schema);

        assert!(ax.contains("type Post {"));
        assert!(ax.contains("  title: String"));
        assert!(ax.contains("  slug: String"));
        assert!(ax.contains("  summary?: String"));
        assert!(ax.contains("// root: List<Post>"));
    }

    #[test]
    fn schema_pull_accepts_object_field_descriptors() {
        let value = serde_json::json!({
            "schema": {
                "Post": {
                    "title": { "type": "String" },
                    "summary": { "type": "String", "optional": true }
                }
            }
        });

        let schema = schema_from_typed_envelope("Post", &value)
            .expect("schema envelope should parse")
            .expect("schema envelope should be detected");
        let ax = render_schema_as_ax(&schema);

        assert!(ax.contains("  title: String"));
        assert!(ax.contains("  summary?: String"));
        assert!(ax.contains("// root: Post"));
    }

    #[test]
    fn schema_pull_can_read_inline_json() {
        let source = read_schema_source(r#"[{"title":"Hello"}]"#).expect("inline JSON works");

        assert_eq!(source, r#"[{"title":"Hello"}]"#);
    }
}

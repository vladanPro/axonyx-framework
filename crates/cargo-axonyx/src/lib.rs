use std::ffi::OsString;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use axonyx_core::ax_backend_codegen_prelude::compile_backend_sources_to_module;
use axonyx_runtime::{
    execute_preview_action_sources, execute_preview_route_sources,
    preview_ax_route_with_request_context, AxPreviewHttpResponse, AxPreviewStore,
};
use clap::{Parser, Subcommand, ValueEnum};

const DOCS_LAYOUT_AX: &str = include_str!("../templates/docs/app/docs/layout.ax.tpl");
const DOCS_HOME_AX: &str = include_str!("../templates/docs/app/docs/page.ax.tpl");
const DOCS_GETTING_STARTED_AX: &str =
    include_str!("../templates/docs/app/docs/getting-started/page.ax.tpl");
const DOCS_REFERENCE_AX: &str = include_str!("../templates/docs/app/docs/reference/page.ax.tpl");
const DOCS_EXAMPLES_AX: &str = include_str!("../templates/docs/app/docs/examples/page.ax.tpl");

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
    Dev(DevArgs),
    Run(RunArgs),
}

#[derive(Debug, Parser)]
struct AddArgs {
    #[arg(value_enum)]
    module: ModuleKind,
}

#[derive(Debug, Parser, Default)]
struct BuildArgs {}

#[derive(Debug, Parser)]
struct DevArgs {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 3000)]
    port: u16,
}

#[derive(Debug, Parser)]
struct RunArgs {
    #[command(subcommand)]
    command: RunCommands,
}

#[derive(Debug, Subcommand)]
enum RunCommands {
    Dev(DevArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ModuleKind {
    Docs,
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

struct DevServerState {
    root: PathBuf,
    preview_store: Mutex<AxPreviewStore>,
}

struct HttpRequest {
    method: String,
    target: String,
    headers: std::collections::BTreeMap<String, String>,
    body: Vec<u8>,
}

enum BackendBuildStatus {
    Generated {
        source_count: usize,
        output_path: PathBuf,
    },
    NoSources {
        output_path: PathBuf,
    },
}

pub fn main_entry() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse_from(normalized_cli_args());

    match cli.command {
        Commands::Add(args) => add_module(args.module),
        Commands::Build(args) => build_command(args),
        Commands::Dev(args) => run_dev_server(args),
        Commands::Run(args) => run_command(args),
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

fn build_command(_args: BuildArgs) -> Result<()> {
    let root = app_root()?;
    let status = compile_backend_from_app_root(&root)?;
    print_backend_build_status(&status);
    Ok(())
}

fn run_command(args: RunArgs) -> Result<()> {
    match args.command {
        RunCommands::Dev(args) => run_dev_server(args),
    }
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
    }

    Ok(())
}

fn run_dev_server(args: DevArgs) -> Result<()> {
    let root = app_root()?;
    let backend_status = compile_backend_from_app_root(&root)?;

    let bind = format!("{}:{}", args.host, args.port);
    let listener =
        TcpListener::bind(&bind).with_context(|| format!("failed to bind dev server at {bind}"))?;
    let shared_state = Arc::new(DevServerState {
        root,
        preview_store: Mutex::new(AxPreviewStore::default()),
    });

    print_backend_build_status(&backend_status);
    println!("Axonyx dev server listening at http://{bind}");
    println!(
        "Routes come from app/**/page.ax with nested layouts, route-local loader.ax, actions.ax POST handling, and routes/**/*.ax API endpoints."
    );
    println!("Press Ctrl+C to stop.");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(error) = handle_connection(stream, &shared_state) {
                    eprintln!("dev server error: {error:#}");
                }
            }
            Err(error) => eprintln!("dev server connection error: {error}"),
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
    let source = fs::read_to_string(axonyx_toml)
        .with_context(|| format!("failed to read '{}'", axonyx_toml.display()))?;
    let mut value = source
        .parse::<toml::Value>()
        .with_context(|| format!("failed to parse '{}'", axonyx_toml.display()))?;

    let root_table = value
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("Axonyx.toml root must be a TOML table"))?;

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

    let rendered = toml::to_string_pretty(&value).context("failed to render Axonyx.toml")?;
    fs::write(axonyx_toml, rendered)
        .with_context(|| format!("failed to write '{}'", axonyx_toml.display()))?;
    Ok(())
}

fn handle_connection(mut stream: TcpStream, state: &DevServerState) -> Result<()> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .context("failed to set read timeout")?;

    let Some(request) = read_http_request(&mut stream)? else {
        return Ok(());
    };

    if request.method == "GET" {
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

    if request.method == "GET" && request.target.starts_with("/__axonyx/version") {
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

        let version = route_version(&route)?;
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

    let html = render_route_html(state, &route)?;
    let html = inject_dev_client(&html, &route.request_path);
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

    preview_ax_route_with_request_context(
        &layout_refs,
        &loader_refs,
        &action_refs,
        &page_source,
        &route.request_target,
        &route.params,
        &store,
    )
    .with_context(|| {
        format!(
            "failed to render route '{}' from '{}'",
            route.request_path,
            route.page_path.display()
        )
    })
}

fn route_version(route: &ResolvedRoute) -> Result<String> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    route.request_path.hash(&mut hasher);

    hash_file(&route.page_path, &mut hasher)?;
    for path in &route.layout_paths {
        hash_file(path, &mut hasher)?;
    }
    if let Some(path) = &route.loader_path {
        hash_file(path, &mut hasher)?;
    }
    if let Some(path) = &route.actions_path {
        hash_file(path, &mut hasher)?;
    }

    Ok(format!("{:x}", hasher.finish()))
}

fn hash_file(path: &Path, hasher: &mut impl Hasher) -> Result<()> {
    path.to_string_lossy().hash(hasher);
    let contents = fs::read(path)
        .with_context(|| format!("failed to read '{}' for hashing", path.display()))?;
    contents.hash(hasher);
    Ok(())
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
}

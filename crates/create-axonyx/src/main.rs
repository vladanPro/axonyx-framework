mod template;

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use axonyx_core::ax_backend_codegen_prelude::compile_backend_sources_to_module;
use clap::{Parser, ValueEnum};

const DEFAULT_RUNTIME_GIT_URL: &str = "https://github.com/vladanPro/axonyx-runtime";
const DEFAULT_RUNTIME_PACKAGE: &str = "axonyx-runtime";
const DEFAULT_RUNTIME_VERSION: &str = "0.1.0";
const DEFAULT_UI_GIT_URL: &str = "https://github.com/vladanPro/axonyx-ui";

#[derive(Debug, Parser)]
#[command(name = "create-axonyx")]
#[command(about = "Create a new Axonyx app", version)]
struct Cli {
    /// Name of the app directory to create
    project_name: String,

    /// Skip prompts and accept defaults
    #[arg(long)]
    yes: bool,

    /// Overwrite target directory if it exists
    #[arg(long)]
    force: bool,

    /// Initialize a git repository in the generated app
    #[arg(long)]
    git: bool,

    /// Starter template to use for the generated app
    #[arg(long, value_enum, default_value_t = AppTemplate::Minimal)]
    template: AppTemplate,

    /// Where the generated app should load axonyx-runtime from (default: git for public use)
    #[arg(long, value_enum, default_value_t = RuntimeSource::Git)]
    runtime_source: RuntimeSource,

    /// Git URL used when --runtime-source git is selected
    #[arg(long, default_value = DEFAULT_RUNTIME_GIT_URL)]
    runtime_git_url: String,

    /// Package name used when --runtime-source registry is selected
    #[arg(long, default_value = DEFAULT_RUNTIME_PACKAGE)]
    runtime_package: String,

    /// Package version used when --runtime-source registry is selected
    #[arg(long, default_value = DEFAULT_RUNTIME_VERSION)]
    runtime_version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum RuntimeSource {
    Path,
    Git,
    Registry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum AppTemplate {
    Minimal,
    Site,
    Docs,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    validate_project_name(&cli.project_name)?;

    let target_dir = std::env::current_dir()
        .context("unable to resolve current directory")?
        .join(&cli.project_name);

    if target_dir.exists() && !cli.force {
        bail!(
            "target directory '{}' already exists (use --force to overwrite)",
            target_dir.display()
        );
    }

    if target_dir.exists() && cli.force {
        fs::remove_dir_all(&target_dir).with_context(|| {
            format!(
                "failed to remove existing directory '{}'",
                target_dir.display()
            )
        })?;
    }

    if !cli.yes {
        println!(
            "This will create a new Axonyx app in '{}'.",
            target_dir.display()
        );
        if !confirm("Continue? [y/N]: ")? {
            println!("Canceled.");
            return Ok(());
        }
    }

    create_app(&target_dir, &cli)?;

    if cli.git {
        init_git(&target_dir)?;
    }

    println!();
    println!("Success! Axonyx app created at {}", target_dir.display());
    println!("Next steps:");
    println!("  cd {}", cli.project_name);
    println!("  cargo ax check");
    println!("  cargo ax build --clean");
    println!("  cargo ax run dev");
    println!("Template: {:?}", cli.template);
    Ok(())
}

fn validate_project_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        bail!("project name cannot be empty");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("project name can contain only letters, digits, '-' and '_'");
    }
    Ok(())
}

fn create_app(target_dir: &PathBuf, cli: &Cli) -> Result<()> {
    let runtime_dependency = runtime_dependency_spec(cli)?;
    let runtime_source_note = runtime_source_note(cli);
    let template = match cli.template {
        AppTemplate::Minimal => template::AppTemplate::Minimal,
        AppTemplate::Site => template::AppTemplate::Site,
        AppTemplate::Docs => template::AppTemplate::Docs,
    };

    fs::create_dir_all(target_dir).with_context(|| {
        format!(
            "failed to create project directory '{}'",
            target_dir.display()
        )
    })?;

    for file in template::template_files(
        template,
        &cli.project_name,
        &runtime_dependency,
        &runtime_source_note,
    ) {
        let full_path = target_dir.join(file.relative_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory '{}'", parent.display()))?;
        }
        fs::write(&full_path, file.contents)
            .with_context(|| format!("failed to write '{}'", full_path.display()))?;
    }

    if matches!(
        template,
        template::AppTemplate::Site | template::AppTemplate::Docs
    ) {
        install_template_ui(target_dir)?;
    }

    compile_initial_backend(target_dir)?;

    Ok(())
}

fn install_template_ui(target_dir: &Path) -> Result<()> {
    let vendor_root = target_dir.join("vendor").join("axonyx-ui");
    ensure_ui_vendor(&vendor_root)?;
    sync_ui_css_snapshot(&vendor_root, target_dir)?;
    Ok(())
}

fn compile_initial_backend(target_dir: &PathBuf) -> Result<()> {
    let mut sources = Vec::new();

    collect_backend_sources(target_dir, &mut sources)?;
    if sources.is_empty() {
        return Ok(());
    }

    let source_refs = sources
        .iter()
        .map(|(name, source)| (name.as_str(), source.as_str()))
        .collect::<Vec<_>>();

    let module = compile_backend_sources_to_module(&source_refs)
        .with_context(|| "failed to compile initial backend .ax sources")?;

    let generated_backend = target_dir.join("src").join("generated").join("backend.rs");
    fs::write(&generated_backend, module).with_context(|| {
        format!(
            "failed to write generated backend module '{}'",
            generated_backend.display()
        )
    })?;

    Ok(())
}

fn collect_backend_sources(target_dir: &PathBuf, out: &mut Vec<(String, String)>) -> Result<()> {
    let routes_root = target_dir.join("routes");
    let jobs_root = target_dir.join("jobs");
    let app_root = target_dir.join("app");

    collect_backend_sources_in_dir(&routes_root, &routes_root, out, true)?;
    collect_backend_sources_in_dir(&jobs_root, &jobs_root, out, true)?;
    collect_named_backend_files(&app_root, &app_root, out, &["loader.ax", "actions.ax"])?;
    Ok(())
}

fn collect_backend_sources_in_dir(
    root: &PathBuf,
    dir: &PathBuf,
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
    root: &PathBuf,
    dir: &PathBuf,
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

fn init_git(target_dir: &PathBuf) -> Result<()> {
    let status = std::process::Command::new("git")
        .arg("init")
        .current_dir(target_dir)
        .status()
        .context("failed to execute git init")?;

    if !status.success() {
        bail!("git init failed with status {status}");
    }

    Ok(())
}

fn runtime_dependency_spec(cli: &Cli) -> Result<String> {
    match cli.runtime_source {
        RuntimeSource::Path => {
            let runtime_crate = runtime_workspace_root()?
                .join("crates")
                .join("axonyx-runtime")
                .canonicalize()
                .context("failed to resolve axonyx-runtime crate path")?;

            let runtime_path = cargo_toml_path(&runtime_crate);
            Ok(format!("axonyx-runtime = {{ path = \"{runtime_path}\" }}"))
        }
        RuntimeSource::Git => Ok(format!(
            "axonyx-runtime = {{ git = \"{}\" }}",
            cli.runtime_git_url.trim()
        )),
        RuntimeSource::Registry => Ok(format!(
            "{} = \"{}\"",
            cli.runtime_package.trim(),
            cli.runtime_version.trim()
        )),
    }
}

fn runtime_source_note(cli: &Cli) -> String {
    match cli.runtime_source {
        RuntimeSource::Path => "This scaffold links against the local `axonyx-runtime` workspace path. Use this mode when contributing to Axonyx itself, typically through the `vendor/axonyx-runtime` git submodule in the framework repo.".to_string(),
        RuntimeSource::Git => format!(
            "This scaffold links against the shared `axonyx-runtime` Git repository at `{}`. This is the recommended public setup until the first crates.io release is available.",
            cli.runtime_git_url.trim()
        ),
        RuntimeSource::Registry => format!(
            "This scaffold is prepared for the crates.io flow and expects the runtime package `{}` at version `{}` to be available in the Cargo registry.",
            cli.runtime_package.trim(),
            cli.runtime_version.trim()
        ),
    }
}

fn ensure_ui_vendor(vendor_root: &Path) -> Result<()> {
    if vendor_root.exists() {
        return Ok(());
    }

    if let Some(source_root) = resolve_local_ui_source() {
        copy_dir_all_filtered(&source_root, vendor_root, |path| {
            path.file_name().is_some_and(|name| name == ".git")
        })?;
        return Ok(());
    }

    if let Some(parent) = vendor_root.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create '{}'", parent.display()))?;
    }

    let status = std::process::Command::new("git")
        .args(["clone", "--depth", "1", DEFAULT_UI_GIT_URL])
        .arg(vendor_root)
        .status()
        .context("failed to launch git while vendoring axonyx-ui")?;

    if !status.success() {
        bail!(
            "failed to clone axonyx-ui from '{}' into '{}'",
            DEFAULT_UI_GIT_URL,
            vendor_root.display()
        );
    }

    let git_dir = vendor_root.join(".git");
    if git_dir.exists() {
        fs::remove_dir_all(&git_dir)
            .with_context(|| format!("failed to clean '{}'", git_dir.display()))?;
    }

    Ok(())
}

fn resolve_local_ui_source() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("AXONYX_UI_SOURCE") {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let workspace = workspace_root();
    let mut candidates = vec![
        workspace.join("vendor").join("axonyx-ui"),
        workspace.parent().map_or_else(
            || PathBuf::from("axonyx-ui"),
            |parent| parent.join("axonyx-ui"),
        ),
    ];

    if let Some(parent) = workspace.parent() {
        candidates.push(parent.join("axonyx-ui"));
    }

    candidates.into_iter().find(|candidate| candidate.exists())
}

fn sync_ui_css_snapshot(vendor_root: &Path, app_root: &Path) -> Result<()> {
    let css_source = vendor_root.join("src").join("css");
    if !css_source.exists() {
        bail!(
            "vendored axonyx-ui did not contain '{}'",
            css_source.display()
        );
    }

    let css_target = app_root.join("public").join("css").join("axonyx-ui");
    copy_dir_all_filtered(&css_source, &css_target, |_| false)?;
    Ok(())
}

fn copy_dir_all_filtered(
    source: &Path,
    target: &Path,
    skip: impl Fn(&Path) -> bool + Copy,
) -> Result<()> {
    if skip(source) {
        return Ok(());
    }

    let metadata = fs::metadata(source)
        .with_context(|| format!("failed to inspect source '{}'", source.display()))?;

    if metadata.is_file() {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create '{}'", parent.display()))?;
        }
        fs::copy(source, target).with_context(|| {
            format!(
                "failed to copy source file '{}' to '{}'",
                source.display(),
                target.display()
            )
        })?;
        return Ok(());
    }

    fs::create_dir_all(target)
        .with_context(|| format!("failed to create '{}'", target.display()))?;

    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read directory '{}'", source.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in '{}'", source.display()))?;
        let from = entry.path();
        if skip(&from) {
            continue;
        }

        let to = target.join(entry.file_name());
        copy_dir_all_filtered(&from, &to, skip)?;
    }

    Ok(())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("create-axonyx should live under <workspace>/crates/create-axonyx")
        .to_path_buf()
}

fn runtime_workspace_root() -> Result<PathBuf> {
    let workspace = workspace_root();
    let submodule = workspace.join("vendor").join("axonyx-runtime");
    if submodule.exists() {
        return Ok(submodule);
    }

    let sibling = workspace.parent().map_or_else(
        || PathBuf::from("axonyx-runtime"),
        |parent| parent.join("axonyx-runtime"),
    );
    if sibling.exists() {
        return Ok(sibling);
    }

    bail!(
        "could not find axonyx-runtime workspace; expected '{}' or '{}'",
        submodule.display(),
        sibling.display()
    );
}

fn cargo_toml_path(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    normalized
        .strip_prefix("//?/")
        .unwrap_or(&normalized)
        .to_string()
}

fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt}");
    io::stdout().flush().context("failed to flush stdout")?;

    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .context("failed to read confirmation input")?;

    let normalized = line.trim().to_ascii_lowercase();
    Ok(normalized == "y" || normalized == "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_temp_dir(label: &str) -> PathBuf {
        let unique = format!(
            "axonyx-create-{}-{}",
            label,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should move forward")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        fs::create_dir_all(&dir).expect("temp dir should create");
        dir
    }

    #[test]
    fn site_template_enables_ui_module() {
        let files = template::template_files(
            template::AppTemplate::Site,
            "demo-site",
            "axonyx-runtime = { git = \"https://example.com/runtime\" }",
            "runtime note",
        );

        let axonyx_toml = files
            .iter()
            .find(|file| file.relative_path == "Axonyx.toml")
            .expect("Axonyx.toml should exist");

        assert!(axonyx_toml.contents.contains("enabled = [\"ui\"]"));
    }

    #[test]
    fn create_site_template_vendors_ui_and_scaffolds_foundry_layout() {
        let workspace = make_temp_dir("site-template");
        let target_dir = workspace.join("demo-site");
        let ui_root = workspace.join("local-ui");

        fs::create_dir_all(ui_root.join("src/ax/foundry")).expect("ui ax dir should exist");
        fs::create_dir_all(ui_root.join("src/css")).expect("ui css dir should exist");
        fs::write(ui_root.join("README.md"), "# Axonyx UI\n").expect("ui readme should write");
        fs::write(
            ui_root.join("src/ax/foundry/SectionCard.ax"),
            "page SectionCard\n  Card title: title\n    Slot\n",
        )
        .expect("ui component should write");
        fs::write(
            ui_root.join("src/css/index.css"),
            "@import './tokens.css';\n",
        )
        .expect("ui index css should write");
        fs::write(
            ui_root.join("src/css/tokens.css"),
            ":root { --ax-text: #fff; }\n",
        )
        .expect("ui tokens css should write");

        let previous_ui_source = std::env::var_os("AXONYX_UI_SOURCE");
        std::env::set_var("AXONYX_UI_SOURCE", &ui_root);

        let cli = Cli {
            project_name: "demo-site".to_string(),
            yes: true,
            force: false,
            git: false,
            template: AppTemplate::Site,
            runtime_source: RuntimeSource::Git,
            runtime_git_url: DEFAULT_RUNTIME_GIT_URL.to_string(),
            runtime_package: DEFAULT_RUNTIME_PACKAGE.to_string(),
            runtime_version: DEFAULT_RUNTIME_VERSION.to_string(),
        };

        let result = create_app(&target_dir, &cli);

        if let Some(value) = previous_ui_source {
            std::env::set_var("AXONYX_UI_SOURCE", value);
        } else {
            std::env::remove_var("AXONYX_UI_SOURCE");
        }

        result.expect("site template should scaffold");

        assert!(target_dir
            .join("vendor/axonyx-ui/src/ax/foundry/SectionCard.ax")
            .exists());
        assert!(target_dir.join("public/css/axonyx-ui/index.css").exists());

        let layout =
            fs::read_to_string(target_dir.join("app/layout.ax")).expect("layout should read");
        assert!(layout.contains("@axonyx/ui/foundry/SiteShell.ax"));
        assert!(layout.contains("<Theme>silver</Theme>"));
        assert!(layout.contains("/css/axonyx-ui/index.css"));

        let page = fs::read_to_string(target_dir.join("app/page.ax")).expect("page should read");
        assert!(page.contains("@axonyx/ui/foundry/SectionCard.ax"));

        let axonyx_toml =
            fs::read_to_string(target_dir.join("Axonyx.toml")).expect("config should read");
        assert!(axonyx_toml.contains("enabled = [\"ui\"]"));
        assert!(axonyx_toml.contains("[package_overrides]"));
        assert!(axonyx_toml.contains("\"@axonyx/ui\" = \"./vendor/axonyx-ui\""));

        let posts_page = fs::read_to_string(target_dir.join("app/posts/page.ax"))
            .expect("posts page should read");
        assert!(posts_page.contains("<If when={load PostsList}>"));
        assert!(posts_page.contains("<Each items={load PostsList} as=\"post\">"));
        assert!(posts_page.contains("<Else>"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }
}

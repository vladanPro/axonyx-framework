mod template;

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use axonix_core::ax_backend_codegen_prelude::compile_backend_sources_to_module;
use clap::{Parser, ValueEnum};

const DEFAULT_RUNTIME_GIT_URL: &str = "https://github.com/vladanPro/axonix-runtime";
const DEFAULT_RUNTIME_PACKAGE: &str = "axonix-runtime";
const DEFAULT_RUNTIME_VERSION: &str = "0.1.0";

#[derive(Debug, Parser)]
#[command(name = "create-axonix")]
#[command(about = "Create a new Axonix app", version)]
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

    /// Where the generated app should load axonix-runtime from
    #[arg(long, value_enum, default_value_t = RuntimeSource::Path)]
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
            "This will create a new Axonix app in '{}'.",
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
    println!("Success! Axonix app created at {}", target_dir.display());
    println!("Next steps:");
    println!("  cd {}", cli.project_name);
    println!("  cargo run");
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

    compile_initial_backend(target_dir)?;

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
            let runtime_crate = workspace_root()
                .join("crates")
                .join("axonix-runtime")
                .canonicalize()
                .context("failed to resolve axonix-runtime crate path")?;

            let runtime_path = cargo_toml_path(&runtime_crate);
            Ok(format!("axonix-runtime = {{ path = \"{runtime_path}\" }}"))
        }
        RuntimeSource::Git => Ok(format!(
            "axonix-runtime = {{ git = \"{}\" }}",
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
        RuntimeSource::Path => "This scaffold links against a local `axonix-runtime` path dependency so monorepo development stays fast while the framework is evolving.".to_string(),
        RuntimeSource::Git => format!(
            "This scaffold links against the shared `axonix-runtime` Git repository at `{}` so the app can track the standalone runtime workspace outside the local monorepo.",
            cli.runtime_git_url.trim()
        ),
        RuntimeSource::Registry => format!(
            "This scaffold is prepared for the crates.io flow and expects the runtime package `{}` at version `{}` to be available in the Cargo registry.",
            cli.runtime_package.trim(),
            cli.runtime_version.trim()
        ),
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("create-axonix should live under <workspace>/crates/create-axonix")
        .to_path_buf()
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

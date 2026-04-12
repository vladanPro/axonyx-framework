mod template;

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use axonix_core::ax_backend_codegen_prelude::compile_backend_sources_to_module;
use clap::Parser;

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
        println!("This will create a new Axonix app in '{}'.", target_dir.display());
        if !confirm("Continue? [y/N]: ")? {
            println!("Canceled.");
            return Ok(());
        }
    }

    create_app(&target_dir, &cli.project_name)?;

    if cli.git {
        init_git(&target_dir)?;
    }

    println!();
    println!("Success! Axonix app created at {}", target_dir.display());
    println!("Next steps:");
    println!("  cd {}", cli.project_name);
    println!("  cargo run");
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

fn create_app(target_dir: &PathBuf, project_name: &str) -> Result<()> {
    fs::create_dir_all(target_dir).with_context(|| {
        format!(
            "failed to create project directory '{}'",
            target_dir.display()
        )
    })?;

    for file in template::minimal_template_files(project_name) {
        let full_path = target_dir.join(file.relative_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create directory '{}'", parent.display())
            })?;
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
    let module = module.replace(
        "use axonix_runtime::backend_prelude::*;",
        "use crate::runtime::backend_prelude::*;",
    );

    let generated_backend = target_dir.join("src").join("generated").join("backend.rs");
    fs::write(&generated_backend, module).with_context(|| {
        format!(
            "failed to write generated backend module '{}'",
            generated_backend.display()
        )
    })?;

    Ok(())
}

fn collect_backend_sources(
    target_dir: &PathBuf,
    out: &mut Vec<(String, String)>,
) -> Result<()> {
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
        let entry = entry.with_context(|| format!("failed to read entry in '{}'", dir.display()))?;
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
        let entry = entry.with_context(|| format!("failed to read entry in '{}'", dir.display()))?;
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

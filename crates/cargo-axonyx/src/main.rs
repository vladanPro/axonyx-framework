use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

const DOCS_LAYOUT_AX: &str = include_str!("../templates/docs/app/docs/layout.ax.tpl");
const DOCS_HOME_AX: &str = include_str!("../templates/docs/app/docs/page.ax.tpl");
const DOCS_GETTING_STARTED_AX: &str =
    include_str!("../templates/docs/app/docs/getting-started/page.ax.tpl");
const DOCS_REFERENCE_AX: &str = include_str!("../templates/docs/app/docs/reference/page.ax.tpl");
const DOCS_EXAMPLES_AX: &str = include_str!("../templates/docs/app/docs/examples/page.ax.tpl");

#[derive(Debug, Parser)]
#[command(name = "cargo-axonyx")]
#[command(bin_name = "cargo-axonyx")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Add(AddArgs),
}

#[derive(Debug, Parser)]
struct AddArgs {
    #[arg(value_enum)]
    module: ModuleKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ModuleKind {
    Docs,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Add(args) => add_module(args.module),
    }
}

fn add_module(module: ModuleKind) -> Result<()> {
    let root = std::env::current_dir().context("unable to resolve current directory")?;
    let axonyx_toml = root.join("Axonyx.toml");

    if !axonyx_toml.exists() {
        bail!(
            "Axonyx.toml was not found in '{}'; run this command from an Axonyx app root",
            root.display()
        );
    }

    match module {
        ModuleKind::Docs => {
            scaffold_docs_module(&root)?;
            enable_module(&axonyx_toml, "docs")?;
            println!("Added docs module.");
        }
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

mod template;

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use axonyx_core::ax_backend_codegen_prelude::compile_backend_sources_to_module;
use clap::{Parser, ValueEnum};

const DEFAULT_RUNTIME_GIT_URL: &str = "https://github.com/vladanPro/axonyx-runtime";
const DEFAULT_RUNTIME_PACKAGE: &str = "axonyx-runtime";
const DEFAULT_RUNTIME_VERSION: &str = "0.1.6";
const DEFAULT_UI_PACKAGE: &str = "axonyx-ui";
const DEFAULT_UI_VERSION: &str = "0.0.33";

#[derive(Debug, Parser)]
#[command(name = "create-axonyx")]
#[command(about = "Create a new Axonyx app", version)]
struct Cli {
    /// Name or path of the app directory to create
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

    /// Where the generated app should load axonyx-runtime from
    #[arg(long, value_enum, default_value_t = RuntimeSource::Registry)]
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
        print_create_error(&err);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let target_dir = resolve_target_dir(&cli.project_name)?;
    let project_name = project_name_from_target(&target_dir)?;
    validate_project_name(&project_name)?;

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

    create_app(&target_dir, &project_name, &cli)?;

    if cli.git {
        init_git(&target_dir)?;
    }

    println!();
    println!("Success! Axonyx app created at {}", target_dir.display());
    println!("Next steps:");
    println!("  cd {}", shell_path_arg(&target_dir));
    println!("  cargo ax check");
    println!("  cargo ax doctor");
    println!("  cargo ax build --clean");
    println!("  cargo ax run dev");
    println!("Template: {:?}", cli.template);
    Ok(())
}

fn shell_path_arg(path: &Path) -> String {
    let display = path.display().to_string();
    if display.chars().any(char::is_whitespace) {
        format!("\"{display}\"")
    } else {
        display
    }
}

fn resolve_target_dir(input: &str) -> Result<PathBuf> {
    if input.trim().is_empty() {
        bail!("project path cannot be empty");
    }

    let path = PathBuf::from(input);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()
            .context("unable to resolve current directory")?
            .join(path))
    }
}

fn project_name_from_target(target_dir: &Path) -> Result<String> {
    let Some(name) = target_dir.file_name().and_then(|name| name.to_str()) else {
        bail!("project path must end with a folder name");
    };

    Ok(name.to_string())
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

fn print_create_error(error: &anyhow::Error) {
    eprintln!("Axonyx could not create this app.");
    eprintln!();
    eprintln!("Problem:");
    eprintln!("  {}", error);

    if let Some(hint) = hint_for_create_error(error) {
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
            eprintln!("  - {detail}");
        }
    }
}

fn hint_for_create_error(error: &anyhow::Error) -> Option<&'static str> {
    let combined = error
        .chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    if combined.contains("project name") {
        return Some("Use a path ending in a simple folder name such as `my-site`, `docs`, or `hello-axonyx`.");
    }

    if combined.contains("target directory") && combined.contains("already exists") {
        return Some("Choose a new project name or pass `--force` if you intentionally want to replace the folder.");
    }

    if combined.contains("could not find axonyx-runtime workspace") {
        return Some("Use `--runtime-source git`, or initialize the framework submodule with `git submodule update --init --recursive`.");
    }

    if combined.contains("failed to clone axonyx-ui")
        || combined.contains("failed to launch git while vendoring axonyx-ui")
    {
        return Some("Set AXONYX_UI_SOURCE to a local axonyx-ui checkout, or check that Git can access the axonyx-ui repository.");
    }

    if combined.contains("failed to compile initial backend .ax sources") {
        return Some("The template backend sources did not compile; this is likely a framework bug. Run the core smoke script to reproduce it.");
    }

    None
}

fn create_app(target_dir: &PathBuf, project_name: &str, cli: &Cli) -> Result<()> {
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
        project_name,
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
    ensure_ui_cargo_dependency(target_dir)?;
    Ok(())
}

fn ensure_ui_cargo_dependency(target_dir: &Path) -> Result<()> {
    let cargo_toml = target_dir.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Ok(());
    }

    let source = fs::read_to_string(&cargo_toml)
        .with_context(|| format!("failed to read '{}'", cargo_toml.display()))?;
    if source.contains("[dependencies.axonyx-ui]") || source.contains("\naxonyx-ui =") {
        return Ok(());
    }

    let mut updated = source;
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&format!(
        "\n{} = \"{}\"\n",
        DEFAULT_UI_PACKAGE, DEFAULT_UI_VERSION
    ));

    fs::write(&cargo_toml, updated)
        .with_context(|| format!("failed to write '{}'", cargo_toml.display()))?;
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
            "This scaffold links against the shared `axonyx-runtime` Git repository at `{}`. Use this mode when testing an unreleased runtime branch.",
            cli.runtime_git_url.trim()
        ),
        RuntimeSource::Registry => format!(
            "This scaffold uses the published crates.io runtime package `{}` at version `{}`.",
            cli.runtime_package.trim(),
            cli.runtime_version.trim()
        ),
    }
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
    fn create_error_hint_detects_existing_target_directory() {
        let error = anyhow::anyhow!(
            "target directory 'demo-site' already exists (use --force to overwrite)"
        );

        assert_eq!(
            hint_for_create_error(&error),
            Some("Choose a new project name or pass `--force` if you intentionally want to replace the folder.")
        );
    }

    #[test]
    fn target_path_uses_last_component_as_project_name() {
        let workspace = make_temp_dir("target-path");
        let target_dir = workspace.join("nested").join("demo-site");

        assert_eq!(
            project_name_from_target(&target_dir).expect("project name should resolve"),
            "demo-site"
        );
        validate_project_name(
            &project_name_from_target(&target_dir).expect("project name should resolve"),
        )
        .expect("last path component should validate");

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }

    #[test]
    fn shell_path_arg_quotes_paths_with_spaces() {
        assert_eq!(
            shell_path_arg(Path::new(r"C:\Temp\Axonyx Sites\demo-site")),
            r#""C:\Temp\Axonyx Sites\demo-site""#
        );
        assert_eq!(
            shell_path_arg(Path::new(r"C:\Temp\demo-site")),
            r"C:\Temp\demo-site"
        );
    }

    #[test]
    fn create_error_hint_detects_missing_local_runtime() {
        let error = anyhow::anyhow!("could not find axonyx-runtime workspace");

        assert_eq!(
            hint_for_create_error(&error),
            Some("Use `--runtime-source git`, or initialize the framework submodule with `git submodule update --init --recursive`.")
        );
    }

    #[test]
    fn create_site_template_adds_registry_ui_and_scaffolds_foundry_layout() {
        let workspace = make_temp_dir("site-template");
        let target_dir = workspace.join("demo-site");

        let cli = Cli {
            project_name: "demo-site".to_string(),
            yes: true,
            force: false,
            git: false,
            template: AppTemplate::Site,
            runtime_source: RuntimeSource::Registry,
            runtime_git_url: DEFAULT_RUNTIME_GIT_URL.to_string(),
            runtime_package: DEFAULT_RUNTIME_PACKAGE.to_string(),
            runtime_version: DEFAULT_RUNTIME_VERSION.to_string(),
        };

        create_app(&target_dir, "demo-site", &cli).expect("site template should scaffold");

        assert!(!target_dir.join("vendor/axonyx-ui").exists());
        assert!(!target_dir.join("public/css/axonyx-ui").exists());

        let layout =
            fs::read_to_string(target_dir.join("app/layout.ax")).expect("layout should read");
        assert!(layout.contains("@axonyx/ui/foundry/SiteShell.ax"));
        assert!(layout.contains("<Theme>silver</Theme>"));
        assert!(layout.contains("/_ax/pkg/axonyx-ui/index.css"));

        let cargo_toml =
            fs::read_to_string(target_dir.join("Cargo.toml")).expect("cargo manifest should read");
        assert!(cargo_toml.contains("axonyx-runtime = \"0.1.6\""));
        assert!(cargo_toml.contains("axonyx-ui = \"0.0.33\""));

        let page = fs::read_to_string(target_dir.join("app/page.ax")).expect("page should read");
        assert!(page.contains("@axonyx/ui/foundry/SectionCard.ax"));

        let axonyx_toml =
            fs::read_to_string(target_dir.join("Axonyx.toml")).expect("config should read");
        assert!(axonyx_toml.contains("enabled = [\"ui\"]"));
        assert!(!axonyx_toml.contains("[package_overrides]"));

        let posts_page = fs::read_to_string(target_dir.join("app/posts/page.ax"))
            .expect("posts page should read");
        assert!(posts_page.contains("<If when={load PostsList}>"));
        assert!(posts_page.contains("<Each items={load PostsList} as=\"post\">"));
        assert!(posts_page.contains("<Else>"));

        fs::remove_dir_all(workspace).expect("temp dir should clean up");
    }
}

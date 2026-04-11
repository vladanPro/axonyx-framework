pub struct TemplateFile {
    pub relative_path: &'static str,
    pub contents: String,
}

const APP_CARGO_TOML: &str = include_str!("../templates/minimal/Cargo.toml.tpl");
const APP_MAIN_RS: &str = include_str!("../templates/minimal/src/main.rs.tpl");
const APP_AXONIX_TOML: &str = include_str!("../templates/minimal/Axonix.toml.tpl");
const APP_PIPELINE_AX: &str = include_str!("../templates/minimal/app/pipeline.ax.tpl");
const APP_README: &str = include_str!("../templates/minimal/README.md.tpl");
const APP_GITIGNORE: &str = include_str!("../templates/minimal/.gitignore.tpl");

pub fn minimal_template_files(project_name: &str) -> Vec<TemplateFile> {
    let vars = [("{{APP_NAME}}", project_name)];
    vec![
        TemplateFile {
            relative_path: "Cargo.toml",
            contents: apply_vars(APP_CARGO_TOML, &vars),
        },
        TemplateFile {
            relative_path: "src/main.rs",
            contents: apply_vars(APP_MAIN_RS, &vars),
        },
        TemplateFile {
            relative_path: "Axonix.toml",
            contents: apply_vars(APP_AXONIX_TOML, &vars),
        },
        TemplateFile {
            relative_path: "app/pipeline.ax",
            contents: apply_vars(APP_PIPELINE_AX, &vars),
        },
        TemplateFile {
            relative_path: "README.md",
            contents: apply_vars(APP_README, &vars),
        },
        TemplateFile {
            relative_path: ".gitignore",
            contents: apply_vars(APP_GITIGNORE, &vars),
        },
    ]
}

fn apply_vars(source: &str, vars: &[(&str, &str)]) -> String {
    vars.iter()
        .fold(source.to_string(), |acc, (k, v)| acc.replace(k, v))
}


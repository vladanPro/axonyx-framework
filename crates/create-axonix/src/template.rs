pub struct TemplateFile {
    pub relative_path: &'static str,
    pub contents: String,
}

const APP_CARGO_TOML: &str = include_str!("../templates/minimal/Cargo.toml.tpl");
const APP_MAIN_RS: &str = include_str!("../templates/minimal/src/main.rs.tpl");
const APP_GENERATED_MOD_RS: &str = include_str!("../templates/minimal/src/generated/mod.rs.tpl");
const APP_GENERATED_BACKEND_RS: &str =
    include_str!("../templates/minimal/src/generated/backend.rs.tpl");
const APP_DOMAIN_POSTS_RS: &str = include_str!("../templates/minimal/src/domain/posts.rs.tpl");
const APP_DB_MOD_RS: &str = include_str!("../templates/minimal/src/db/mod.rs.tpl");
const APP_AXONIX_TOML: &str = include_str!("../templates/minimal/Axonix.toml.tpl");
const APP_LAYOUT_AX: &str = include_str!("../templates/minimal/app/layout.ax.tpl");
const APP_PAGE_AX: &str = include_str!("../templates/minimal/app/page.ax.tpl");
const APP_POSTS_PAGE_AX: &str = include_str!("../templates/minimal/app/posts/page.ax.tpl");
const APP_POSTS_LOADER_AX: &str = include_str!("../templates/minimal/app/posts/loader.ax.tpl");
const APP_POSTS_ACTIONS_AX: &str = include_str!("../templates/minimal/app/posts/actions.ax.tpl");
const APP_ROUTE_POSTS_AX: &str = include_str!("../templates/minimal/routes/api/posts.ax.tpl");
const APP_JOB_DIGEST_AX: &str = include_str!("../templates/minimal/jobs/digest.ax.tpl");
const APP_README: &str = include_str!("../templates/minimal/README.md.tpl");
const APP_GITIGNORE: &str = include_str!("../templates/minimal/.gitignore.tpl");
const APP_ENV_EXAMPLE: &str = include_str!("../templates/minimal/.env.example.tpl");

pub fn minimal_template_files(project_name: &str, runtime_dependency: &str) -> Vec<TemplateFile> {
    let vars = [
        ("{{APP_NAME}}", project_name),
        ("{{AXONIX_RUNTIME_DEPENDENCY}}", runtime_dependency),
    ];
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
            relative_path: "src/generated/mod.rs",
            contents: apply_vars(APP_GENERATED_MOD_RS, &vars),
        },
        TemplateFile {
            relative_path: "src/generated/backend.rs",
            contents: apply_vars(APP_GENERATED_BACKEND_RS, &vars),
        },
        TemplateFile {
            relative_path: "src/domain/posts.rs",
            contents: apply_vars(APP_DOMAIN_POSTS_RS, &vars),
        },
        TemplateFile {
            relative_path: "src/db/mod.rs",
            contents: apply_vars(APP_DB_MOD_RS, &vars),
        },
        TemplateFile {
            relative_path: "Axonix.toml",
            contents: apply_vars(APP_AXONIX_TOML, &vars),
        },
        TemplateFile {
            relative_path: "app/layout.ax",
            contents: apply_vars(APP_LAYOUT_AX, &vars),
        },
        TemplateFile {
            relative_path: "app/page.ax",
            contents: apply_vars(APP_PAGE_AX, &vars),
        },
        TemplateFile {
            relative_path: "app/posts/page.ax",
            contents: apply_vars(APP_POSTS_PAGE_AX, &vars),
        },
        TemplateFile {
            relative_path: "app/posts/loader.ax",
            contents: apply_vars(APP_POSTS_LOADER_AX, &vars),
        },
        TemplateFile {
            relative_path: "app/posts/actions.ax",
            contents: apply_vars(APP_POSTS_ACTIONS_AX, &vars),
        },
        TemplateFile {
            relative_path: "routes/api/posts.ax",
            contents: apply_vars(APP_ROUTE_POSTS_AX, &vars),
        },
        TemplateFile {
            relative_path: "jobs/digest.ax",
            contents: apply_vars(APP_JOB_DIGEST_AX, &vars),
        },
        TemplateFile {
            relative_path: "README.md",
            contents: apply_vars(APP_README, &vars),
        },
        TemplateFile {
            relative_path: ".env.example",
            contents: apply_vars(APP_ENV_EXAMPLE, &vars),
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

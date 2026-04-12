#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTemplate {
    Minimal,
    Site,
}

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

const SITE_APP_LAYOUT_AX: &str = include_str!("../templates/site/app/layout.ax.tpl");
const SITE_APP_PAGE_AX: &str = include_str!("../templates/site/app/page.ax.tpl");
const SITE_APP_POSTS_PAGE_AX: &str = include_str!("../templates/site/app/posts/page.ax.tpl");
const SITE_APP_POSTS_LOADER_AX: &str = include_str!("../templates/site/app/posts/loader.ax.tpl");
const SITE_APP_POSTS_ACTIONS_AX: &str = include_str!("../templates/site/app/posts/actions.ax.tpl");
const SITE_APP_ROUTE_POSTS_AX: &str = include_str!("../templates/site/routes/api/posts.ax.tpl");
const SITE_APP_JOB_DIGEST_AX: &str = include_str!("../templates/site/jobs/digest.ax.tpl");
const SITE_APP_README: &str = include_str!("../templates/site/README.md.tpl");

pub fn template_files(
    template: AppTemplate,
    project_name: &str,
    runtime_dependency: &str,
    runtime_source_note: &str,
) -> Vec<TemplateFile> {
    let vars = [
        ("{{APP_NAME}}", project_name),
        ("{{AXONIX_RUNTIME_DEPENDENCY}}", runtime_dependency),
        ("{{AXONIX_RUNTIME_SOURCE_NOTE}}", runtime_source_note),
    ];

    let (
        readme,
        layout_ax,
        page_ax,
        posts_page_ax,
        posts_loader_ax,
        posts_actions_ax,
        route_posts_ax,
        job_digest_ax,
    ) = match template {
        AppTemplate::Minimal => (
            APP_README,
            APP_LAYOUT_AX,
            APP_PAGE_AX,
            APP_POSTS_PAGE_AX,
            APP_POSTS_LOADER_AX,
            APP_POSTS_ACTIONS_AX,
            APP_ROUTE_POSTS_AX,
            APP_JOB_DIGEST_AX,
        ),
        AppTemplate::Site => (
            SITE_APP_README,
            SITE_APP_LAYOUT_AX,
            SITE_APP_PAGE_AX,
            SITE_APP_POSTS_PAGE_AX,
            SITE_APP_POSTS_LOADER_AX,
            SITE_APP_POSTS_ACTIONS_AX,
            SITE_APP_ROUTE_POSTS_AX,
            SITE_APP_JOB_DIGEST_AX,
        ),
    };

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
            contents: apply_vars(layout_ax, &vars),
        },
        TemplateFile {
            relative_path: "app/page.ax",
            contents: apply_vars(page_ax, &vars),
        },
        TemplateFile {
            relative_path: "app/posts/page.ax",
            contents: apply_vars(posts_page_ax, &vars),
        },
        TemplateFile {
            relative_path: "app/posts/loader.ax",
            contents: apply_vars(posts_loader_ax, &vars),
        },
        TemplateFile {
            relative_path: "app/posts/actions.ax",
            contents: apply_vars(posts_actions_ax, &vars),
        },
        TemplateFile {
            relative_path: "routes/api/posts.ax",
            contents: apply_vars(route_posts_ax, &vars),
        },
        TemplateFile {
            relative_path: "jobs/digest.ax",
            contents: apply_vars(job_digest_ax, &vars),
        },
        TemplateFile {
            relative_path: "README.md",
            contents: apply_vars(readme, &vars),
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

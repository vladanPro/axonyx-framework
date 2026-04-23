#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTemplate {
    Minimal,
    Site,
    Docs,
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
const APP_AXONYX_TOML: &str = include_str!("../templates/minimal/Axonyx.toml.tpl");
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
const APP_PUBLIC_FAVICON_SVG: &str = include_str!("../templates/minimal/public/favicon.svg.tpl");

const SITE_APP_LAYOUT_AX: &str = include_str!("../templates/site/app/layout.ax.tpl");
const SITE_APP_PAGE_AX: &str = include_str!("../templates/site/app/page.ax.tpl");
const SITE_APP_POSTS_PAGE_AX: &str = include_str!("../templates/site/app/posts/page.ax.tpl");
const SITE_APP_POSTS_LOADER_AX: &str = include_str!("../templates/site/app/posts/loader.ax.tpl");
const SITE_APP_POSTS_ACTIONS_AX: &str = include_str!("../templates/site/app/posts/actions.ax.tpl");
const SITE_APP_ROUTE_POSTS_AX: &str = include_str!("../templates/site/routes/api/posts.ax.tpl");
const SITE_APP_JOB_DIGEST_AX: &str = include_str!("../templates/site/jobs/digest.ax.tpl");
const SITE_APP_README: &str = include_str!("../templates/site/README.md.tpl");
const SITE_PUBLIC_FAVICON_SVG: &str = include_str!("../templates/site/public/favicon.svg.tpl");
const SITE_PUBLIC_BRAND_MARK_SVG: &str =
    include_str!("../templates/site/public/brand-mark.svg.tpl");

const DOCS_APP_LAYOUT_AX: &str = include_str!("../templates/docs/app/layout.ax.tpl");
const DOCS_APP_PAGE_AX: &str = include_str!("../templates/docs/app/page.ax.tpl");
const DOCS_APP_GETTING_STARTED_AX: &str =
    include_str!("../templates/docs/app/getting-started/page.ax.tpl");
const DOCS_APP_REFERENCE_AX: &str = include_str!("../templates/docs/app/reference/page.ax.tpl");
const DOCS_APP_EXAMPLES_AX: &str = include_str!("../templates/docs/app/examples/page.ax.tpl");
const DOCS_APP_README: &str = include_str!("../templates/docs/README.md.tpl");
const DOCS_PUBLIC_FAVICON_SVG: &str = include_str!("../templates/docs/public/favicon.svg.tpl");
const DOCS_PUBLIC_BRAND_MARK_SVG: &str =
    include_str!("../templates/docs/public/brand-mark.svg.tpl");

pub fn template_files(
    template: AppTemplate,
    project_name: &str,
    runtime_dependency: &str,
    runtime_source_note: &str,
) -> Vec<TemplateFile> {
    let vars = [
        ("{{APP_NAME}}", project_name),
        ("{{AXONYX_RUNTIME_DEPENDENCY}}", runtime_dependency),
        ("{{AXONYX_RUNTIME_SOURCE_NOTE}}", runtime_source_note),
    ];

    let axonyx_toml = match template {
        AppTemplate::Minimal => APP_AXONYX_TOML.to_string(),
        AppTemplate::Site | AppTemplate::Docs => ui_ready_axonyx_toml(),
    };

    let mut files = vec![
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
            relative_path: "Axonyx.toml",
            contents: apply_vars(&axonyx_toml, &vars),
        },
        TemplateFile {
            relative_path: ".env.example",
            contents: apply_vars(APP_ENV_EXAMPLE, &vars),
        },
        TemplateFile {
            relative_path: ".gitignore",
            contents: apply_vars(APP_GITIGNORE, &vars),
        },
    ];

    match template {
        AppTemplate::Minimal => {
            files.extend([
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
                    relative_path: "public/favicon.svg",
                    contents: apply_vars(APP_PUBLIC_FAVICON_SVG, &vars),
                },
                TemplateFile {
                    relative_path: "README.md",
                    contents: apply_vars(APP_README, &vars),
                },
            ]);
        }
        AppTemplate::Site => {
            files.extend([
                TemplateFile {
                    relative_path: "app/layout.ax",
                    contents: apply_vars(SITE_APP_LAYOUT_AX, &vars),
                },
                TemplateFile {
                    relative_path: "app/page.ax",
                    contents: apply_vars(SITE_APP_PAGE_AX, &vars),
                },
                TemplateFile {
                    relative_path: "app/posts/page.ax",
                    contents: apply_vars(SITE_APP_POSTS_PAGE_AX, &vars),
                },
                TemplateFile {
                    relative_path: "app/posts/loader.ax",
                    contents: apply_vars(SITE_APP_POSTS_LOADER_AX, &vars),
                },
                TemplateFile {
                    relative_path: "app/posts/actions.ax",
                    contents: apply_vars(SITE_APP_POSTS_ACTIONS_AX, &vars),
                },
                TemplateFile {
                    relative_path: "routes/api/posts.ax",
                    contents: apply_vars(SITE_APP_ROUTE_POSTS_AX, &vars),
                },
                TemplateFile {
                    relative_path: "jobs/digest.ax",
                    contents: apply_vars(SITE_APP_JOB_DIGEST_AX, &vars),
                },
                TemplateFile {
                    relative_path: "public/favicon.svg",
                    contents: apply_vars(SITE_PUBLIC_FAVICON_SVG, &vars),
                },
                TemplateFile {
                    relative_path: "public/brand-mark.svg",
                    contents: apply_vars(SITE_PUBLIC_BRAND_MARK_SVG, &vars),
                },
                TemplateFile {
                    relative_path: "README.md",
                    contents: apply_vars(SITE_APP_README, &vars),
                },
            ]);
        }
        AppTemplate::Docs => {
            files.extend([
                TemplateFile {
                    relative_path: "app/layout.ax",
                    contents: apply_vars(DOCS_APP_LAYOUT_AX, &vars),
                },
                TemplateFile {
                    relative_path: "app/page.ax",
                    contents: apply_vars(DOCS_APP_PAGE_AX, &vars),
                },
                TemplateFile {
                    relative_path: "app/getting-started/page.ax",
                    contents: apply_vars(DOCS_APP_GETTING_STARTED_AX, &vars),
                },
                TemplateFile {
                    relative_path: "app/reference/page.ax",
                    contents: apply_vars(DOCS_APP_REFERENCE_AX, &vars),
                },
                TemplateFile {
                    relative_path: "app/examples/page.ax",
                    contents: apply_vars(DOCS_APP_EXAMPLES_AX, &vars),
                },
                TemplateFile {
                    relative_path: "public/favicon.svg",
                    contents: apply_vars(DOCS_PUBLIC_FAVICON_SVG, &vars),
                },
                TemplateFile {
                    relative_path: "public/brand-mark.svg",
                    contents: apply_vars(DOCS_PUBLIC_BRAND_MARK_SVG, &vars),
                },
                TemplateFile {
                    relative_path: "README.md",
                    contents: apply_vars(DOCS_APP_README, &vars),
                },
            ]);
        }
    }

    files
}

fn apply_vars(source: &str, vars: &[(&str, &str)]) -> String {
    vars.iter()
        .fold(source.to_string(), |acc, (k, v)| acc.replace(k, v))
}

fn ui_ready_axonyx_toml() -> String {
    let mut source = APP_AXONYX_TOML.replace("enabled = []", "enabled = [\"ui\"]");
    source.push_str(
        r#"

[package_overrides]
"@axonyx/ui" = "./vendor/axonyx-ui"
"#,
    );
    source
}

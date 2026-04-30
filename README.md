# Axonyx Framework Monorepo

Axonyx is a Rust-first framework focused on:

- Single-binary architecture
- Algebraic UI pipelines (`|>`)
- Fast data-to-UI execution flow

This repository includes:

- `create-axonyx`: project scaffolding CLI (similar to `create-next-app`)
- `cargo-axonyx`: project helper CLI (`cargo ax ...`) for local authoring flows
- integration with the `vendor/axonyx-runtime` git submodule for `axonyx-core`, `axonyx-runtime`, and `axonyx-macros`

The public framework name, crate names, commands, and app config file now use `Axonyx` / `axonyx-*`.
Repository URLs and local workspace folders are now aligned to `axonyx-*`.

## Package Model

Current package roles inside this repo:

- `create-axonyx`: CLI that scaffolds a new Axonyx app
- `cargo-axonyx`: local CLI for `add` and `run dev`
- runtime crates are imported from the `vendor/axonyx-runtime` submodule

Generated apps can now target either:

- a local Cargo `path` dependency into the checked out runtime workspace
- the standalone Git repo at `https://github.com/vladanPro/axonyx-runtime`
- a future crates.io package release such as `axonyx-runtime = "0.1.0"`

Current local flow:

```bash
git submodule update --init --recursive
```

## Quick Start

### 1) Create a new Axonyx app locally

```bash
cargo run -p create-axonyx -- my-app --yes
```

The default runtime source is now `git`, so generated apps work without needing the framework repo or its submodule layout.

Available templates today:

- `minimal`
- `site`
- `docs`

Recommended authoring path today:

- JSX-like `.ax` files in `app/**/page.ax` and `app/**/layout.ax`
- nested app routes plus route-local `loader.ax` and `actions.ax`
- imports from local app components via `@/components/...`
- imports from Axonyx UI packages via `@axonyx/ui/...`

Legacy indentation-first `.ax` syntax still exists for compatibility and reference work,
but new examples and new framework authoring should prefer the JSX-like `.ax` direction.

Example:

```bash
cargo run -p create-axonyx -- my-site --yes --template site
```

```bash
cargo run -p create-axonyx -- my-docs --yes --template docs
```

### 1a) Add a docs module into an existing Axonyx app

From an app root:

```bash
cargo run --manifest-path H:/CODE/axonyx/axonyx-framework/Cargo.toml -p cargo-axonyx --bin cargo-ax -- add docs
```

This first proof-of-concept adds an `app/docs/...` route tree and enables the module in `Axonyx.toml`.

### 1b) Build generated backend output from `.ax` sources

From an app root:

```bash
cargo ax build
```

This scans:

- `app/**/loader.ax`
- `app/**/actions.ax`
- `routes/**/*.ax`
- `jobs/**/*.ax`

and regenerates:

```text
src/generated/backend.rs
```

### 1c) Create a new Axonyx app against the standalone runtime repo

```bash
cargo run -p create-axonyx -- my-app --yes
```

Use `--runtime-source path` only when contributing to Axonyx itself from the framework workspace.

### 1d) Create a new Axonyx app against the future registry release

```bash
cargo run -p create-axonyx -- my-app --yes --runtime-source registry
```

Use the registry mode once `axonyx-runtime` is published. Until then, prefer `path` or `git`.

### 2) Run the generated app

```bash
cd my-app
cargo run
```

For route-aware local serving with an automatic backend compile at startup:

```bash
cargo ax run dev
```

## Planned global CLI flow

Once published, we target:

```bash
cargo install create-axonyx
create-axonyx my-app --yes
```

## IR Demo Flow

Rust runtime executes compiled IR directly:

```bash
cargo run --manifest-path H:/CODE/axonyx/axonyx-framework/vendor/axonyx-runtime/Cargo.toml -p axonyx-runtime --example execute_json
```

## Docs

The first structured docs index now lives in:

```text
docs/README.md
```

Recommended reading order for the current framework path:

- `docs/overview.md`
- `docs/ax-v2-authoring.md`
- `docs/templates.md`
- `docs/backend-authoring.md`

## Reactive Component Draft

```rust
use axonyx_core::component;
use axonyx_core::prelude::*;

#[component]
fn CounterCard() -> AxNode {
    let count = signal(1);
    let count_for_mem = count.clone();
    let doubled = mem(move || count_for_mem.get() * 2);

    view(|| {
        element("article", vec![
            element("h2", vec![text("Counter")]),
            element("p", vec![text(format!("Count: {}", count.get()))]),
            element("p", vec![text(format!("Double: {}", doubled.get()))]),
        ])
    })
}
```

Props work with the same component shape:

```rust
#[derive(Clone)]
struct GreetingCardProps {
    title: String,
    count: i32,
}

#[component]
fn GreetingCard(props: GreetingCardProps) -> AxNode {
    view(|| {
        element("article", vec![
            element("h2", vec![text(props.title)]),
            element("p", vec![text(format!("Count: {}", props.count))]),
        ])
    })
}
```

Children can stay explicit and simple through props:

```rust
#[derive(Clone)]
struct PanelProps {
    title: String,
    children: Children,
}

#[component]
fn Panel(props: PanelProps) -> AxNode {
    let mut body = vec![element("h2", vec![text(props.title)])];
    body.extend(props.children);

    view(|| element("section", body))
}
```

And for cleaner tree authoring, there is now a first `ax!` draft:

```rust
use axonyx_core::ax;

let node = ax!(article[
    h2["Counter"],
    p["Ready"],
    p[format!("Count: {}", 2)],
]);
```

Attributes are supported too:

```rust
let node = ax!(button(class="primary", data_state="ready")[
    "Launch",
]);
```

Layout primitives now exist as normal components too:

```rust
use axonyx_core::layout_prelude::*;
use axonyx_core::prelude::*;

let node = render_component(
    grid,
    GridProps {
        cols: 3,
        gap: Gap::Token("md"),
        children: children([
            text("Card A"),
            text("Card B"),
            text("Card C"),
        ]),
    },
);
```

The first layout kit now includes:

- `stack`
- `grid`
- `container`
- `center`
- `box`
- `spacer`

The first UI primitive kit now includes:

- `button`
- `card`
- `input`
- `copy`

## Pipeline To UI Draft

Pipelines can now be rendered into real `AxNode` trees through the first pipeline rendering bridge:

```rust
use axonyx_core::pipeline_prelude::*;

let records = vec![
    PipelineRecord::new("p1")
        .titled("Card A")
        .field("status", "draft"),
    PipelineRecord::new("p2")
        .titled("Card B")
        .field("status", "published"),
];

let node = render_pipeline_node(
    r#"Db.Stream("posts") |> layout.Grid(2) |> Card()"#,
    &records,
)?;
```

This first bridge keeps the model simple:

- source metadata becomes a root container
- transforms wrap the rendered record views
- `Card()` uses the first-party `card` and `copy` primitives
- named views such as `ProfileCard()` preserve their identity with `data-view`

## `.ax` AST Draft

Axonyx now also has a first Rust AST draft for `.ax` authoring:

```rust
use axonyx_core::ax_ast_prelude::*;

let document = AxDocument::page(
    "Home",
    [
        AxStatement::data(
            "posts",
            AxExpr::call(["Db", "Stream"], [AxExpr::string("posts")]),
        ),
        AxStatement::component(
            AxComponent::new("Container")
                .prop("max", "xl")
                .block([AxStatement::component(
                    AxComponent::new("Grid")
                        .prop("cols", 3_i64)
                        .prop("gap", "md")
                        .block([AxStatement::each(
                            "post",
                            AxExpr::ident("posts"),
                            [AxStatement::component(
                                AxComponent::new("Card")
                                    .prop("title", AxExpr::ident("post").member("title"))
                                    .block([AxStatement::component(
                                        AxComponent::new("Copy")
                                            .inline(AxExpr::ident("post").member("excerpt")),
                                    )]),
                            )],
                        )]),
                )]),
        ),
    ],
);
```

This draft intentionally models:

- `page`
- `data`
- components
- `each`
- inline content through `->`
- pipeline stages
- styling layers through semantic props, `recipe`, and `class`

## `.ax` Parser And Lowering Sketch

Axonyx now also has a first parser sketch for the indentation-based `.ax` style and a first lowering pass into `AxNode`.

Current parser sketch handles:

- `page`
- `data`
- `each`
- indentation-based component nesting
- inline `->` children
- styling fields such as `recipe` and `class`
- a minimal `|>` pipeline sketch

Current lowering sketch handles:

- evaluating `data` bindings through a resolver
- iterating `each` blocks over lists
- lowering `Container`, `Grid`, `Card`, `Copy`, and `Button`
- preserving `recipe` and `class` as style-level attributes

## Backend AST Draft

Axonyx now also has a first backend AST draft for full-stack authoring layers that lower into Rust.

Current backend model includes:

- `route`
- `loader`
- `action`
- `job`

The draft is intentionally small and focused on framework-shaped patterns rather than replacing all of Rust.

## Query AST Draft

Axonyx now also has a first query AST draft for backend data loading.

Current query model covers:

- `Db.Stream("collection")`
- `where field = value`
- `order field asc|desc`
- `limit`
- `offset`

## Backend Parser Draft

Axonyx now also has a first backend parser draft that can read indentation-based backend authoring blocks.

Current parser draft handles:

- `route METHOD "/path"`
- `loader Name`
- `action Name`
- `job Name`
- `data`
- `input:`
- `insert`
- `update`
- `revalidate`
- `return`
- `send ... with ...`
- query clauses through `where`, `order`, `limit`, and `offset`

## Backend Lowering Draft

Axonyx now also has a first backend lowering draft that turns backend AST blocks into a stable Rust-oriented execution plan.

Current lowering draft covers:

- stable handler identities for `route`, `loader`, `action`, and `job`
- Rust-friendly function names such as `loader_posts_list` and `route_get_api_posts`
- lowered `data` bindings into either expression values or structured query plans
- lowered action input fields into Rust types such as `String` and `bool`
- lowered mutations, `revalidate`, `return`, and `send` steps into codegen-ready plan nodes

## Backend Runtime And Codegen Draft

Axonyx now also has a first backend runtime contract and codegen draft.

Current runtime contract covers:

- `AxQueryExecutor`
- `AxMutationExecutor`
- `AxRevalidator`
- `AxMessenger`
- `AxEnv` with `public` and `secret` namespaces
- the combined `AxBackendRuntime` trait

Current codegen draft covers:

- generating Rust handlers from the backend lowering plan
- emitting runtime-facing request types such as `AxQueryRequest` and `AxInsertRequest`
- generating action input structs
- direct compile flow from backend `.ax` source into a Rust module string

Current env convention covers:

- `.ax`: `Runtime.Env.public.app_name`
- `.env`: `AX_PUBLIC_APP_NAME`
- `.ax`: `Runtime.Env.secret.db_url`
- `.env`: `AX_SECRET_DB_URL`
- `.ax`: `Runtime.Env.secret.db_driver`
- `.env`: `AX_SECRET_DB_DIALECT` with fallback to `AX_SECRET_DB_DRIVER`
- `.env`: `AX_SECRET_DB_TRANSPORT` with default `direct`

Current database adapter draft covers:

- keeping `.ax` query authoring database-agnostic
- selecting `postgres`, `mysql`, `sqlite`, or `memory` at runtime
- resolving the active dialect from `AX_SECRET_DB_DIALECT`
- resolving the active transport from `AX_SECRET_DB_TRANSPORT`
- keeping one backend authoring shape while adapters translate into concrete driver behavior

Current transport draft covers:

- `direct` as the default runtime mode for normal SQL connections
- `api` as an explicit mode for API-key-backed data providers
- provider-specific env values such as `AX_PUBLIC_DATA_API_URL` and `AX_SECRET_DATA_API_KEY`
- backward compatibility with the earlier `AX_SECRET_DB_DRIVER` draft

Axonyx now also has a first SQL dialect draft in `axonyx-core`:

- lowers `AxQueryPlan` into SQL text plus bound parameter slots
- supports `postgres`, `mysql`, and `sqlite`
- keeps placeholder rules dialect-aware such as `$1` for Postgres and `?` for MySQL/SQLite
- gives the runtime adapter layer a clean seam for future real driver execution

## Repo Layout

```text
crates/
  cargo-axonyx/
  create-axonyx/
```

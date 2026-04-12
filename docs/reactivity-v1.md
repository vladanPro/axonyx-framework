# Axonix Reactivity v1

Axonix should not copy React hook names when we have room to choose clearer or shorter terms.
For that reason, this draft uses:

- `signal`
- `mem`
- `effect`
- `resource`

## Mental model

- `signal<T>`: mutable reactive source state
- `mem<T>`: derived state computed from one or more signals
- `effect(...)`: side-effect that reacts to the signals it touches
- `resource<T, E>`: async state container for data loading

## `ax!` draft

To reduce the amount of manual `element(..., vec![...])` code, Axonix now has a first draft of an `ax!` macro:

```rust
use axonix_core::ax;
use axonix_core::prelude::*;

let node = ax!(article[
    h2["Counter"],
    p["Ready"],
    p[format!("Count: {}", 2)],
]);
```

The macro currently supports:

- nested elements like `article[...]`
- attributes like `button(class="primary", data_state="ready")[...]`
- text expressions like `"Hello"` or `format!(...)`
- embedding an existing `AxNode` with `@node ...`

Example:

```rust
let suffix = text("!");

let node = ax!(article(class="shell", data_state="ready")[
    h2["Counter"],
    @node element("span", vec![suffix]),
]);
```

## Draft usage

```rust
use axonix_core::prelude::*;
use axonix_core::component;

#[component]
fn counter_card() -> AxNode {
    let count = signal(0);
    let count_for_mem = count.clone();
    let doubled = mem(move || count_for_mem.get() * 2);

    let count_for_effect = count.clone();
    effect(move || {
        println!("count changed: {}", count_for_effect.get());
    });

    view(|| {
        element(
            "article",
            vec![
                element("h2", vec![text("Axonix Counter")]),
                element("p", vec![text(format!("Count: {}", count.get()))]),
                element("p", vec![text(format!("Double: {}", doubled.get()))]),
            ],
        )
    })
}
```

## Why `#[component]`

The first draft of `#[component]` is intentionally thin.
Right now it keeps the function unchanged, but it gives Axonix a stable syntax surface for future work:

- component metadata
- props validation
- compile-time optimizations
- better dev tooling and diagnostics

## Props draft

Axonix components can now follow a direct props shape:

```rust
use axonix_core::component;
use axonix_core::prelude::*;

#[derive(Clone)]
struct GreetingCardProps {
    title: String,
    count: i32,
}

#[component]
fn greeting_card(props: GreetingCardProps) -> AxNode {
    let count = signal(props.count);
    let title = props.title.clone();

    view(|| {
        element(
            "article",
            vec![
                element("h2", vec![text(title)]),
                element("p", vec![text(format!("Count: {}", count.get()))]),
            ],
        )
    })
}

let node = render_component(
    greeting_card,
    GreetingCardProps {
        title: "Welcome".into(),
        count: 7,
    },
);
```

Right now `Props` is a lightweight marker trait with a blanket impl for `Clone + 'static`.
That keeps the API simple while leaving room for stricter compile-time props validation later.

## Children draft

Axonix can already model `children` explicitly through props:

```rust
use axonix_core::component;
use axonix_core::prelude::*;

#[derive(Clone)]
struct PanelProps {
    title: String,
    children: Children,
}

#[component]
fn panel(props: PanelProps) -> AxNode {
    let mut body = vec![element("h2", vec![text(props.title)])];
    body.extend(props.children);

    view(|| element("section", body))
}

let node = render_component(
    panel,
    PanelProps {
        title: "Axonix".into(),
        children: children([
            element("p", vec![text("First child")]),
            element("p", vec![text("Second child")]),
        ]),
    },
);
```

This keeps the model simple:

- `Children` is just `Vec<AxNode>`
- `children([...])` is a small helper for readability
- later we can add more ergonomic sugar without changing the data shape

## Resource example

```rust
use axonix_core::prelude::*;

async fn load_posts() -> Result<Vec<String>, String> {
    Ok(vec!["One".into(), "Two".into()])
}

fn posts_panel() -> AxNode {
    let posts = resource(load_posts);

    match posts.state() {
        ResourceState::Loading => element("div", vec![text("Loading...")]),
        ResourceState::Ready(items) => element("div", vec![text(format!("{} posts", items.len()))]),
        ResourceState::Error(error) => element("div", vec![text(error)]),
    }
}
```

## Important note

The current implementation in `axonix-core` is intentionally a draft:

- `signal` is functional and mutable
- `mem` computes on demand
- `effect` runs immediately once
- `resource` returns a loading state placeholder

That is enough to stabilize API shape before we build the real scheduler and dependency graph.

## Layout draft

Axonix now has a first layout layer through ordinary components:

```rust
use axonix_core::layout_prelude::*;
use axonix_core::prelude::*;

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

Current layout primitives:

- `stack`
- `grid`
- `container`
- `center`
- `box`
- `spacer`

Current design choice:

- layout is modeled first as ordinary components
- later it can be exposed as pipe-friendly sugar as well
- the stable base stays simple and composable

## UI primitives draft

Axonix now also has a first UI primitive layer through ordinary components:

- `button`
- `card`
- `input`
- `copy`

Example:

```rust
use axonix_core::layout_prelude::*;
use axonix_core::prelude::*;
use axonix_core::ui_prelude::*;

let node = render_component(
    container,
    ContainerProps {
        max_width: "xl",
        children: children([render_component(
            card,
            CardProps {
                title: Some("Axonix".into()),
                children: children([
                    render_component(
                        copy,
                        CopyProps {
                            tag: "p",
                            tone: Tone::Neutral,
                            children: children([text("Single-binary UI framework")]),
                        },
                    ),
                    render_component(
                        button,
                        ButtonProps {
                            tone: Tone::Primary,
                            disabled: false,
                            children: children([text("Launch")]),
                        },
                    ),
                ]),
            },
        )]),
    },
);
```

## Pipeline rendering draft

Axonix now has a first bridge from pipeline IR into real `AxNode` output.

Example:

```rust
use axonix_core::pipeline_prelude::*;

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

What this currently gives us:

- a real renderable tree instead of only parsed IR
- source metadata wrapped into the output tree
- transforms mapped to Axonix layout components
- `Card()` mapped to first-party UI primitives
- named views such as `ProfileCard()` preserved with `data-view`

This is still intentionally modest.
The goal is to prove the pipeline can become real UI through Axonix-native components before we add a more advanced scheduler, richer transforms, or adapter layers.

## `.ax` AST draft

Axonix now has a first Rust AST draft for the future `.ax` authoring format.

The goal is not to freeze parser behavior yet.
The goal is to stabilize the shape of the syntax model before implementing the real parser.

Current AST draft includes:

- `AxDocument`
- `AxPage`
- `AxStatement`
- `AxComponent`
- `AxBody`
- `AxExpr`
- `AxPipeline`
- `AxStyle`

Important design choices in this draft:

- semantic props stay separate from styling override layers
- `recipe` and `class` live in `AxStyle`
- inline child syntax such as `Copy -> post.excerpt` maps to `AxBody::Inline`
- nested indentation maps to `AxBody::Block(Vec<AxStatement>)`
- pipeline expressions have their own AST layer instead of being flattened too early

## `.ax` parser and lowering sketch

Axonix now also has a first parser sketch and lowering layer for the `.ax` direction.

What the parser sketch currently targets:

- the indentation-first page style
- `data` bindings
- `each` blocks
- compact `->` inline children
- style fields like `recipe` and `class`
- a minimal `|>` pipeline form

What the lowering sketch currently targets:

- resolving `Db.Stream(...)`-style calls through an injected resolver
- turning `AxDocument` into a renderable `AxNode`
- mapping common components such as `Container`, `Grid`, `Card`, `Copy`, and `Button`
- preserving style layering as concrete output attributes

This is intentionally still a sketch.
The goal is to validate the data flow from `.ax` source shape to AST and then into a render tree before we build the full parser and compiler pipeline.

## Backend AST draft

Axonix now also has a first backend AST draft that mirrors the full-stack direction described in the notes.

Current backend blocks:

- `AxRoute`
- `AxLoader`
- `AxAction`
- `AxJob`

Current backend statements:

- `data`
- `insert`
- `update`
- `revalidate`
- `return`
- `send`

This draft is deliberately limited.
It is meant to capture framework-native backend authoring shapes that can lower into Rust, not to become a new general-purpose language.

## Query AST draft

Axonix now also has a first query AST draft for backend data loading and database-oriented lowering.

Current query nodes cover:

- stream sources such as `Db.Stream("posts")`
- equality filters through `where`
- sort clauses through `order`
- pagination through `limit` and `offset`

## Backend parser draft

Axonix now also has a first backend parser draft that reads the backend authoring syntax into the backend AST.

What it currently targets:

- top-level `route`, `loader`, `action`, and `job` blocks
- `data` bindings
- `input:` sections for actions
- mutation statements through `insert` and `update`
- response and invalidation steps such as `return` and `revalidate`
- async-style steps such as `send ... with ...`
- query clauses such as `where`, `order`, `limit`, and `offset`

This is intentionally the backend equivalent of the current frontend parser sketch:
small, focused, and built around real template examples instead of trying to solve every future language feature immediately.

## Backend lowering draft

Axonix now also has a first backend lowering draft that sits between the backend AST and future Rust code generation.

What it currently targets:

- stable handler plans for `route`, `loader`, `action`, and `job`
- deterministic Rust function names for generated handlers
- query lowering into a structured query plan instead of flattening straight into strings
- action input lowering into Rust-shaped field types
- statement lowering for `data`, `insert`, `update`, `revalidate`, `return`, and `send`

This matters because it gives Axonix a clean compiler seam:

- parser owns syntax
- AST owns authoring structure
- lowering owns execution shape
- codegen can later focus only on emitting Rust from the stable plan

## Backend runtime and codegen draft

Axonix now also has the first bridge from backend lowering into runtime-facing generated Rust.

What exists now:

- a backend runtime contract in `axonix-runtime`
- query, mutation, revalidation, and send request types
- an `AxEnv` layer with `public` and `secret` namespaces
- a combined `AxBackendRuntime` trait for generated handlers to target
- a first codegen pass that emits Rust module text from the lowered backend plan
- a direct `compile_backend_ax_to_module(...)` helper for the full backend compile path

Why this matters:

- the parser and lowering layers stay independent from any one database
- generated handlers now have a stable contract to call
- we can swap runtime adapters later without redesigning the authoring syntax

Current env naming convention:

- `.ax`: `Runtime.Env.public.app_name`
- `.env`: `AX_PUBLIC_APP_NAME`
- `.ax`: `Runtime.Env.secret.db_url`
- `.env`: `AX_SECRET_DB_URL`

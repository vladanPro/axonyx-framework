# Axonix Framework Monorepo

Axonix is a Rust-first framework focused on:

- Single-binary architecture
- Algebraic UI pipelines (`|>`)
- Fast data-to-UI execution flow

This repository includes:

- `axonix-core`: pipeline parser and core types
- `axonix-macros`: attribute macros such as `#[component]`
- `axonix-runtime`: runtime stub that executes Axonix IR into a render plan
- `create-axonix`: project scaffolding CLI (similar to `create-next-app`)
- `packages/axonix-ts`: TypeScript builder API that emits Axonix IR JSON

## Quick Start

### 1) Create a new Axonix app locally

```bash
cargo run -p create-axonix -- my-app --yes
```

### 2) Run the generated app

```bash
cd my-app
cargo run
```

## Planned global CLI flow

Once published, we target:

```bash
cargo install create-axonix
create-axonix my-app --yes
```

## IR Demo Flow

1. TypeScript builder emits IR:

```ts
import { from } from "@axonix/ts";

const ir = from("posts").grid(3).card().toIR();
```

2. Rust runtime executes the same IR:

```bash
cargo run -p axonix-runtime --example execute_json
```

## Reactive Component Draft

```rust
use axonix_core::component;
use axonix_core::prelude::*;

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
use axonix_core::ax;

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

This first bridge keeps the model simple:

- source metadata becomes a root container
- transforms wrap the rendered record views
- `Card()` uses the first-party `card` and `copy` primitives
- named views such as `ProfileCard()` preserve their identity with `data-view`

## `.ax` AST Draft

Axonix now also has a first Rust AST draft for `.ax` authoring:

```rust
use axonix_core::ax_ast_prelude::*;

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

Axonix now also has a first parser sketch for the indentation-based `.ax` style and a first lowering pass into `AxNode`.

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

Axonix now also has a first backend AST draft for full-stack authoring layers that lower into Rust.

Current backend model includes:

- `route`
- `loader`
- `action`
- `job`

The draft is intentionally small and focused on framework-shaped patterns rather than replacing all of Rust.

## Repo Layout

```text
crates/
  axonix-core/
  axonix-macros/
  axonix-runtime/
  create-axonix/
packages/
  axonix-ts/
```

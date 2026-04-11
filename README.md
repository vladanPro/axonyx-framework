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

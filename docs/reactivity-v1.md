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

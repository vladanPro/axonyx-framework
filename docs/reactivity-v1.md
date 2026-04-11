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

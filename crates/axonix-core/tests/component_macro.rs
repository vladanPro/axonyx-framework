use axonix_core::ax;
use axonix_core::component;
use axonix_core::layout_prelude::*;
use axonix_core::prelude::*;

#[component]
fn counter_card() -> AxNode {
    let count = signal(1);
    let count_for_mem = count.clone();
    let doubled = mem(move || count_for_mem.get() * 2);

    view(|| {
        ax!(article[
            h2["Counter"],
            p[format!("Count: {}", count.get())],
            p[format!("Double: {}", doubled.get())],
        ])
    })
}

#[test]
fn component_attribute_keeps_component_callable() {
    let node = counter_card();

    assert_eq!(
        node,
        AxNode::Element {
            tag: "article",
            attrs: vec![],
            children: vec![
                AxNode::Element {
                    tag: "h2",
                    attrs: vec![],
                    children: vec![AxNode::Text("Counter".to_string())],
                },
                AxNode::Element {
                    tag: "p",
                    attrs: vec![],
                    children: vec![AxNode::Text("Count: 1".to_string())],
                },
                AxNode::Element {
                    tag: "p",
                    attrs: vec![],
                    children: vec![AxNode::Text("Double: 2".to_string())],
                },
            ],
        }
    );
}

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

#[test]
fn component_attribute_supports_props_signature() {
    let node = render_component(
        greeting_card,
        GreetingCardProps {
            title: "Welcome".to_string(),
            count: 7,
        },
    );

    assert_eq!(
        node,
        AxNode::Element {
            tag: "article",
            attrs: vec![],
            children: vec![
                AxNode::Element {
                    tag: "h2",
                    attrs: vec![],
                    children: vec![AxNode::Text("Welcome".to_string())],
                },
                AxNode::Element {
                    tag: "p",
                    attrs: vec![],
                    children: vec![AxNode::Text("Count: 7".to_string())],
                },
            ],
        }
    );
}

#[test]
fn ax_macro_builds_nested_tree() {
    let suffix = text("!");
    let node = ax!(article(class="shell", data_state="ready")[
        h2["Counter"],
        p["Ready"],
        @node element("span", vec![suffix]),
    ]);

    assert_eq!(
        node,
        AxNode::Element {
            tag: "article",
            attrs: vec![
                Attribute {
                    name: "class",
                    value: "shell".to_string(),
                },
                Attribute {
                    name: "data_state",
                    value: "ready".to_string(),
                },
            ],
            children: vec![
                AxNode::Element {
                    tag: "h2",
                    attrs: vec![],
                    children: vec![AxNode::Text("Counter".to_string())],
                },
                AxNode::Element {
                    tag: "p",
                    attrs: vec![],
                    children: vec![AxNode::Text("Ready".to_string())],
                },
                AxNode::Element {
                    tag: "span",
                    attrs: vec![],
                    children: vec![AxNode::Text("!".to_string())],
                },
            ],
        }
    );
}

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

#[test]
fn component_attribute_supports_children_via_props() {
    let node = render_component(
        panel,
        PanelProps {
            title: "Axonix".to_string(),
            children: children([
                element("p", vec![text("First child")]),
                element("p", vec![text("Second child")]),
            ]),
        },
    );

    assert_eq!(
        node,
        AxNode::Element {
            tag: "section",
            attrs: vec![],
            children: vec![
                AxNode::Element {
                    tag: "h2",
                    attrs: vec![],
                    children: vec![AxNode::Text("Axonix".to_string())],
                },
                AxNode::Element {
                    tag: "p",
                    attrs: vec![],
                    children: vec![AxNode::Text("First child".to_string())],
                },
                AxNode::Element {
                    tag: "p",
                    attrs: vec![],
                    children: vec![AxNode::Text("Second child".to_string())],
                },
            ],
        }
    );
}

#[test]
fn layout_components_compose_with_regular_components() {
    let node = render_component(
        grid,
        GridProps {
            cols: 2,
            gap: Gap::Token("lg"),
            children: children([
                render_component(
                    greeting_card,
                    GreetingCardProps {
                        title: "Card A".to_string(),
                        count: 1,
                    },
                ),
                render_component(
                    greeting_card,
                    GreetingCardProps {
                        title: "Card B".to_string(),
                        count: 2,
                    },
                ),
            ]),
        },
    );

    assert_eq!(
        node,
        AxNode::Element {
            tag: "div",
            attrs: vec![
                Attribute {
                    name: "data-layout",
                    value: "grid".to_string(),
                },
                Attribute {
                    name: "data-cols",
                    value: "2".to_string(),
                },
                Attribute {
                    name: "data-gap",
                    value: "lg".to_string(),
                },
            ],
            children: vec![
                AxNode::Element {
                    tag: "article",
                    attrs: vec![],
                    children: vec![
                        AxNode::Element {
                            tag: "h2",
                            attrs: vec![],
                            children: vec![AxNode::Text("Card A".to_string())],
                        },
                        AxNode::Element {
                            tag: "p",
                            attrs: vec![],
                            children: vec![AxNode::Text("Count: 1".to_string())],
                        },
                    ],
                },
                AxNode::Element {
                    tag: "article",
                    attrs: vec![],
                    children: vec![
                        AxNode::Element {
                            tag: "h2",
                            attrs: vec![],
                            children: vec![AxNode::Text("Card B".to_string())],
                        },
                        AxNode::Element {
                            tag: "p",
                            attrs: vec![],
                            children: vec![AxNode::Text("Count: 2".to_string())],
                        },
                    ],
                },
            ],
        }
    );
}

#[test]
fn container_and_center_compose_with_layout_tree() {
    let centered = render_component(
        center,
        CenterProps {
            axis: Some(Axis::Horizontal),
            children: children([text("Hello Axonix")]),
        },
    );

    let node = render_component(
        container,
        ContainerProps {
            max_width: "2xl",
            children: children([centered]),
        },
    );

    assert_eq!(
        node,
        AxNode::Element {
            tag: "div",
            attrs: vec![
                Attribute {
                    name: "data-layout",
                    value: "container".to_string(),
                },
                Attribute {
                    name: "data-max-width",
                    value: "2xl".to_string(),
                },
            ],
            children: vec![AxNode::Element {
                tag: "div",
                attrs: vec![
                    Attribute {
                        name: "data-layout",
                        value: "center".to_string(),
                    },
                    Attribute {
                        name: "data-axis",
                        value: "horizontal".to_string(),
                    },
                ],
                children: vec![AxNode::Text("Hello Axonix".to_string())],
            }],
        }
    );
}

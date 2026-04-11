use axonix_core::ax;
use axonix_core::component;
use axonix_core::layout_prelude::*;
use axonix_core::prelude::*;
use axonix_core::ui_prelude::*;

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

#[test]
fn box_and_spacer_compose_inside_stack() {
    let node = render_component(
        stack,
        StackProps {
            axis: Axis::Vertical,
            gap: Gap::Token("sm"),
            children: children([
                render_component(
                    r#box,
                    BoxProps {
                        tag: "section",
                        attrs: vec![attr("class", "hero")],
                        children: children([text("Header")]),
                    },
                ),
                render_component(
                    spacer,
                    SpacerProps {
                        axis: Axis::Vertical,
                        size: Gap::Px(12),
                    },
                ),
                render_component(
                    r#box,
                    BoxProps {
                        tag: "section",
                        attrs: vec![attr("class", "content")],
                        children: children([text("Body")]),
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
                    value: "stack".to_string(),
                },
                Attribute {
                    name: "data-axis",
                    value: "column".to_string(),
                },
                Attribute {
                    name: "data-gap",
                    value: "sm".to_string(),
                },
            ],
            children: vec![
                AxNode::Element {
                    tag: "section",
                    attrs: vec![
                        Attribute {
                            name: "data-layout",
                            value: "box".to_string(),
                        },
                        Attribute {
                            name: "class",
                            value: "hero".to_string(),
                        },
                    ],
                    children: vec![AxNode::Text("Header".to_string())],
                },
                AxNode::Element {
                    tag: "div",
                    attrs: vec![
                        Attribute {
                            name: "data-layout",
                            value: "spacer".to_string(),
                        },
                        Attribute {
                            name: "data-axis",
                            value: "vertical".to_string(),
                        },
                        Attribute {
                            name: "data-size",
                            value: "12px".to_string(),
                        },
                    ],
                    children: vec![],
                },
                AxNode::Element {
                    tag: "section",
                    attrs: vec![
                        Attribute {
                            name: "data-layout",
                            value: "box".to_string(),
                        },
                        Attribute {
                            name: "class",
                            value: "content".to_string(),
                        },
                    ],
                    children: vec![AxNode::Text("Body".to_string())],
                },
            ],
        }
    );
}

#[test]
fn ui_primitives_compose_inside_layout() {
    let node = render_component(
        container,
        ContainerProps {
            max_width: "xl",
            children: children([render_component(
                card,
                CardProps {
                    title: Some("Axonix".to_string()),
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
                            input,
                            InputProps {
                                value: String::new(),
                                placeholder: Some("Search docs".to_string()),
                                input_type: "text",
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
                    value: "xl".to_string(),
                },
            ],
            children: vec![AxNode::Element {
                tag: "article",
                attrs: vec![Attribute {
                    name: "data-ui",
                    value: "card".to_string(),
                }],
                children: vec![
                    AxNode::Element {
                        tag: "header",
                        attrs: vec![Attribute {
                            name: "data-ui",
                            value: "card-header".to_string(),
                        }],
                        children: vec![AxNode::Text("Axonix".to_string())],
                    },
                    AxNode::Element {
                        tag: "p",
                        attrs: vec![
                            Attribute {
                                name: "data-ui",
                                value: "copy".to_string(),
                            },
                            Attribute {
                                name: "data-tone",
                                value: "neutral".to_string(),
                            },
                        ],
                        children: vec![AxNode::Text("Single-binary UI framework".to_string())],
                    },
                    AxNode::Element {
                        tag: "input",
                        attrs: vec![
                            Attribute {
                                name: "data-ui",
                                value: "input".to_string(),
                            },
                            Attribute {
                                name: "type",
                                value: "text".to_string(),
                            },
                            Attribute {
                                name: "value",
                                value: String::new(),
                            },
                            Attribute {
                                name: "placeholder",
                                value: "Search docs".to_string(),
                            },
                        ],
                        children: vec![],
                    },
                    AxNode::Element {
                        tag: "button",
                        attrs: vec![
                            Attribute {
                                name: "data-ui",
                                value: "button".to_string(),
                            },
                            Attribute {
                                name: "data-tone",
                                value: "primary".to_string(),
                            },
                            Attribute {
                                name: "data-disabled",
                                value: "false".to_string(),
                            },
                        ],
                        children: vec![AxNode::Text("Launch".to_string())],
                    },
                ],
            }],
        }
    );
}

use crate::component;
use crate::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Primary,
    Accent,
}

impl Tone {
    pub fn to_attr_value(&self) -> &'static str {
        match self {
            Tone::Neutral => "neutral",
            Tone::Primary => "primary",
            Tone::Accent => "accent",
        }
    }
}

#[derive(Clone)]
pub struct ButtonProps {
    pub tone: Tone,
    pub disabled: bool,
    pub children: Children,
}

#[component]
pub fn button(props: ButtonProps) -> AxNode {
    element_with_attrs(
        "button",
        vec![
            attr("data-ui", "button"),
            attr("data-tone", props.tone.to_attr_value()),
            attr("data-disabled", props.disabled.to_string()),
        ],
        props.children,
    )
}

#[derive(Clone)]
pub struct CardProps {
    pub title: Option<String>,
    pub children: Children,
}

#[component]
pub fn card(props: CardProps) -> AxNode {
    let mut body = Vec::new();

    if let Some(title) = props.title {
        body.push(element_with_attrs(
            "header",
            vec![attr("data-ui", "card-header")],
            vec![text(title)],
        ));
    }

    body.extend(props.children);

    element_with_attrs("article", vec![attr("data-ui", "card")], body)
}

#[derive(Clone)]
pub struct InputProps {
    pub value: String,
    pub placeholder: Option<String>,
    pub input_type: &'static str,
}

#[component]
pub fn input(props: InputProps) -> AxNode {
    let mut attrs = vec![
        attr("data-ui", "input"),
        attr("type", props.input_type),
        attr("value", props.value),
    ];

    if let Some(placeholder) = props.placeholder {
        attrs.push(attr("placeholder", placeholder));
    }

    element_with_attrs("input", attrs, vec![])
}

#[derive(Clone)]
pub struct CopyProps {
    pub tag: &'static str,
    pub tone: Tone,
    pub children: Children,
}

#[component]
pub fn copy(props: CopyProps) -> AxNode {
    element_with_attrs(
        props.tag,
        vec![
            attr("data-ui", "copy"),
            attr("data-tone", props.tone.to_attr_value()),
        ],
        props.children,
    )
}

pub mod prelude {
    pub use super::button;
    pub use super::card;
    pub use super::copy;
    pub use super::input;
    pub use super::ButtonProps;
    pub use super::CardProps;
    pub use super::CopyProps;
    pub use super::InputProps;
    pub use super::Tone;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_component_emits_ui_attributes() {
        let node = render_component(
            button,
            ButtonProps {
                tone: Tone::Primary,
                disabled: false,
                children: children([text("Launch")]),
            },
        );

        assert_eq!(
            node,
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
            }
        );
    }

    #[test]
    fn card_component_wraps_optional_title() {
        let node = render_component(
            card,
            CardProps {
                title: Some("Overview".to_string()),
                children: children([text("Body")]),
            },
        );

        assert_eq!(
            node,
            AxNode::Element {
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
                        children: vec![AxNode::Text("Overview".to_string())],
                    },
                    AxNode::Text("Body".to_string()),
                ],
            }
        );
    }

    #[test]
    fn input_component_emits_value_and_placeholder() {
        let node = render_component(
            input,
            InputProps {
                value: "axonyx".to_string(),
                placeholder: Some("Search".to_string()),
                input_type: "text",
            },
        );

        assert_eq!(
            node,
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
                        value: "axonyx".to_string(),
                    },
                    Attribute {
                        name: "placeholder",
                        value: "Search".to_string(),
                    },
                ],
                children: vec![],
            }
        );
    }

    #[test]
    fn copy_component_emits_tone_attributes() {
        let node = render_component(
            copy,
            CopyProps {
                tag: "p",
                tone: Tone::Accent,
                children: children([text("Axonyx is fast.")]),
            },
        );

        assert_eq!(
            node,
            AxNode::Element {
                tag: "p",
                attrs: vec![
                    Attribute {
                        name: "data-ui",
                        value: "copy".to_string(),
                    },
                    Attribute {
                        name: "data-tone",
                        value: "accent".to_string(),
                    },
                ],
                children: vec![AxNode::Text("Axonyx is fast.".to_string())],
            }
        );
    }
}

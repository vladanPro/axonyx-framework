use crate::component;
use crate::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gap {
    None,
    Px(u16),
    Token(&'static str),
}

impl Gap {
    pub fn to_attr_value(&self) -> String {
        match self {
            Gap::None => "0".to_string(),
            Gap::Px(value) => format!("{value}px"),
            Gap::Token(token) => (*token).to_string(),
        }
    }
}

#[derive(Clone)]
pub struct StackProps {
    pub axis: Axis,
    pub gap: Gap,
    pub children: Children,
}

#[component]
pub fn stack(props: StackProps) -> AxNode {
    let direction = match props.axis {
        Axis::Horizontal => "row",
        Axis::Vertical => "column",
    };

    element_with_attrs(
        "div",
        vec![
            attr("data-layout", "stack"),
            attr("data-axis", direction),
            attr("data-gap", props.gap.to_attr_value()),
        ],
        props.children,
    )
}

#[derive(Clone)]
pub struct GridProps {
    pub cols: u16,
    pub gap: Gap,
    pub children: Children,
}

#[component]
pub fn grid(props: GridProps) -> AxNode {
    element_with_attrs(
        "div",
        vec![
            attr("data-layout", "grid"),
            attr("data-cols", props.cols.to_string()),
            attr("data-gap", props.gap.to_attr_value()),
        ],
        props.children,
    )
}

#[derive(Clone)]
pub struct ContainerProps {
    pub max_width: &'static str,
    pub children: Children,
}

#[component]
pub fn container(props: ContainerProps) -> AxNode {
    element_with_attrs(
        "div",
        vec![
            attr("data-layout", "container"),
            attr("data-max-width", props.max_width),
        ],
        props.children,
    )
}

#[derive(Clone)]
pub struct CenterProps {
    pub axis: Option<Axis>,
    pub children: Children,
}

#[component]
pub fn center(props: CenterProps) -> AxNode {
    let axis = match props.axis {
        Some(Axis::Horizontal) => "horizontal",
        Some(Axis::Vertical) => "vertical",
        None => "both",
    };

    element_with_attrs(
        "div",
        vec![
            attr("data-layout", "center"),
            attr("data-axis", axis),
        ],
        props.children,
    )
}

pub mod prelude {
    pub use super::center;
    pub use super::container;
    pub use super::grid;
    pub use super::stack;
    pub use super::Axis;
    pub use super::CenterProps;
    pub use super::ContainerProps;
    pub use super::Gap;
    pub use super::GridProps;
    pub use super::StackProps;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_component_emits_layout_attributes() {
        let node = render_component(
            stack,
            StackProps {
                axis: Axis::Vertical,
                gap: Gap::Px(16),
                children: children([text("one"), text("two")]),
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
                        value: "16px".to_string(),
                    },
                ],
                children: vec![
                    AxNode::Text("one".to_string()),
                    AxNode::Text("two".to_string()),
                ],
            }
        );
    }

    #[test]
    fn grid_component_emits_layout_attributes() {
        let node = render_component(
            grid,
            GridProps {
                cols: 3,
                gap: Gap::Token("md"),
                children: children([text("card-a"), text("card-b")]),
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
                        value: "3".to_string(),
                    },
                    Attribute {
                        name: "data-gap",
                        value: "md".to_string(),
                    },
                ],
                children: vec![
                    AxNode::Text("card-a".to_string()),
                    AxNode::Text("card-b".to_string()),
                ],
            }
        );
    }

    #[test]
    fn container_component_emits_layout_attributes() {
        let node = render_component(
            container,
            ContainerProps {
                max_width: "xl",
                children: children([text("content")]),
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
                children: vec![AxNode::Text("content".to_string())],
            }
        );
    }

    #[test]
    fn center_component_emits_layout_attributes() {
        let node = render_component(
            center,
            CenterProps {
                axis: None,
                children: children([text("content")]),
            },
        );

        assert_eq!(
            node,
            AxNode::Element {
                tag: "div",
                attrs: vec![
                    Attribute {
                        name: "data-layout",
                        value: "center".to_string(),
                    },
                    Attribute {
                        name: "data-axis",
                        value: "both".to_string(),
                    },
                ],
                children: vec![AxNode::Text("content".to_string())],
            }
        );
    }
}

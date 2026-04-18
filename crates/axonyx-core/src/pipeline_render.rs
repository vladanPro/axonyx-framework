use serde::{Deserialize, Serialize};

use crate::layout::prelude::*;
use crate::prelude::*;
use crate::ui::prelude::*;
use crate::{
    compile_pipeline, AxonyxIr, CompileError, Source, SourceKind, Transform, TransformKind, View,
    ViewKind,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineField {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineRecord {
    pub id: String,
    pub title: Option<String>,
    pub fields: Vec<PipelineField>,
}

impl PipelineRecord {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: None,
            fields: Vec::new(),
        }
    }

    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn field(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push(PipelineField {
            name: name.into(),
            value: value.into(),
        });
        self
    }
}

pub fn render_pipeline_node(
    input: &str,
    records: &[PipelineRecord],
) -> Result<AxNode, CompileError> {
    let ir = compile_pipeline(input)?;
    Ok(render_ir_node(&ir, records))
}

pub fn render_ir_node(ir: &AxonyxIr, records: &[PipelineRecord]) -> AxNode {
    let rendered_records = children(
        records
            .iter()
            .map(|record| render_view_node(&ir.view, record)),
    );
    let content = apply_transforms(&ir.transforms, rendered_records);
    wrap_source(&ir.source, content, records.len())
}

fn apply_transforms(transforms: &[Transform], rendered_records: Children) -> AxNode {
    enum StageOutput {
        Many(Children),
        One(AxNode),
    }

    let mut current = StageOutput::Many(rendered_records);

    for transform in transforms.iter().rev() {
        current = match (&transform.kind, current) {
            (TransformKind::Grid { columns }, StageOutput::Many(nodes)) => {
                StageOutput::One(render_component(
                    grid,
                    GridProps {
                        cols: *columns,
                        gap: Gap::Token("md"),
                        children: nodes,
                    },
                ))
            }
            (TransformKind::Grid { columns }, StageOutput::One(node)) => {
                StageOutput::One(render_component(
                    grid,
                    GridProps {
                        cols: *columns,
                        gap: Gap::Token("md"),
                        children: children([node]),
                    },
                ))
            }
        };
    }

    match current {
        StageOutput::Many(nodes) => {
            element_with_attrs("div", vec![attr("data-axonyx-stage", "view-list")], nodes)
        }
        StageOutput::One(node) => node,
    }
}

fn wrap_source(source: &Source, content: AxNode, item_count: usize) -> AxNode {
    match &source.kind {
        SourceKind::Collection { name } => element_with_attrs(
            "section",
            vec![
                attr("data-axonyx-source", "collection"),
                attr("data-collection", name.clone()),
                attr("data-items", item_count.to_string()),
            ],
            vec![content],
        ),
    }
}

fn render_view_node(view: &View, record: &PipelineRecord) -> AxNode {
    match &view.kind {
        ViewKind::Card => render_card_view(record),
        ViewKind::Named { name } => element_with_attrs(
            "section",
            vec![attr("data-view", name.clone())],
            vec![render_card_view(record)],
        ),
    }
}

fn render_card_view(record: &PipelineRecord) -> AxNode {
    let body = if record.fields.is_empty() {
        children([render_component(
            copy,
            CopyProps {
                tag: "p",
                tone: Tone::Neutral,
                children: children([text(format!("id: {}", record.id))]),
            },
        )])
    } else {
        children(record.fields.iter().map(render_field))
    };

    render_component(
        card,
        CardProps {
            title: record.title.clone().or_else(|| Some(record.id.clone())),
            children: body,
        },
    )
}

fn render_field(field: &PipelineField) -> AxNode {
    element_with_attrs(
        "div",
        vec![attr("data-field", field.name.clone())],
        vec![
            render_component(
                copy,
                CopyProps {
                    tag: "strong",
                    tone: Tone::Accent,
                    children: children([text(format!("{}:", field.name))]),
                },
            ),
            render_component(
                copy,
                CopyProps {
                    tag: "span",
                    tone: Tone::Neutral,
                    children: children([text(field.value.clone())]),
                },
            ),
        ],
    )
}

pub mod prelude {
    pub use super::render_ir_node;
    pub use super::render_pipeline_node;
    pub use super::PipelineField;
    pub use super::PipelineRecord;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_pipeline_into_grid_of_cards() {
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
        )
        .expect("pipeline should render");

        assert_eq!(
            node,
            AxNode::Element {
                tag: "section",
                attrs: vec![
                    Attribute {
                        name: "data-axonyx-source",
                        value: "collection".to_string(),
                    },
                    Attribute {
                        name: "data-collection",
                        value: "posts".to_string(),
                    },
                    Attribute {
                        name: "data-items",
                        value: "2".to_string(),
                    },
                ],
                children: vec![AxNode::Element {
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
                            value: "md".to_string(),
                        },
                    ],
                    children: vec![
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
                                    children: vec![AxNode::Text("Card A".to_string())],
                                },
                                AxNode::Element {
                                    tag: "div",
                                    attrs: vec![Attribute {
                                        name: "data-field",
                                        value: "status".to_string(),
                                    }],
                                    children: vec![
                                        AxNode::Element {
                                            tag: "strong",
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
                                            children: vec![AxNode::Text("status:".to_string())],
                                        },
                                        AxNode::Element {
                                            tag: "span",
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
                                            children: vec![AxNode::Text("draft".to_string())],
                                        },
                                    ],
                                },
                            ],
                        },
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
                                    children: vec![AxNode::Text("Card B".to_string())],
                                },
                                AxNode::Element {
                                    tag: "div",
                                    attrs: vec![Attribute {
                                        name: "data-field",
                                        value: "status".to_string(),
                                    }],
                                    children: vec![
                                        AxNode::Element {
                                            tag: "strong",
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
                                            children: vec![AxNode::Text("status:".to_string())],
                                        },
                                        AxNode::Element {
                                            tag: "span",
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
                                            children: vec![AxNode::Text("published".to_string())],
                                        },
                                    ],
                                },
                            ],
                        },
                    ],
                }],
            }
        );
    }

    #[test]
    fn named_view_keeps_view_identity_in_output() {
        let records = vec![PipelineRecord::new("u1").field("role", "founder")];

        let node = render_pipeline_node(r#"Db.Stream("users") |> ProfileCard()"#, &records)
            .expect("pipeline should render");

        assert_eq!(
            node,
            AxNode::Element {
                tag: "section",
                attrs: vec![
                    Attribute {
                        name: "data-axonyx-source",
                        value: "collection".to_string(),
                    },
                    Attribute {
                        name: "data-collection",
                        value: "users".to_string(),
                    },
                    Attribute {
                        name: "data-items",
                        value: "1".to_string(),
                    },
                ],
                children: vec![AxNode::Element {
                    tag: "div",
                    attrs: vec![Attribute {
                        name: "data-axonyx-stage",
                        value: "view-list".to_string(),
                    }],
                    children: vec![AxNode::Element {
                        tag: "section",
                        attrs: vec![Attribute {
                            name: "data-view",
                            value: "ProfileCard".to_string(),
                        }],
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
                                    children: vec![AxNode::Text("u1".to_string())],
                                },
                                AxNode::Element {
                                    tag: "div",
                                    attrs: vec![Attribute {
                                        name: "data-field",
                                        value: "role".to_string(),
                                    }],
                                    children: vec![
                                        AxNode::Element {
                                            tag: "strong",
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
                                            children: vec![AxNode::Text("role:".to_string())],
                                        },
                                        AxNode::Element {
                                            tag: "span",
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
                                            children: vec![AxNode::Text("founder".to_string())],
                                        },
                                    ],
                                },
                            ],
                        }],
                    }],
                }],
            }
        );
    }
}

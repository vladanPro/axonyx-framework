use axonix_core::component;
use axonix_core::prelude::*;

#[component]
fn counter_card() -> AxNode {
    let count = signal(1);
    let count_for_mem = count.clone();
    let doubled = mem(move || count_for_mem.get() * 2);

    view(|| {
        element(
            "article",
            vec![
                element("h2", vec![text("Counter")]),
                element("p", vec![text(format!("Count: {}", count.get()))]),
                element("p", vec![text(format!("Double: {}", doubled.get()))]),
            ],
        )
    })
}

#[test]
fn component_attribute_keeps_component_callable() {
    let node = counter_card();

    assert_eq!(
        node,
        AxNode::Element {
            tag: "article",
            children: vec![
                AxNode::Element {
                    tag: "h2",
                    children: vec![AxNode::Text("Counter".to_string())],
                },
                AxNode::Element {
                    tag: "p",
                    children: vec![AxNode::Text("Count: 1".to_string())],
                },
                AxNode::Element {
                    tag: "p",
                    children: vec![AxNode::Text("Double: 2".to_string())],
                },
            ],
        }
    );
}


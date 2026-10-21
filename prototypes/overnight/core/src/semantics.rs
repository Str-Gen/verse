//! Verse derives inline semantics from HTML, never publisher CSS.
use roxmltree::Node;

#[derive(Default)]
pub(crate) struct Style {
    pub display_none: bool,
    pub emphasis: bool,
    pub strong: bool,
}

pub(crate) fn style(node: Node<'_, '_>) -> Style {
    let mut style = Style::default();
    for element in node.ancestors().filter(Node::is_element) {
        style.display_none |= element.has_attribute("hidden");
        match element.tag_name().name() {
            "em" | "i" => style.emphasis = true,
            "strong" | "b" => style.strong = true,
            "head" | "script" | "style" | "template" => style.display_none = true,
            _ => {}
        }
    }
    style
}

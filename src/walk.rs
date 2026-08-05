//! Shared recursive traversal over a parsed document's node tree.

use mq_markdown::Node;

/// Visits every node in `nodes`, recursing into each node's children via `Node::children()`
/// (an image inside a link, text inside emphasis, a heading inside a blockquote, and so on).
///
/// `nodes` is expected to be a full document's top-level node list (`Markdown::nodes`), where
/// block-level siblings (headings, list items, table cells, ...) already appear flat in source
/// order; this only needs to descend for nesting *within* a single node. Traversal order is
/// depth-first but not guaranteed to match document order once it descends — callers that care
/// about order (e.g. comparing heading depths) should sort by `Node::position()` afterwards.
pub(crate) fn walk<'a>(nodes: impl IntoIterator<Item = &'a Node>, f: &mut impl FnMut(&Node)) {
    for node in nodes {
        f(node);
        let children = node.children();
        if !children.is_empty() {
            walk(children.iter(), f);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_visits_nested_nodes_inside_containers() {
        let doc: mq_markdown::Markdown = "[![alt]()](https://example.com)\n".parse().unwrap();
        let mut names = Vec::new();
        walk(doc.nodes.iter(), &mut |node| names.push(node.name().to_string()));
        assert!(names.contains(&"link".to_string()));
        assert!(names.contains(&"image".to_string()));
    }

    #[test]
    fn walk_visits_images_nested_in_table_cells() {
        let doc: mq_markdown::Markdown = "| A |\n|---|\n| ![alt]() |\n".parse().unwrap();
        let mut count = 0;
        walk(doc.nodes.iter(), &mut |node| {
            if matches!(node, Node::Image(_)) {
                count += 1;
            }
        });
        assert_eq!(count, 1);
    }
}

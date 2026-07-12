use crate::document::{parse_xml_document, XmlNode};

pub fn format_xml_lossy(text: &str) -> Result<String, String> {
    let doc = parse_xml_document(text, "format_xml")?;
    let mut output = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");

    format_node(&mut output, doc.root_element(), 0);

    Ok(output)
}

fn format_node(output: &mut String, node: XmlNode<'_, '_>, depth: usize) {
    let indent = "  ".repeat(depth);

    output.push_str(&indent);
    output.push('<');
    output.push_str(node.tag_name().name());

    for attribute in node.attributes() {
        output.push(' ');
        output.push_str(attribute.name());
        output.push_str("=\"");
        push_escaped(output, attribute.value());
        output.push('"');
    }

    let children = node
        .children()
        .filter(|child| child.is_element())
        .collect::<Vec<_>>();
    let text = node.text().map(str::trim).filter(|text| !text.is_empty());

    if children.is_empty() && text.is_none() {
        output.push_str(" />\n");
        return;
    }

    output.push('>');

    if let Some(text) = text {
        push_escaped(output, text);
    }

    if !children.is_empty() {
        output.push('\n');

        for child in children {
            format_node(output, child, depth + 1);
        }

        output.push_str(&indent);
    }

    output.push_str("</");
    output.push_str(node.tag_name().name());
    output.push_str(">\n");
}

fn push_escaped(output: &mut String, value: &str) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&apos;"),
            other => output.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatter_indents_elements_and_escapes_values() {
        let formatted = format_xml_lossy(r#"<Root label="A&amp;B"><Child>1&lt;2</Child></Root>"#)
            .expect("valid XML");

        assert_eq!(
            formatted,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Root label=\"A&amp;B\">\n  <Child>1&lt;2</Child>\n</Root>\n"
        );
    }
}

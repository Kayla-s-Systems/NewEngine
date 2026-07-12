pub type XmlDocument<'input> = roxmltree::Document<'input>;
pub type XmlNode<'a, 'input> = roxmltree::Node<'a, 'input>;

#[inline]
pub fn body_is_xml(body: &[u8]) -> bool {
    std::str::from_utf8(body).map(text_is_xml).unwrap_or(false)
}

#[inline]
pub fn text_is_xml(text: &str) -> bool {
    text.trim_start().starts_with('<')
}

pub fn parse_xml_document<'input>(
    text: &'input str,
    label: &str,
) -> Result<XmlDocument<'input>, String> {
    roxmltree::Document::parse(text).map_err(|error| format!("{label}: XML parse failed: {error}"))
}

pub fn parse_xml_body<'input>(
    body: &'input [u8],
    label: &str,
) -> Result<XmlDocument<'input>, String> {
    let text = std::str::from_utf8(body)
        .map_err(|error| format!("{label}: XML body is not UTF-8: {error}"))?;

    parse_xml_document(text, label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_detection_accepts_leading_whitespace() {
        assert!(text_is_xml("\n  <Root />"));
        assert!(body_is_xml(b"\t<Root />"));
    }

    #[test]
    fn xml_detection_rejects_non_utf8_bodies() {
        assert!(!body_is_xml(&[0xff, b'<']));
    }
}

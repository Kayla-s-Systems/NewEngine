use super::*;

pub(crate) fn source_span_for_named_element(
    xml: &str,
    name: &str,
    source_ref: &str,
) -> UiSourceSpan {
    first_element(xml, name)
        .map(|element| source_span_for_open(xml, &element.open, source_ref))
        .unwrap_or_else(|| source_span_for_offset(xml, 0, source_ref))
}

pub(crate) fn source_span_for_open(xml: &str, open: &str, source_ref: &str) -> UiSourceSpan {
    let offset = xml.find(open).unwrap_or(0);
    source_span_for_offset(xml, offset, source_ref)
}

pub(crate) fn source_span_for_offset(xml: &str, offset: usize, source_ref: &str) -> UiSourceSpan {
    let mut line = 1u32;
    let mut column = 1u32;
    for ch in xml[..offset.min(xml.len())].chars() {
        if ch == '\n' {
            line = line.saturating_add(1);
            column = 1;
        } else {
            column = column.saturating_add(1);
        }
    }
    UiSourceSpan {
        source_ref: source_ref.to_owned(),
        line,
        column,
    }
}

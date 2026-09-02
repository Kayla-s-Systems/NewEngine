use super::{authored_tag, AuthoredPlacementEdit};
use newengine_math::{EulerRot, Vec3};

pub(super) fn patch_authored_transform(
    xml: &mut String,
    edit: &AuthoredPlacementEdit,
) -> Result<(), String> {
    let (yaw, pitch, roll) = edit.transform.rotation.to_euler(EulerRot::YXZ);
    let position = format_vec3(edit.transform.position);
    let rotation = format_triplet([yaw, pitch, roll]);
    let scale = format_vec3(edit.transform.scale);
    patch_tag_attributes_by_id(
        xml,
        authored_tag(edit.source),
        &edit.placement_id,
        &[
            ("position", position.as_str()),
            ("rotation_ypr", rotation.as_str()),
            ("scale", scale.as_str()),
        ],
    )
    .map_err(|error| {
        format!(
            "project save cannot patch {} id='{}' map='{}': {error}",
            authored_tag(edit.source),
            edit.placement_id,
            edit.map_ref
        )
    })
}

fn format_vec3(value: Vec3) -> String {
    format_triplet([value.x, value.y, value.z])
}

fn format_triplet(value: [f32; 3]) -> String {
    format!(
        "{},{},{}",
        format_scalar(value[0]),
        format_scalar(value[1]),
        format_scalar(value[2])
    )
}

fn format_scalar(value: f32) -> String {
    let value = if value.abs() < 0.000_000_5 {
        0.0
    } else {
        value
    };
    let mut text = format!("{value:.6}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.push('0');
    }
    text
}

fn find_tag_span_by_id(
    xml: &str,
    tag: &str,
    id: &str,
) -> Result<Option<(usize, usize, usize)>, String> {
    let needle = format!("<{tag}");
    let mut search_from = 0usize;
    while let Some(relative) = xml[search_from..].find(&needle) {
        let start = search_from + relative;
        let opening_end = xml[start..]
            .find('>')
            .map(|offset| start + offset + 1)
            .ok_or_else(|| format!("unterminated <{tag}> tag"))?;
        let opening = &xml[start..opening_end];
        if xml_attribute_value(opening, "id").as_deref() == Some(id) {
            let element_end = if opening.trim_end().ends_with("/>") {
                opening_end
            } else {
                let closing = format!("</{tag}>");
                xml[opening_end..]
                    .find(&closing)
                    .map(|offset| opening_end + offset + closing.len())
                    .ok_or_else(|| format!("unterminated <{tag}> element id='{id}'"))?
            };
            return Ok(Some((start, opening_end, element_end)));
        }
        search_from = opening_end;
    }
    Ok(None)
}

pub(super) fn patch_tag_attributes_by_id(
    xml: &mut String,
    tag: &str,
    id: &str,
    attributes: &[(&str, &str)],
) -> Result<(), String> {
    let Some((start, opening_end, _)) = find_tag_span_by_id(xml, tag, id)? else {
        return Err(format!("<{tag}> with id='{id}' was not found"));
    };
    let mut patched = xml[start..opening_end].to_owned();
    for (name, value) in attributes {
        patched = set_xml_attribute(&patched, name, value);
    }
    xml.replace_range(start..opening_end, &patched);
    Ok(())
}

pub(super) fn clone_tag_by_id(
    xml: &mut String,
    tag: &str,
    source_id: &str,
    target_id: &str,
) -> Result<bool, String> {
    if find_tag_span_by_id(xml, tag, target_id)?.is_some() {
        return Ok(false);
    }
    let Some((source_start, opening_end, source_end)) = find_tag_span_by_id(xml, tag, source_id)?
    else {
        return Err(format!("<{tag}> with id='{source_id}' was not found"));
    };

    let source_element = xml[source_start..source_end].to_owned();
    let opening_len = opening_end - source_start;
    let mut target_opening = source_element[..opening_len].to_owned();
    target_opening = set_xml_attribute(&target_opening, "id", target_id);
    let mut target_element = source_element;
    target_element.replace_range(..opening_len, &target_opening);

    let line_start = xml[..source_start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let indent = &xml[line_start..source_start];
    let indent = if indent.chars().all(char::is_whitespace) {
        indent
    } else {
        ""
    };
    xml.insert_str(source_end, &format!("\n{indent}{target_element}"));
    Ok(true)
}

pub(super) fn remove_tag_by_id(xml: &mut String, tag: &str, id: &str) -> Result<bool, String> {
    let Some((start, _, element_end)) = find_tag_span_by_id(xml, tag, id)? else {
        return Ok(false);
    };
    let line_start = xml[..start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(start);
    let remove_start = if xml[line_start..start].chars().all(char::is_whitespace) {
        line_start
    } else {
        start
    };
    let mut remove_end = element_end;
    if xml[remove_end..].starts_with("\r\n") {
        remove_end += 2;
    } else if xml[remove_end..].starts_with('\n') {
        remove_end += 1;
    }
    xml.replace_range(remove_start..remove_end, "");
    Ok(true)
}

fn xml_attribute_value(tag: &str, name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let rel = tag[cursor..].find(name)?;
        let start = cursor + rel;
        let before_ok = start == 0 || !is_xml_name_byte(bytes[start - 1]);
        let after = start + name.len();
        let after_ok = after >= bytes.len() || !is_xml_name_byte(bytes[after]);
        if !before_ok || !after_ok {
            cursor = after;
            continue;
        }
        let mut i = after;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'=') {
            cursor = after;
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let quote = *bytes.get(i)?;
        if quote != b'"' && quote != b'\'' {
            cursor = after;
            continue;
        }
        let value_start = i + 1;
        let value_end = tag[value_start..].find(quote as char)? + value_start;
        return Some(tag[value_start..value_end].to_owned());
    }
    None
}

fn set_xml_attribute(tag: &str, name: &str, value: &str) -> String {
    let bytes = tag.as_bytes();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let Some(rel) = tag[cursor..].find(name) else {
            break;
        };
        let start = cursor + rel;
        let before_ok = start == 0 || !is_xml_name_byte(bytes[start - 1]);
        let after = start + name.len();
        let after_ok = after >= bytes.len() || !is_xml_name_byte(bytes[after]);
        if !before_ok || !after_ok {
            cursor = after;
            continue;
        }
        let mut i = after;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) != Some(&b'=') {
            cursor = after;
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let Some(&quote) = bytes.get(i) else {
            break;
        };
        if quote != b'"' && quote != b'\'' {
            cursor = after;
            continue;
        }
        let value_start = i + 1;
        let Some(rel_end) = tag[value_start..].find(quote as char) else {
            break;
        };
        let value_end = value_start + rel_end;
        let mut out = tag.to_owned();
        out.replace_range(value_start..value_end, value);
        return out;
    }

    let insertion = tag
        .rfind("/>")
        .or_else(|| tag.rfind('>'))
        .unwrap_or(tag.len());
    let mut out = tag.to_owned();
    out.insert_str(insertion, &format!(" {name}=\"{value}\""));
    out
}

#[inline]
fn is_xml_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':' | b'.')
}

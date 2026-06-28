use super::*;

#[derive(Clone, Debug)]
pub(crate) struct XmlElement {
    pub(crate) name: String,
    pub(crate) open: String,
    pub(crate) inner: String,
}

pub(crate) fn root_name(xml: &str) -> Option<&str> {
    let mut rest = xml.trim_start();
    if rest.starts_with("<?") {
        let end = rest.find("?>")?;
        rest = rest.get(end + 2..)?.trim_start();
    }
    let open = rest.strip_prefix('<')?;
    let name_end = open.find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')?;
    open.get(..name_end)
}

pub(crate) fn section(xml: &str, name: &str) -> Option<String> {
    first_element(xml, name).map(|element| element.inner)
}

pub(crate) fn first_element(xml: &str, name: &str) -> Option<XmlElement> {
    elements(xml, name).into_iter().next()
}

pub(crate) fn elements(xml: &str, name: &str) -> Vec<XmlElement> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while let Some(start_rel) = find_open_tag(&xml[offset..], name) {
        let start = offset + start_rel;
        let Some(open_end_rel) = xml[start..].find('>') else {
            break;
        };
        let open_end = start + open_end_rel;
        let open = &xml[start..=open_end];
        let self_closing = open.trim_end().ends_with("/>");
        if self_closing {
            out.push(XmlElement {
                name: name.to_owned(),
                open: open.to_owned(),
                inner: String::new(),
            });
            offset = open_end + 1;
            continue;
        }
        let close_token = format!("</{}>", name);
        let Some(close_rel) = xml[open_end + 1..].find(&close_token) else {
            break;
        };
        let inner_start = open_end + 1;
        let close_start = inner_start + close_rel;
        let close_end = close_start + close_token.len();
        out.push(XmlElement {
            name: name.to_owned(),
            open: open.to_owned(),
            inner: xml[inner_start..close_start].to_owned(),
        });
        offset = close_end;
    }
    out
}

pub(crate) fn direct_child_elements(xml: &str) -> Vec<XmlElement> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while let Some(start_rel) = xml[offset..].find('<') {
        let start = offset + start_rel;
        let Some(next) = xml.as_bytes().get(start + 1).copied() else {
            break;
        };
        if matches!(next, b'/' | b'!' | b'?') {
            offset = xml[start..]
                .find('>')
                .map(|end| start + end + 1)
                .unwrap_or(xml.len());
            continue;
        }
        let Some(open_end_rel) = xml[start..].find('>') else {
            break;
        };
        let open_end = start + open_end_rel;
        let open = &xml[start..=open_end];
        let Some(name) = element_name_from_open(open) else {
            offset = open_end + 1;
            continue;
        };
        let self_closing = open.trim_end().ends_with("/>");
        if self_closing {
            out.push(XmlElement {
                name,
                open: open.to_owned(),
                inner: String::new(),
            });
            offset = open_end + 1;
            continue;
        }
        let Some((close_start, close_end)) = matching_close_tag(xml, &name, open_end + 1) else {
            break;
        };
        out.push(XmlElement {
            name,
            open: open.to_owned(),
            inner: xml[open_end + 1..close_start].to_owned(),
        });
        offset = close_end;
    }
    out
}

pub(crate) fn element_name_from_open(open: &str) -> Option<String> {
    let rest = open.trim_start().strip_prefix('<')?.trim_start();
    let name_end = rest.find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')?;
    Some(rest[..name_end].to_owned())
}

pub(crate) fn matching_close_tag(xml: &str, name: &str, from: usize) -> Option<(usize, usize)> {
    let open_token = format!("<{}", name);
    let close_token = format!("</{}>", name);
    let mut depth = 1usize;
    let mut offset = from;
    loop {
        let next_open = xml[offset..].find(&open_token).map(|pos| offset + pos);
        let next_close = xml[offset..].find(&close_token).map(|pos| offset + pos);
        match (next_open, next_close) {
            (Some(open_pos), Some(close_pos)) if open_pos < close_pos => {
                let next = xml.as_bytes().get(open_pos + open_token.len()).copied();
                if matches!(
                    next,
                    Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'>') | Some(b'/')
                ) {
                    let Some(open_end_rel) = xml[open_pos..].find('>') else {
                        return None;
                    };
                    let open_end = open_pos + open_end_rel;
                    if !xml[open_pos..=open_end].trim_end().ends_with("/>") {
                        depth += 1;
                    }
                    offset = open_end + 1;
                } else {
                    offset = open_pos + open_token.len();
                }
            }
            (_, Some(close_pos)) => {
                depth = depth.saturating_sub(1);
                let close_end = close_pos + close_token.len();
                if depth == 0 {
                    return Some((close_pos, close_end));
                }
                offset = close_end;
            }
            _ => return None,
        }
    }
}

pub(crate) fn find_open_tag(haystack: &str, name: &str) -> Option<usize> {
    let needle = format!("<{}", name);
    let mut search = 0usize;
    while let Some(pos_rel) = haystack[search..].find(&needle) {
        let pos = search + pos_rel;
        let next = haystack.as_bytes().get(pos + needle.len()).copied();
        if matches!(
            next,
            Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'>') | Some(b'/')
        ) {
            return Some(pos);
        }
        search = pos + needle.len();
    }
    None
}

pub(crate) fn attr_value(open: &str, key: &str) -> Option<String> {
    parse_attrs(open)
        .remove(key)
        .map(|value| xml_unescape(&value))
}

pub(crate) fn parse_attrs(open: &str) -> BTreeMap<String, String> {
    let mut attrs = BTreeMap::new();
    let bytes = open.as_bytes();
    let mut i = 0usize;
    while i < bytes.len()
        && bytes[i] != b' '
        && bytes[i] != b'\t'
        && bytes[i] != b'\n'
        && bytes[i] != b'\r'
        && bytes[i] != b'>'
        && bytes[i] != b'/'
    {
        i += 1;
    }
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b'>' || bytes[i] == b'/' {
            break;
        }
        let key_start = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric()
                || bytes[i] == b'_'
                || bytes[i] == b'-'
                || bytes[i] == b'.'
                || bytes[i] == b':')
        {
            i += 1;
        }
        let key = open[key_start..i].trim();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || (bytes[i] != b'\"' && bytes[i] != b'\'') {
            continue;
        }
        let quote = bytes[i];
        i += 1;
        let value_start = i;
        while i < bytes.len() && bytes[i] != quote {
            i += 1;
        }
        if i <= bytes.len() && !key.is_empty() {
            attrs.insert(key.to_owned(), open[value_start..i].to_owned());
        }
        i = i.saturating_add(1);
    }
    attrs
}

pub(crate) fn xml_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

pub(crate) fn first_non_empty<const N: usize>(values: [&str; N]) -> String {
    for value in values {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    String::new()
}

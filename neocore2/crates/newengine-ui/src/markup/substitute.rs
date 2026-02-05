#![forbid(unsafe_op_in_unsafe_fn)]

use std::borrow::Cow;

use ahash::AHashMap;

#[inline]
pub fn substitute_vars<'a>(src: &'a str, vars: &AHashMap<String, String>) -> Cow<'a, str> {
    if !src.contains('$') {
        return Cow::Borrowed(src);
    }

    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let b = src.as_bytes();

    while i < b.len() {
        if b[i] == b'$' {
            i += 1;
            let start = i;
            while i < b.len() && is_var_char(b[i]) {
                i += 1;
            }
            let key = &src[start..i];
            if let Some(v) = vars.get(key) {
                out.push_str(v);
            } else {
                out.push('$');
                out.push_str(key);
            }
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }

    Cow::Owned(out)
}

#[inline]
fn is_var_char(c: u8) -> bool {
    matches!(c, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'.' | b'-')
}
fn parse_placement(p: WindowPlacementJson) -> Option<WindowPlacement> {
    let kind = p
        .kind
        .unwrap_or_else(|| "default".to_owned())
        .to_ascii_lowercase();

    match kind.as_str() {
        "centered" => {
            let off = p.offset.unwrap_or([0, 0]);
            Some(WindowPlacement::Centered {
                offset: (off[0], off[1]),
            })
        }
        "default" => Some(WindowPlacement::Default),
        _ => None,
    }
}

#[inline]
fn apply_string(report: &mut StartupLoadReport, key: &'static str, dst: &mut String, v: String) {
    let from = dst.clone();
    if from != v {
        *dst = v.clone();
        report.overrides.push(StartupOverride { key, from, to: v });
    }
}

#[inline]
fn apply_opt_string(
    report: &mut StartupLoadReport,
    key: &'static str,
    dst: &mut Option<String>,
    v: String,
) {
    let from = dst.clone().unwrap_or_else(|| "null".to_owned());
    let to = v.clone();

    let changed = match dst {
        Some(cur) => cur != &v,
        None => true,
    };

    if changed {
        *dst = Some(v);
        report.overrides.push(StartupOverride { key, from, to });
    }
}

#[inline]
fn apply_size(
    report: &mut StartupLoadReport,
    key: &'static str,
    dst: &mut (u32, u32),
    v: (u32, u32),
) {
    let from = format_size(*dst);
    let to = format_size(v);
    if *dst != v {
        *dst = v;
        report.overrides.push(StartupOverride { key, from, to });
    }
}

#[inline]
fn apply_placement(
    report: &mut StartupLoadReport,
    key: &'static str,
    dst: &mut WindowPlacement,
    v: WindowPlacement,
) {
    let from = format!("{:?}", dst);
    let to = format!("{:?}", v);
    if *dst != v {
        *dst = v;
        report.overrides.push(StartupOverride { key, from, to });
    }
}

#[inline]
fn apply_path(report: &mut StartupLoadReport, key: &'static str, dst: &mut PathBuf, v: String) {
    let from = dst.display().to_string();
    let pb = PathBuf::from(v);
    let to = pb.display().to_string();
    if *dst != pb {
        *dst = pb;
        report.overrides.push(StartupOverride { key, from, to });
    }
}

#[inline]
fn format_size(s: (u32, u32)) -> String {
    format!("{}x{}", s.0, s.1)
}

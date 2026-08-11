#![forbid(unsafe_op_in_unsafe_fn)]

use std::ffi::OsStr;

const DISABLE_ARGS: &[&str] = &[
    "--no-startup-window",
    "--skip-startup-window",
    "--disable-startup-window",
    "--headless",
];

const DISABLE_ENV: &[&str] = &[
    "NEWENGINE_STARTUP_WINDOW_DISABLED",
    "NEWENGINE_STARTUP_WINDOW_SKIP",
    "NEWENGINE_HEADLESS",
];

pub(crate) fn disabled_by_process_args_or_env() -> Option<String> {
    if cfg!(test) {
        return Some("cfg:test".to_owned());
    }

    for arg in std::env::args_os().skip(1) {
        if let Some(reason) = disabled_by_arg(&arg) {
            return Some(reason);
        }
    }

    for key in DISABLE_ENV {
        let Ok(value) = std::env::var(key) else {
            continue;
        };
        if is_truthy(&value) {
            return Some(format!("env:{key}"));
        }
    }

    None
}

fn disabled_by_arg(arg: &OsStr) -> Option<String> {
    let text = arg.to_string_lossy().to_ascii_lowercase();
    if DISABLE_ARGS.iter().any(|flag| text == *flag) {
        return Some(format!("arg:{text}"));
    }

    if let Some(value) = text.strip_prefix("--startup-window=") {
        if matches!(value, "0" | "false" | "off" | "no" | "disabled") {
            return Some(format!("arg:{text}"));
        }
    }

    None
}

fn is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "disabled" | "skip"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_environment_disables_prestart_window() {
        assert!(DISABLE_ENV.contains(&"NEWENGINE_HEADLESS"));
        assert!(is_truthy("1"));
        assert!(is_truthy("true"));
        assert!(!is_truthy("0"));
    }
}

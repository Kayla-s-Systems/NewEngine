#![forbid(unsafe_op_in_unsafe_fn)]

use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct Args {
    pub report: Option<PathBuf>,
    pub product: Option<String>,
    pub app: Option<String>,
    pub version: Option<String>,
}

impl Args {
    pub fn parse_env() -> Self {
        let mut out = Args::default();
        let mut it = std::env::args().skip(1);

        while let Some(a) = it.next() {
            match a.as_str() {
                "--report" => out.report = it.next().map(PathBuf::from),
                "--product" => out.product = it.next(),
                "--app" => out.app = it.next(),
                "--version" => out.version = it.next(),
                _ => {}
            }
        }

        out
    }
}

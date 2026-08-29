use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum StartupError {
    Io(PathBuf, io::Error),
    Parse(PathBuf, serde_json::Error),
}
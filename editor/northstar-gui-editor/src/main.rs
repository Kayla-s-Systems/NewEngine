mod app;
mod discovery;
mod format_types;
mod host;
mod inspector;
mod preview;
mod registry;
mod tool_runtime;
mod tools;
mod ui;
mod workspace;
mod ytd_preview;

use std::env;
use std::path::PathBuf;

use crate::host::paths::EditorPaths;
use app::{EditorApp, EditorCommand};

fn main() {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        let root = default_newengine_root();
        if let Err(err) = EditorApp::new(root).run(EditorCommand::LaunchUi) {
            eprintln!("[ERROR] {err}");
            std::process::exit(1);
        }
        return;
    }

    let command = args.remove(0);
    let root = value_after(&args, "--root")
        .map(PathBuf::from)
        .unwrap_or_else(default_newengine_root);
    let with_tools = has_flag(&args, "--with-tools");

    let result = match command.as_str() {
        "ui" | "launch-ui" => EditorApp::new(root).run(EditorCommand::LaunchUi),
        "doctor" => EditorApp::new(root).run(if with_tools {
            EditorCommand::DoctorWithTools
        } else {
            EditorCommand::Doctor
        }),
        "list" => EditorApp::new(root).run(EditorCommand::List),
        "types-list" => EditorApp::new(root).run(EditorCommand::TypesList),
        "types-load-dir" => {
            let Some(dir) = value_after(&args, "--dir") else {
                eprintln!("[ERROR] types-load-dir requires --dir <path>");
                std::process::exit(2);
            };
            EditorApp::new(root).run(EditorCommand::TypesLoadDir(PathBuf::from(dir)))
        }
        "types-add" => {
            let Some(type_id) = value_after(&args, "--type-id") else {
                eprintln!("[ERROR] types-add requires --type-id <id>");
                std::process::exit(2);
            };
            let label = value_after(&args, "--label").unwrap_or_else(|| type_id.clone());
            let content_kind =
                value_after(&args, "--content-kind").unwrap_or_else(|| "unknown".to_owned());
            let extensions = value_after(&args, "--extensions")
                .or_else(|| value_after(&args, "--ext"))
                .map(|value| split_csv(&value))
                .unwrap_or_default();
            let provider_id = value_after(&args, "--provider");
            EditorApp::new(root).run(EditorCommand::TypesAddRuntime {
                type_id,
                label,
                content_kind,
                extensions,
                provider_id,
                can_read: has_flag(&args, "--can-read"),
                can_write: has_flag(&args, "--can-write"),
                can_preview: has_flag(&args, "--can-preview"),
                can_edit_schema: has_flag(&args, "--can-edit-schema"),
                can_validate: has_flag(&args, "--can-validate"),
                can_diff: has_flag(&args, "--can-diff"),
            })
        }
        "tools-list" => EditorApp::new(root).run(EditorCommand::ToolsList),
        "tools-doctor" => EditorApp::new(root).run(EditorCommand::ToolsDoctor),
        "tools-load-dir" => {
            let Some(dir) = value_after(&args, "--dir") else {
                eprintln!("[ERROR] tools-load-dir requires --dir <path>");
                std::process::exit(2);
            };
            EditorApp::new(root).run(EditorCommand::ToolsLoadDir(PathBuf::from(dir)))
        }
        "open" => {
            let Some(asset) = value_after(&args, "--asset") else {
                eprintln!("[ERROR] open requires --asset <path>");
                std::process::exit(2);
            };
            EditorApp::new(root).run(EditorCommand::OpenAsset(PathBuf::from(asset)))
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(format!("unknown command: {other}")),
    };

    if let Err(err) = result {
        eprintln!("[ERROR] {err}");
        std::process::exit(1);
    }
}

fn print_help() {
    println!("NorthStar GUI Editor host shell");
    println!();
    println!("Usage:");
    println!("  northstar-gui-editor                         # launch UI shell");
    println!("  northstar-gui-editor ui [--root <EngineRepo/NewEngine>]");
    println!("  northstar-gui-editor doctor [--with-tools] [--root <EngineRepo/NewEngine>]");
    println!("  northstar-gui-editor list [--root <EngineRepo/NewEngine>]");
    println!("  northstar-gui-editor types-list [--root <EngineRepo/NewEngine>]");
    println!("  northstar-gui-editor types-load-dir --dir <path> [--root <EngineRepo/NewEngine>]");
    println!("  northstar-gui-editor types-add --type-id <id> --ext .foo --can-read [--root <EngineRepo/NewEngine>]");
    println!("  northstar-gui-editor tools-list [--root <EngineRepo/NewEngine>]");
    println!("  northstar-gui-editor tools-doctor [--root <EngineRepo/NewEngine>]");
    println!(
        "  northstar-gui-editor tools-load-dir --dir <tool-root> [--root <EngineRepo/NewEngine>]"
    );
    println!("  northstar-gui-editor open --asset <path> [--root <EngineRepo/NewEngine>]");
    println!();
    println!("This shell is the registry/discovery foundation for the future Rust GUI layer.");
}

fn value_after(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn default_newengine_root() -> PathBuf {
    let current = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    if EditorPaths::looks_like_newengine_root(&current) {
        return current;
    }

    if current.ends_with("northstar-gui-editor") {
        if let Some(editor_dir) = current.parent() {
            if let Some(newengine_root) = editor_dir.parent() {
                if EditorPaths::looks_like_newengine_root(newengine_root) {
                    return newengine_root.to_path_buf();
                }
            }
        }
    }

    let nested = current.join("EngineRepo").join("NewEngine");
    if EditorPaths::looks_like_newengine_root(&nested) {
        return nested;
    }

    current
}

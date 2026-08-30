use std::path::PathBuf;
use newengine_plugin_host::{default_host_api, init_host_context, PluginLoadOrigin, PluginManager};

fn main() {
    let dir = std::env::args_os().nth(1).map(PathBuf::from).expect("usage: load_target_plugin <dir> <id>");
    let id = std::env::args().nth(2).expect("plugin id");
    init_host_context();
    let host = default_host_api();
    let mut manager = PluginManager::new();
    eprintln!("env exclude={:?} target={:?}", std::env::var("NEWENGINE_PLUGIN_EXCLUDE_IDS").ok(), std::env::var("NEWENGINE_PLUGIN_TARGET").ok());
    match manager.load_plugin_id_from_dir_with_origin(&dir, &id, host, PluginLoadOrigin::FirstPartyPlugin) {
        Ok(loaded) => {
            println!("LOADED={loaded}");
            for p in manager.snapshot() {
                println!("PLUGIN id={} version={} caps={}", p.id, p.version, p.capabilities.len());
            }
            manager.shutdown();
        }
        Err(error) => {
            eprintln!("ERROR {error}");
            manager.shutdown();
            std::process::exit(1);
        }
    }
}

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use northstar_gui_editor_gateway::registry::ProviderDescriptor;
use northstar_gui_editor_gateway::tools::{CodecManifest, ToolManifest};

pub struct Discovery {
    root: PathBuf,
}

impl Discovery {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn discover(&self) -> Result<Vec<ProviderDescriptor>, String> {
        let mut providers = Vec::new();

        self.discover_editor_local_tools(&mut providers)?;
        self.discover_codec_manifest(&mut providers)?;
        self.discover_codec_dlls(&mut providers)?;

        providers.sort_by(|a, b| a.id.cmp(&b.id));
        providers.dedup_by(|a, b| a.id == b.id);
        Ok(providers)
    }

    fn discover_editor_local_tools(&self, out: &mut Vec<ProviderDescriptor>) -> Result<(), String> {
        let root = self.editor_local_tools_root();
        self.collect_tool_json(root, out)
    }

    fn editor_local_tools_root(&self) -> PathBuf {
        self.root
            .join("editor")
            .join("northstar-gui-editor")
            .join("tools")
            .join("first_party")
    }

    fn runtime_plugins_root(&self) -> PathBuf {
        self.root
            .parent()
            .map(|parent| parent.join("pluginsRuntime"))
            .unwrap_or_else(|| self.root.join("pluginsRuntime"))
    }

    fn collect_tool_json(&self, root: PathBuf, out: &mut Vec<ProviderDescriptor>) -> Result<(), String> {
        if !root.exists() {
            println!("[DISCOVERY] skip missing editor-local tool root: {}", root.display());
            return Ok(());
        }

        let mut descriptors = Vec::new();
        walk_files(&root, &mut |path| {
            if path.file_name().and_then(|x| x.to_str()) == Some("tool.json") {
                descriptors.push(path.to_path_buf());
            }
            Ok(())
        })
        .map_err(|e| format!("failed to scan {}: {e}", root.display()))?;

        for path in descriptors {
            let text = fs::read_to_string(&path)
                .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
            let manifest = ToolManifest::parse(&text, path.clone())?;
            out.push(manifest.into_provider_descriptor());
        }

        Ok(())
    }

    fn discover_codec_manifest(&self, out: &mut Vec<ProviderDescriptor>) -> Result<(), String> {
        let manifest_path = self
            .runtime_plugins_root()
            .join("codecs")
            .join("codec_manifest.json");

        if !manifest_path.exists() {
            println!("[DISCOVERY] skip missing codec manifest: {}", manifest_path.display());
            return Ok(());
        }

        let text = fs::read_to_string(&manifest_path)
            .map_err(|e| format!("failed to read {}: {e}", manifest_path.display()))?;
        let manifest = CodecManifest::parse(&text, manifest_path.clone())?;
        out.extend(manifest.into_provider_descriptors());
        Ok(())
    }

    fn discover_codec_dlls(&self, out: &mut Vec<ProviderDescriptor>) -> Result<(), String> {
        let root = self.runtime_plugins_root().join("codecs");
        if !root.exists() {
            println!("[DISCOVERY] skip missing codec dll root: {}", root.display());
            return Ok(());
        }

        let mut dlls = Vec::new();
        walk_files(&root, &mut |path| {
            if path.extension().and_then(|x| x.to_str()).is_some_and(|ext| ext.eq_ignore_ascii_case("dll")) {
                dlls.push(path.to_path_buf());
            }
            Ok(())
        })
        .map_err(|e| format!("failed to scan codec dlls {}: {e}", root.display()))?;

        for dll in dlls {
            let Some(stem) = dll.file_stem().and_then(|x| x.to_str()).map(ToOwned::to_owned) else {
                continue;
            };
            let id = normalize_codec_id(&stem);
            if out.iter().any(|provider| provider.id == id) {
                continue;
            }
            out.push(ProviderDescriptor {
                id,
                name: stem.clone(),
                kind: "native-codec-dll".to_owned(),
                source: dll.clone(),
                capabilities: vec![
                    "asset.format.inspect".to_owned(),
                    "asset.format.validate".to_owned(),
                ],
                formats: infer_formats_from_codec_name(&stem),
            });
        }

        Ok(())
    }
}

fn normalize_codec_id(stem: &str) -> String {
    let mut id = stem.to_owned();
    for suffix in ["-dev", "-debug", "-release"] {
        if id.ends_with(suffix) {
            id.truncate(id.len() - suffix.len());
        }
    }
    id
}

fn infer_formats_from_codec_name(name: &str) -> Vec<String> {
    let lower = name.to_ascii_lowercase();
    let mut formats = Vec::new();
    for token in ["listfile", "nepak", "ytd", "ydd", "ytyp", "ymap", "nemat", "neui"] {
        if lower.contains(token) {
            formats.push(format!(".{token}"));
        }
    }
    formats
}

fn walk_files<F>(root: &Path, f: &mut F) -> io::Result<()>
where
    F: FnMut(&Path) -> io::Result<()>,
{
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(name.as_str(), ".git" | ".takesome" | "target" | "node_modules" | "cache" | "logs") {
                continue;
            }
            walk_files(&path, f)?;
        } else {
            f(&path)?;
        }
    }
    Ok(())
}

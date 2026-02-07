#![forbid(unsafe_op_in_unsafe_fn)]

use crate::plugins::host_context;

use super::types::{ConsoleCmdEntry, DynCommand, DynPayload, SuggestItem, SuggestResponse};

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

type CmdFn = fn(&ConsoleRuntime, &str) -> Result<String, String>;

struct Cmd {
    help: &'static str,
    usage: &'static str,
    f: CmdFn,
}

pub struct ConsoleRuntime {
    cmds: BTreeMap<&'static str, Cmd>,

    dyn_cmds: Mutex<BTreeMap<String, DynCommand>>,
    method_cache: Mutex<BTreeMap<String, Vec<String>>>,

    // Generation of service registry snapshot used to build method_cache/dyn_cmds.
    cached_services_gen: AtomicU64,

    exit_requested: AtomicBool,
}

impl ConsoleRuntime {
    pub fn new() -> Self {
        let mut cmds = BTreeMap::<&'static str, Cmd>::new();

        cmds.insert(
            "help",
            Cmd {
                help: "List commands",
                usage: "help",
                f: |rt, _| rt.help_text(),
            },
        );

        cmds.insert(
            "services",
            Cmd {
                help: "List services",
                usage: "services",
                f: |_, _| {
                    let c = host_context::ctx();
                    let g = c
                        .services
                        .lock()
                        .map_err(|_| "services mutex poisoned".to_string())?;
                    Ok(g.keys().cloned().collect::<Vec<_>>().join("\n"))
                },
            },
        );

        cmds.insert(
            "refresh",
            Cmd {
                help: "Refresh console commands from services",
                usage: "refresh",
                f: |rt, _| {
                    rt.refresh_dyn_commands();
                    Ok("refreshed".into())
                },
            },
        );

        cmds.insert(
            "describe",
            Cmd {
                help: "Describe a service",
                usage: "describe <service_id>",
                f: |rt, line| rt.describe_service(line),
            },
        );

        cmds.insert(
            "call",
            Cmd {
                help: "Call a service method",
                usage: "call <service_id> <method> [payload]",
                f: |rt, line| rt.call_service_cmd(line),
            },
        );

        cmds.insert(
            "quit",
            Cmd {
                help: "Exit engine",
                usage: "quit",
                f: |rt, _| {
                    rt.exit_requested.store(true, Ordering::Release);
                    Ok("exit requested".into())
                },
            },
        );

        Self {
            cmds,
            dyn_cmds: Mutex::new(BTreeMap::new()),
            method_cache: Mutex::new(BTreeMap::new()),
            cached_services_gen: AtomicU64::new(0),
            exit_requested: AtomicBool::new(false),
        }
    }

    pub fn take_exit_requested(&self) -> bool {
        self.exit_requested.swap(false, Ordering::AcqRel)
    }

    pub fn exec(&self, line: &str) -> Result<String, String> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(String::new());
        }

        self.refresh_if_services_changed();

        let mut it = line.split_whitespace();
        let head = it.next().unwrap_or("");

        if let Some(d) = self
            .dyn_cmds
            .lock()
            .map_err(|_| "dyn_cmds mutex poisoned".to_string())?
            .get(head)
            .cloned()
        {
            let args = it.collect::<Vec<_>>().join(" ");
            let payload = match d.payload {
                DynPayload::Empty => Vec::new(),
                DynPayload::Raw => args.into_bytes(),
            };
            return self.call_service_raw(&d.service_id, &d.method, &payload);
        }

        if let Some(c) = self.cmds.get(head) {
            return (c.f)(self, line);
        }

        Err(format!("unknown command: {head}"))
    }

    pub fn complete(&self, input: &str) -> Vec<String> {
        self.refresh_if_services_changed();

        let s = input.trim_start();

        if let Some(rest) = s.strip_prefix("describe ") {
            return self.complete_service_id(rest.trim());
        }

        if let Some(rest) = s.strip_prefix("call ") {
            let mut parts = rest.split_whitespace();
            let sid = parts.next().unwrap_or("");
            let after_sid = rest[sid.len()..].trim_start();

            if sid.is_empty() || after_sid.is_empty() {
                return self.complete_service_id(sid);
            }

            let method_prefix = after_sid.split_whitespace().next().unwrap_or("");
            return self.complete_method(sid, method_prefix);
        }

        let head = s.split_whitespace().next().unwrap_or("");
        let mut out = Vec::new();

        for k in self.cmds.keys() {
            if k.starts_with(head) {
                out.push(k.to_string());
            }
        }

        if let Ok(g) = self.dyn_cmds.lock() {
            for k in g.keys() {
                if k.starts_with(head) {
                    out.push(k.to_string());
                }
            }
        }

        out.sort();
        out.dedup();
        out
    }

    pub fn suggest(&self, input: &str) -> SuggestResponse {
        self.refresh_if_services_changed();

        let raw = input;
        let s = raw.trim_start();
        let ends_with_space = raw.ends_with(' ');

        let mut items = Vec::<SuggestItem>::new();

        let tokens: Vec<&str> = s.split_whitespace().collect();
        if tokens.is_empty() {
            self.suggest_first_token("", &mut items);
            items.sort_by(|a, b| a.display.cmp(&b.display));
            return SuggestResponse {
                signature: String::new(),
                items,
            };
        }

        let head = tokens[0];
        if tokens.len() == 1 && !ends_with_space {
            self.suggest_first_token(head, &mut items);
            items.sort_by(|a, b| a.display.cmp(&b.display));
            return SuggestResponse {
                signature: String::new(),
                items,
            };
        }

        if head == "describe" {
            let prefix = if tokens.len() >= 2 { tokens[1] } else { "" };
            let signature = self
                .cmds
                .get("describe")
                .map(|c| c.usage.to_string())
                .unwrap_or_default();

            for sid in self.complete_service_id(prefix) {
                let insert = format!("describe {} ", sid);
                items.push(SuggestItem {
                    kind: "service".into(),
                    display: sid.clone(),
                    insert,
                    help: "service id".into(),
                    usage: "describe <service_id>".into(),
                });
            }

            return SuggestResponse { signature, items };
        }

        if head == "call" {
            let signature = self
                .cmds
                .get("call")
                .map(|c| c.usage.to_string())
                .unwrap_or_default();

            let sid = if tokens.len() >= 2 { tokens[1] } else { "" };
            let want_methods = tokens.len() >= 3 || (ends_with_space && tokens.len() == 2);

            if sid.is_empty() || !want_methods {
                let prefix = sid;
                for s in self.complete_service_id(prefix) {
                    items.push(SuggestItem {
                        kind: "service".into(),
                        display: s.clone(),
                        insert: format!("call {} ", s),
                        help: "service id".into(),
                        usage: "call <service_id> <method> [payload]".into(),
                    });
                }
                return SuggestResponse { signature, items };
            }

            let method_prefix = if tokens.len() >= 3 { tokens[2] } else { "" };
            for m in self.complete_method(sid, method_prefix) {
                items.push(SuggestItem {
                    kind: "method".into(),
                    display: m.clone(),
                    insert: format!("call {} {} ", sid, m),
                    help: "service method".into(),
                    usage: "call <service_id> <method> [payload]".into(),
                });
            }

            return SuggestResponse { signature, items };
        }

        if let Some(c) = self.cmds.get(head) {
            let signature = c.usage.to_string();
            return SuggestResponse { signature, items };
        }

        if let Ok(g) = self.dyn_cmds.lock() {
            if let Some(d) = g.get(head) {
                return SuggestResponse {
                    signature: d.usage.clone(),
                    items,
                };
            }
        }

        SuggestResponse {
            signature: String::new(),
            items,
        }
    }

    fn suggest_first_token(&self, prefix: &str, out: &mut Vec<SuggestItem>) {
        for (name, c) in &self.cmds {
            if name.starts_with(prefix) {
                let insert = if c.usage.contains('<') {
                    format!("{} ", name)
                } else {
                    name.to_string()
                };
                out.push(SuggestItem {
                    kind: "command".into(),
                    display: (*name).to_string(),
                    insert,
                    help: c.help.to_string(),
                    usage: c.usage.to_string(),
                });
            }
        }

        if let Ok(g) = self.dyn_cmds.lock() {
            for (name, c) in g.iter() {
                if name.starts_with(prefix) {
                    let insert = if c.usage.contains('<') {
                        format!("{} ", name)
                    } else {
                        name.to_string()
                    };
                    out.push(SuggestItem {
                        kind: "command".into(),
                        display: name.clone(),
                        insert,
                        help: c.help.clone(),
                        usage: c.usage.clone(),
                    });
                }
            }
        }
    }

    fn complete_service_id(&self, prefix: &str) -> Vec<String> {
        let c = host_context::ctx();
        let g = match c.services.lock() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };

        let mut v: Vec<String> = g
            .keys()
            .filter(|id| id.starts_with(prefix))
            .cloned()
            .collect();

        v.sort();
        v
    }

    fn complete_method(&self, service_id: &str, prefix: &str) -> Vec<String> {
        self.ensure_method_cache(service_id);

        let g = match self.method_cache.lock() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };

        let Some(methods) = g.get(service_id) else {
            return Vec::new();
        };

        let mut out: Vec<String> = methods
            .iter()
            .filter(|m| m.starts_with(prefix))
            .cloned()
            .collect();

        out.sort();
        out.dedup();
        out
    }

    fn refresh_if_services_changed(&self) {
        let gen = host_context::services_generation();
        let cached = self.cached_services_gen.load(Ordering::Acquire);
        if cached != gen {
            self.refresh_dyn_commands();
        }
    }

    fn ensure_method_cache(&self, service_id: &str) {
        // If registry changed - rebuild everything once.
        self.refresh_if_services_changed();

        let has = match self.method_cache.lock() {
            Ok(g) => g.contains_key(service_id),
            Err(_) => false,
        };

        if has {
            return;
        }

        let json = match self.describe_raw(service_id) {
            Ok(v) => v,
            Err(_) => {
                if let Ok(mut g) = self.method_cache.lock() {
                    let _ = g.insert(service_id.to_string(), Vec::new());
                }
                return;
            }
        };

        let mut methods = Vec::new();

        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json) {
            if let Some(arr) = val.get("methods").and_then(|v| v.as_array()) {
                for m in arr {
                    if let Some(name) = m.get("name").and_then(|v| v.as_str()) {
                        methods.push(name.to_string());
                    }
                }
            }
        }

        methods.sort();
        methods.dedup();

        if let Ok(mut g) = self.method_cache.lock() {
            let _ = g.insert(service_id.to_string(), methods);
        }
    }

    pub fn refresh_dyn_commands(&self) {
        let mut out: BTreeMap<String, DynCommand> = BTreeMap::new();
        let mut methods: BTreeMap<String, Vec<String>> = BTreeMap::new();

        let c = host_context::ctx();
        let services = match c.services.lock() {
            Ok(g) => g,
            Err(_) => {
                if let Ok(mut g) = self.dyn_cmds.lock() {
                    g.clear();
                }
                if let Ok(mut g) = self.method_cache.lock() {
                    g.clear();
                }
                self.cached_services_gen
                    .store(host_context::services_generation(), Ordering::Release);
                return;
            }
        };

        for (id, svc) in services.iter() {
            let describe = svc.describe().to_string();

            let Ok(v) = serde_json::from_str::<serde_json::Value>(&describe) else {
                continue;
            };

            if let Some(arr) = v.get("methods").and_then(|x| x.as_array()) {
                let mut mm = Vec::new();
                for m in arr {
                    if let Some(name) = m.get("name").and_then(|x| x.as_str()) {
                        mm.push(name.to_string());
                    }
                }
                mm.sort();
                mm.dedup();
                methods.insert(id.clone(), mm);
            }

            let commands = v
                .get("console")
                .and_then(|c| c.get("commands"))
                .and_then(|c| c.as_array());

            let Some(cmds) = commands else {
                continue;
            };

            for c in cmds {
                let Ok(entry) = serde_json::from_value::<ConsoleCmdEntry>(c.clone()) else {
                    continue;
                };

                let kind = entry.kind.as_deref().unwrap_or("service_call");
                if kind != "service_call" {
                    continue;
                }

                let sid = entry.service_id.clone().unwrap_or_else(|| id.clone());
                let method = entry.method.clone().unwrap_or_default();
                if method.is_empty() {
                    continue;
                }

                let payload = match entry.payload.as_deref() {
                    Some("empty") => DynPayload::Empty,
                    _ => DynPayload::Raw,
                };

                let usage = entry
                    .usage
                    .clone()
                    .unwrap_or_else(|| format!("{} <args>", entry.name));
                let help = entry
                    .help
                    .clone()
                    .unwrap_or_else(|| format!("{sid}::{method}"));

                out.insert(
                    entry.name,
                    DynCommand {
                        help,
                        usage,
                        service_id: sid,
                        method,
                        payload,
                    },
                );
            }
        }

        if let Ok(mut g) = self.dyn_cmds.lock() {
            *g = out;
        }

        if let Ok(mut g) = self.method_cache.lock() {
            *g = methods;
        }

        self.cached_services_gen
            .store(host_context::services_generation(), Ordering::Release);
    }

    fn describe_raw(&self, service_id: &str) -> Result<String, String> {
        let c = host_context::ctx();
        let g = c
            .services
            .lock()
            .map_err(|_| "services mutex poisoned".to_string())?;

        let svc = g
            .get(service_id)
            .ok_or_else(|| format!("unknown service: {service_id}"))?;

        Ok(svc.describe().to_string())
    }

    fn describe_service(&self, line: &str) -> Result<String, String> {
        let mut it = line.split_whitespace();
        let _ = it.next();

        let sid = it.next().unwrap_or("").trim();
        if sid.is_empty() {
            return Err("usage: describe <service_id>".into());
        }

        let raw = self.describe_raw(sid)?;
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
            return Ok(serde_json::to_string_pretty(&v).unwrap_or(raw));
        }
        Ok(raw)
    }

    fn call_service_cmd(&self, line: &str) -> Result<String, String> {
        let mut it = line.split_whitespace();
        let _ = it.next();

        let sid = it.next().unwrap_or("").trim();
        let method = it.next().unwrap_or("").trim();
        let payload = it.collect::<Vec<_>>().join(" ");

        if sid.is_empty() || method.is_empty() {
            return Err("usage: call <service_id> <method> [payload]".into());
        }

        self.call_service_raw(sid, method, payload.as_bytes())
    }

    fn call_service_raw(&self, service_id: &str, method: &str, payload: &[u8]) -> Result<String, String> {
        let c = host_context::ctx();
        let g = c
            .services
            .lock()
            .map_err(|_| "services mutex poisoned".to_string())?;

        let svc = g
            .get(service_id)
            .ok_or_else(|| format!("unknown service: {service_id}"))?;

        let res = svc.call(abi_stable::std_types::RString::from(method), newengine_plugin_api::Blob::from(payload.to_vec()));

        match res.into_result() {
            Ok(b) => {
                let bytes = b.into_vec();
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    return Ok(serde_json::to_string_pretty(&v)
                        .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).to_string()));
                }
                Ok(String::from_utf8_lossy(&bytes).to_string())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn help_text(&self) -> Result<String, String> {
        self.refresh_if_services_changed();

        let mut out = String::new();
        out.push_str("Built-in:\n");
        for (name, c) in &self.cmds {
            out.push_str("  ");
            out.push_str(name);
            out.push_str("  - ");
            out.push_str(c.help);
            out.push('\n');
        }

        if let Ok(dyn_cmds) = self.dyn_cmds.lock() {
            if !dyn_cmds.is_empty() {
                out.push('\n');
                out.push_str("From services:\n");
                for (name, c) in dyn_cmds.iter() {
                    out.push_str("  ");
                    out.push_str(name);
                    out.push_str("  - ");
                    out.push_str(&c.help);
                    out.push('\n');
                }
            }
        }

        Ok(out.trim_end().to_string())
    }
}

impl ConsoleRuntime {
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::new())
    }
}
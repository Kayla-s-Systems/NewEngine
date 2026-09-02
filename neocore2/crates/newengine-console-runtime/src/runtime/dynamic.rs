impl ConsoleRuntime {
    pub fn refresh_dyn_commands(&self) {
        let mut out: BTreeMap<String, DynCommand> = BTreeMap::new();
        let mut methods: BTreeMap<String, Vec<String>> = BTreeMap::new();

        let services = list_service_ids();

        for id in services {
            if id == ENGINE_COMMAND_GATEWAY_ID
                || id == newengine_console_api::COMMAND_PROVIDER_SERVICE_ID
            {
                continue;
            }
            let Some(describe) = describe_service(&id) else {
                continue;
            };

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
                let Ok(entry_cmd) = serde_json::from_value::<ConsoleCmdEntry>(c.clone()) else {
                    continue;
                };

                let kind = entry_cmd.kind.as_deref().unwrap_or("service_call");
                if kind != "service_call" {
                    continue;
                }

                let sid = entry_cmd.service_id.clone().unwrap_or_else(|| id.clone());
                let method = entry_cmd.method.clone().unwrap_or_default();
                if method.is_empty() {
                    continue;
                }

                let payload = match entry_cmd.payload.as_deref() {
                    Some("empty") => DynPayload::Empty,
                    _ => DynPayload::Raw,
                };

                let usage = entry_cmd
                    .usage
                    .clone()
                    .unwrap_or_else(|| format!("{} <args>", entry_cmd.name));
                let help = entry_cmd
                    .help
                    .clone()
                    .or_else(|| entry_cmd.description.clone())
                    .unwrap_or_else(|| format!("{sid}::{method}"));

                out.insert(
                    entry_cmd.name,
                    DynCommand {
                        help,
                        usage,
                        service_id: sid.clone(),
                        method,
                        payload,
                        args: entry_cmd.args,
                        flags: entry_cmd.flags,
                        owner: entry_cmd.owner.unwrap_or(sid),
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
            .store(services_generation(), Ordering::Release);
    }
}

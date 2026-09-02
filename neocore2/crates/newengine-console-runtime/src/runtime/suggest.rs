impl ConsoleRuntime {
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
        let mut v: Vec<String> = list_service_ids()
            .into_iter()
            .filter(|id| id.starts_with(prefix))
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
        let gen = services_generation();
        let cached = self.cached_services_gen.load(Ordering::Acquire);
        if cached != gen {
            self.refresh_dyn_commands();
        }
    }

    fn ensure_method_cache(&self, service_id: &str) {
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
}

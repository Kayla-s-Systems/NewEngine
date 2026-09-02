impl ConsoleRuntime {
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

        if let Some(rest) = s.strip_prefix("get ") {
            return self.complete_cvar_id(rest.trim());
        }

        if let Some(rest) = s.strip_prefix("set ") {
            let id = rest.split_whitespace().next().unwrap_or("");
            if !rest[id.len()..].starts_with(char::is_whitespace) {
                return self.complete_cvar_id(id);
            }
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
}

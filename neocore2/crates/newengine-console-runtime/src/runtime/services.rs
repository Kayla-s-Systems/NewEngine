impl ConsoleRuntime {
    fn describe_raw(&self, service_id: &str) -> Result<String, String> {
        describe_service(service_id).ok_or_else(|| format!("unknown service: {service_id}"))
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

    fn call_service_raw(
        &self,
        service_id: &str,
        method: &str,
        payload: &[u8],
    ) -> Result<String, String> {
        let bytes = call_service_v1(service_id, method, payload)?;

        match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(v) => Ok(serde_json::to_string_pretty(&v)
                .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).to_string())),
            Err(_) => Ok(String::from_utf8_lossy(&bytes).to_string()),
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

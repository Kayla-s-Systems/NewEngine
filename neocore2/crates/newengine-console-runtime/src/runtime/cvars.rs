impl ConsoleRuntime {
    pub fn command_descriptors(&self) -> Vec<CommandDescriptor> {
        self.refresh_if_services_changed();
        let mut descriptors = self
            .cmds
            .iter()
            .map(|(id, command)| CommandDescriptor {
                id: (*id).to_owned(),
                description: command.help.to_owned(),
                usage: command.usage.to_owned(),
                args: Vec::new(),
                flags: CommandFlags::default(),
                owner: "newengine-console-runtime".to_owned(),
            })
            .collect::<Vec<_>>();
        if let Ok(dynamic) = self.dyn_cmds.lock() {
            descriptors.extend(dynamic.iter().map(|(id, command)| CommandDescriptor {
                id: id.clone(),
                description: command.help.clone(),
                usage: command.usage.clone(),
                args: command.args.clone(),
                flags: command.flags.clone(),
                owner: command.owner.clone(),
            }));
        }
        descriptors.sort_by(|a, b| a.id.cmp(&b.id));
        descriptors
    }

    #[inline]
    pub fn cvar_snapshots(&self) -> Vec<CVarSnapshot> {
        self.cvars.snapshots()
    }

    fn complete_cvar_id(&self, prefix: &str) -> Vec<String> {
        self.cvars
            .snapshots()
            .into_iter()
            .map(|entry| entry.descriptor.id)
            .filter(|id| id.starts_with(prefix))
            .collect()
    }

    fn cvars_text(&self) -> Result<String, String> {
        let rows = self.cvars.snapshots();
        serde_json::to_string_pretty(&rows).map_err(|error| error.to_string())
    }

    fn cvar_get_cmd(&self, line: &str) -> Result<String, String> {
        let id = line.split_whitespace().nth(1).unwrap_or("").trim();
        if id.is_empty() {
            return Err("usage: get <cvar_id>".to_owned());
        }
        Ok(self.cvars.get(id)?.display_value())
    }

    fn cvar_set_cmd(&self, line: &str) -> Result<String, String> {
        let mut parts = line
            .splitn(3, char::is_whitespace)
            .filter(|part| !part.is_empty());
        let _ = parts.next();
        let id = parts.next().unwrap_or("").trim();
        let value = parts.next().unwrap_or("");
        if id.is_empty() || value.is_empty() {
            return Err("usage: set <cvar_id> <value>".to_owned());
        }
        Ok(self.cvars.set_from_str(id, value)?.display_value())
    }
}

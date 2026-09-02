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
                f: |_, _| Ok(list_service_ids().join("\n")),
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
            "cvars",
            Cmd {
                help: "List registered typed CVars",
                usage: "cvars",
                f: |rt, _| rt.cvars_text(),
            },
        );

        cmds.insert(
            "get",
            Cmd {
                help: "Read a typed CVar",
                usage: "get <cvar_id>",
                f: |rt, line| rt.cvar_get_cmd(line),
            },
        );

        cmds.insert(
            "set",
            Cmd {
                help: "Write a typed CVar",
                usage: "set <cvar_id> <value>",
                f: |rt, line| rt.cvar_set_cmd(line),
            },
        );

        cmds.insert(
            "quit",
            Cmd {
                help: "Exit engine",
                usage: "quit",
                f: |_, _| {
                    ShutdownToken::global_request();
                    Ok("shutdown requested".into())
                },
            },
        );

        Self {
            cmds,
            dyn_cmds: Mutex::new(BTreeMap::new()),
            method_cache: Mutex::new(BTreeMap::new()),
            cvars: global_cvar_registry(),
            cached_services_gen: AtomicU64::new(0),
        }
    }
}

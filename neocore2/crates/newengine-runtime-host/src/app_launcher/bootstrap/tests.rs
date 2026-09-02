#[cfg(test)]
mod project_scripting_policy_tests {
    use super::project_scripting_backend_tag;

    #[test]
    fn scripting_runtime_hint_maps_to_backend_tags() {
        assert_eq!(
            project_scripting_backend_tag("typescript"),
            Some("backend.typescript.v8")
        );
        assert_eq!(
            project_scripting_backend_tag("TS"),
            Some("backend.typescript.v8")
        );
        assert_eq!(project_scripting_backend_tag("lua"), Some("backend.lua54"));
        assert_eq!(project_scripting_backend_tag("unknown"), None);
    }
}

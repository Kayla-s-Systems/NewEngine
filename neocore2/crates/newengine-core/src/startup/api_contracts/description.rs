#![forbid(unsafe_op_in_unsafe_fn)]

pub(crate) fn parse_methods_from_description(description: &str) -> Result<Vec<String>, String> {
    let v: serde_json::Value = serde_json::from_str(description).map_err(|e| e.to_string())?;
    let Some(methods) = v.get("methods") else {
        return Err("missing methods".to_owned());
    };

    let mut out = Vec::new();
    if let Some(methods_array) = methods.as_array() {
        out.reserve(methods_array.len());
        for item in methods_array {
            if let Some(name) = item.as_str() {
                out.push(name.to_owned());
                continue;
            }
            if let Some(name) = item.get("name").and_then(|x| x.as_str()) {
                out.push(name.to_owned());
            }
        }
    } else if let Some(methods_object) = methods.as_object() {
        out.reserve(methods_object.len());
        for name in methods_object.keys() {
            out.push(name.to_owned());
        }
    } else {
        return Err("methods must be an array or object".to_owned());
    }
    out.sort();
    out.dedup();
    Ok(out)
}

pub(crate) fn method_statuses(methods: &[&str]) -> Vec<String> {
    methods
        .iter()
        .map(|method| {
            let label = method
                .rsplit_once('.')
                .map(|(_, tail)| tail)
                .unwrap_or(method);
            format!("{label}=yes")
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedServiceDescriptionContract {
    pub version: u32,
    pub protocol: Option<String>,
    pub contract: Option<String>,
}

pub(crate) fn parse_contract_from_description(
    description: &str,
) -> Result<ParsedServiceDescriptionContract, String> {
    let value: serde_json::Value = serde_json::from_str(description).map_err(|e| e.to_string())?;
    let version = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .map(u32::try_from)
        .transpose()
        .map_err(|_| "service description version exceeds u32".to_owned())?
        .unwrap_or(1);
    let text = |key: &str| {
        value
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    Ok(ParsedServiceDescriptionContract {
        version,
        protocol: text("protocol"),
        contract: text("contract"),
    })
}

pub(crate) fn expected_contract_family(expected: &str) -> String {
    let expected = expected.trim();
    if let Some((family, _range)) = expected.split_once(">=") {
        return family.trim().to_owned();
    }
    expected.to_owned()
}

pub(crate) fn contract_family_matches(
    expected: &str,
    actual: &ParsedServiceDescriptionContract,
) -> bool {
    let family = expected_contract_family(expected);
    if family.is_empty() {
        return true;
    }
    let exact_expected = !expected.contains(">=");
    let identity_candidates = actual
        .protocol
        .iter()
        .chain(actual.contract.iter())
        .filter(|candidate| candidate.starts_with("newengine."))
        .collect::<Vec<_>>();
    // Legacy services may expose transport names such as `json` as their protocol.
    // Those values are not interface identities and must not be compared to API families.
    // Once a provider publishes a `newengine.*` identity, however, it must match.
    if identity_candidates.is_empty() {
        return true;
    }
    identity_candidates.into_iter().any(|candidate| {
        if exact_expected {
            candidate == &family || candidate.starts_with(&format!("{family}/"))
        } else {
            candidate == &family
                || candidate.starts_with(&format!("{family}/"))
                || candidate.starts_with(&format!("{family}."))
        }
    })
}

#[cfg(test)]
mod contract_tests {
    use super::*;

    #[test]
    fn semver_style_expectation_matches_versioned_protocol_family() {
        let actual = ParsedServiceDescriptionContract {
            version: 1,
            protocol: Some("newengine.entity-api/v1".to_owned()),
            contract: None,
        };
        assert!(contract_family_matches(
            "newengine.entity-api >= 0.1.x",
            &actual
        ));
        assert!(!contract_family_matches(
            "newengine.world-api >= 0.1.x",
            &actual
        ));
    }

    #[test]
    fn generic_transport_protocol_does_not_fake_interface_identity() {
        let actual = ParsedServiceDescriptionContract {
            version: 1,
            protocol: Some("json".to_owned()),
            contract: None,
        };
        assert!(contract_family_matches(
            "newengine.assets.materials-api >= 0.1.x",
            &actual
        ));
    }

    #[test]
    fn exact_runtime_contract_matches_exact_field() {
        let actual = ParsedServiceDescriptionContract {
            version: 1,
            protocol: None,
            contract: Some("newengine.time.runtime.v1".to_owned()),
        };
        assert!(contract_family_matches(
            "newengine.time.runtime.v1",
            &actual
        ));
    }
}

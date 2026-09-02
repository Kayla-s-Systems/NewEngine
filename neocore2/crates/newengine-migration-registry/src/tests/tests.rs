    use super::*;

    #[test]
    fn registry_is_well_formed_and_covers_readable_legacy_versions() {
        if let Err(errors) = validate_registry() {
            panic!("migration registry invalid: {}", errors.join("; "));
        }
    }

    #[test]
    fn ytd_v2_to_v1_preserves_stored_body_and_metadata() {
        use std::io::Write as _;
        let raw = b"legacy-ytd-body";
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(raw).unwrap();
        let stored = encoder.finish().unwrap();
        let metadata=br#"{"schema":"newengine.asset.list_file.header_metadata.v1","logical_path":"Content/test.ytd","content_kind":"ytd_texture_dictionary","entries":[],"dependencies":[],"namespaces":[],"metadata":{},"warnings":[],"policy":[]}"#;
        let source =
            newengine_assets_api::encode_list_file(newengine_assets_api::ListFileEncodeRequest {
                content_kind: newengine_assets_api::LIST_FILE_CONTENT_KIND_YTD,
                content_schema_version: 2,
                entry_count: 0,
                additional_flags: 0,
                min_size_class: 5,
                header_metadata: metadata,
                body_stored: &stored,
                body_uncompressed_len: raw.len() as u64,
                body_raw_hash: None,
                stable_file_id: None,
                import_settings_hash: None,
            })
            .unwrap();
        let spec = migration("asset.ytd.schema.v2_to_v1").unwrap();
        let out = migrate_bytes(spec, &source, "Content/test.ytd").unwrap();
        let h = newengine_assets_api::parse_list_file_header(&out).unwrap();
        assert_eq!(h.content_schema_version, 1);
        let decoded = newengine_assets_api::decode_list_file_envelope(
            &out,
            newengine_assets_api::LIST_FILE_CONTENT_KIND_YTD,
            "Content/test.ytd",
        )
        .unwrap();
        assert_eq!(decoded.body, raw);
    }
    #[test]
    fn first_party_corpus_contains_no_registered_legacy_versions() {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let neocore = crate_dir.parent().and_then(Path::parent).expect("neocore");
        let repo_root = neocore
            .parent()
            .and_then(Path::parent)
            .expect("NorthStar root");
        if let Err(errors) = validate_corpus_canonical(repo_root) {
            panic!("migration corpus gate failed: {}", errors.join("; "));
        }
    }

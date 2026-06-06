#![forbid(unsafe_op_in_unsafe_fn)]
//! Unified first-party NEF8/ListFile and package format registry.
//!
//! One crate owns the descriptor table; the registry remains data-driven and
//! consumers still receive `AssetFileTypeDescriptor` records.

use newengine_assets_api::{codec_type, method, AssetFileTypeDescriptor};

pub const NEF8_MAGIC_HEX: &str = "4e454638";
pub const DOMAIN_MANIFEST_OUTPUT: &str = "domain.manifest_json";
pub const ASSET_BLOB_OUTPUT: &str = "asset.blob";

#[derive(Clone, Copy, Debug)]
pub struct Nef8FormatSpec {
    pub extension: &'static str,
    pub asset_kind: &'static str,
    pub content_kind: Option<u32>,
    pub semantic_gateway: &'static str,
    pub handler_service: &'static str,
    pub selector_syntax: Option<&'static str>,
    pub purpose: &'static str,
    pub consumer_domains: &'static [&'static str],
}

impl Nef8FormatSpec {
    pub fn descriptor(self) -> AssetFileTypeDescriptor {
        if self.extension == "nepak" {
            return package_descriptor(self);
        }
        let extension = AssetFileTypeDescriptor::extension_key(self.extension);
        let mut descriptor = AssetFileTypeDescriptor {
            extension: extension.clone(),
            asset_kind: self.asset_kind.to_owned(),
            container: format!("newengine.listfile.nef8.{extension}"),
            content_kind: self.content_kind,
            codec_type: codec_type::LIST_FILE.to_owned(),
            byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned(),
            semantic_gateway: self.semantic_gateway.to_owned(),
            handler_service: self.handler_service.to_owned(),
            read_method: method::DECODE_V1.to_owned(),
            selector_syntax: self.selector_syntax.map(ToOwned::to_owned),
            consumer_domains: self.consumer_domains.iter().map(|it| (*it).to_owned()).collect(),
            magic: Some(NEF8_MAGIC_HEX.to_owned()),
            outputs: vec![
                newengine_assets_api::ASSET_LIST_FILE_MANIFEST_OUTPUT.to_owned(),
                newengine_assets_api::ASSET_LIST_FILE_HEADER_OUTPUT.to_owned(),
                newengine_assets_api::ASSET_LIST_FILE_BODY_OUTPUT.to_owned(),
                DOMAIN_MANIFEST_OUTPUT.to_owned(),
                ASSET_BLOB_OUTPUT.to_owned(),
            ],
            priority: 0,
            vfs_backed: true,
            runtime_ready: true,
            preview_provider: true,
            // Generic inspect providers may expose editable field schema, but
            // write-back requires a concrete format/package writer capability.
            editable: false,
            schema_editable: matches!(extension.as_str(), "ytyp" | "ydd" | "ytd" | "nemat" | "neftd"),
            write_back_available: matches!(extension.as_str(), "ytyp" | "ydd" | "ytd" | "nemat" | "neftd"),
            writer_capability: if matches!(extension.as_str(), "ytyp" | "ydd" | "ytd" | "nemat" | "neftd") {
                newengine_assets_api::ASSETS_PACKAGE_WRITER_CAPABILITY_ID.to_owned()
            } else {
                String::new()
            },
            inspect_contract: format!("asset.inspect.{extension}.v1"),
            edit_contract: if matches!(extension.as_str(), "ytyp" | "ydd" | "ytd" | "nemat" | "neftd") {
                format!("asset.edit.{extension}.v1")
            } else {
                String::new()
            },
            allow_nested_assets: false,
            native_container: true,
            requires_magic: true,
            notes: format!("Unified NEF8 descriptor: {}", self.purpose),
            ..Default::default()
        };
        descriptor.normalize_layer_contract();
        descriptor
    }
}

fn package_descriptor(spec: Nef8FormatSpec) -> AssetFileTypeDescriptor {
    let mut descriptor = AssetFileTypeDescriptor {
        extension: spec.extension.to_owned(),
        asset_kind: spec.asset_kind.to_owned(),
        container: "newengine.asset_package.v1".to_owned(),
        codec_type: codec_type::CONTAINER.to_owned(),
        byte_owner: newengine_assets_api::ENGINE_ASSET_SERVICE_ID.to_owned(),
        semantic_gateway: spec.semantic_gateway.to_owned(),
        handler_service: spec.handler_service.to_owned(),
        read_method: method::DECODE_V1.to_owned(),
        consumer_domains: spec.consumer_domains.iter().map(|it| (*it).to_owned()).collect(),
        magic: Some("4e4550414b010000".to_owned()),
        outputs: vec!["container.vfs_layer".to_owned(), ASSET_BLOB_OUTPUT.to_owned()],
        priority: 0,
        vfs_backed: true,
        runtime_ready: true,
        preview_provider: true,
        editable: false,
        schema_editable: false,
        write_back_available: false,
        writer_capability: String::new(),
        inspect_contract: "asset.inspect.nepak.v1".to_owned(),
        edit_contract: String::new(),
        allow_nested_assets: true,
        native_container: true,
        requires_magic: true,
        notes: format!("Unified package descriptor: {}", spec.purpose),
        ..Default::default()
    };
    descriptor.normalize_layer_contract();
    descriptor
}

pub mod nemat {
    pub const EXTENSION: &str = "nemat";
    pub const ASSET_KIND: &str = "material_library";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_NEMAT;
    pub const PURPOSE: &str = "Material Library";
    pub const SEMANTIC_GATEWAY: &str = "engine.materials";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.nemat";
    pub const SELECTOR_SYNTAX: &str = "file.nemat@material_entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.materials", "engine.model", "engine.render"];
}

pub mod nepak {
    pub const EXTENSION: &str = "nepak";
    pub const ASSET_KIND: &str = "asset_package";
    pub const PURPOSE: &str = "NewEngine Asset Package";
    pub const SEMANTIC_GATEWAY: &str = newengine_assets_api::ENGINE_ASSET_SERVICE_ID;
    pub const HANDLER_SERVICE: &str = "asset.codec.nepak";
    pub const CONSUMER_DOMAINS: &[&str] = &[newengine_assets_api::ENGINE_ASSET_SERVICE_ID];
}

pub mod neui {
    pub const EXTENSION: &str = "neui";
    pub const ASSET_KIND: &str = "ui_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_NEUI;
    pub const PURPOSE: &str = "NewEngine UI Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.ui";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.neui";
    pub const SELECTOR_SYNTAX: &str = "file.neui@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.ui", "engine.assets.graph", "engine.ui", "engine.ui.text", "engine.assets.textures", "engine.render", "engine.editor"];
}

pub mod ybd {
    pub const EXTENSION: &str = "ybd";
    pub const ASSET_KIND: &str = "bounds_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YBD;
    pub const PURPOSE: &str = "Bounds Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models.collisions";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ybd";
    pub const SELECTOR_SYNTAX: &str = "file.ybd@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.models.collisions", "engine.physics", "engine.assets.models", "engine.scene"];
}

pub mod ybn {
    pub const EXTENSION: &str = "ybn";
    pub const ASSET_KIND: &str = "bounds_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YBN;
    pub const PURPOSE: &str = "Bounds / Collision";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models.collisions";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ybn";
    pub const SELECTOR_SYNTAX: &str = "file.ybn@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.models.collisions", "engine.physics", "engine.assets.models", "engine.scene"];
}

pub mod ycd {
    pub const EXTENSION: &str = "ycd";
    pub const ASSET_KIND: &str = "clip_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YCD;
    pub const PURPOSE: &str = "Animation Clips / Clip Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models.skeletons";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ycd";
    pub const SELECTOR_SYNTAX: &str = "file.ycd@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.models.skeletons", "engine.assets.models", "engine.scene", "engine.render"];
}

pub mod ydd {
    pub const EXTENSION: &str = "ydd";
    pub const ASSET_KIND: &str = "drawable_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD;
    pub const PURPOSE: &str = "Drawable/Model Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.model";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ydd";
    pub const SELECTOR_SYNTAX: &str = "file.ydd@drawable_entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.model", "engine.materials", "engine.render"];
}

pub mod ydr {
    pub const EXTENSION: &str = "ydr";
    pub const ASSET_KIND: &str = "drawable";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YDR;
    pub const PURPOSE: &str = "Single Drawable";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ydr";
    pub const SELECTOR_SYNTAX: &str = "file.ydr@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.models", "engine.assets.materials", "engine.render"];
}

pub mod yed {
    pub const EXTENSION: &str = "yed";
    pub const ASSET_KIND: &str = "expression_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YED;
    pub const PURPOSE: &str = "Expression Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models.skeletons";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.yed";
    pub const SELECTOR_SYNTAX: &str = "file.yed@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.models.skeletons", "engine.assets.models", "engine.scene", "engine.render"];
}

pub mod yfd {
    pub const EXTENSION: &str = "yfd";
    pub const ASSET_KIND: &str = "frame_filter_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YFD;
    pub const PURPOSE: &str = "Frame Filter Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models.skeletons";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.yfd";
    pub const SELECTOR_SYNTAX: &str = "file.yfd@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.models.skeletons", "engine.assets.models", "engine.scene", "engine.render"];
}

pub mod neftd {
    pub const EXTENSION: &str = "neftd";
    pub const ASSET_KIND: &str = "font_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_NEFTD;
    pub const PURPOSE: &str = "Font Dictionary / Typeface Family";
    pub const SEMANTIC_GATEWAY: &str = "engine.ui.text";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.neftd";
    pub const SELECTOR_SYNTAX: &str = "file.neftd@font_face";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.ui.text", "engine.ui", "engine.assets.ui", "engine.render", "engine.editor"];
}

pub mod yld {
    pub const EXTENSION: &str = "yld";
    pub const ASSET_KIND: &str = "cloth_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YLD;
    pub const PURPOSE: &str = "Cloth Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.yld";
    pub const SELECTOR_SYNTAX: &str = "file.yld@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.models.skeletons", "engine.assets.models", "engine.scene", "engine.render"];
}

pub mod ymap {
    pub const EXTENSION: &str = "ymap";
    pub const ASSET_KIND: &str = "map_data";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YMAP;
    pub const PURPOSE: &str = "Map Data / Placement";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.maps";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ymap";
    pub const SELECTOR_SYNTAX: &str = "file.ymap@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.maps", "engine.scene", "engine.assets.definitions", "engine.assets.graph", "engine.assets.models", "engine.assets.materials", "engine.assets.textures", "engine.physics", "engine.streaming", "engine.editor"];
}

pub mod ymf {
    pub const EXTENSION: &str = "ymf";
    pub const ASSET_KIND: &str = "manifest";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YMF;
    pub const PURPOSE: &str = "Manifest / Dependencies";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.graph";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ymf";
    pub const SELECTOR_SYNTAX: &str = "file.ymf@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.graph", "engine.assets.definitions", "engine.assets.maps", "engine.scene", "engine.assets.models", "engine.assets.materials", "engine.assets.textures"];
}

pub mod ymt {
    pub const EXTENSION: &str = "ymt";
    pub const ASSET_KIND: &str = "metadata";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YMT;
    pub const PURPOSE: &str = "Metadata Container";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.definitions";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ymt";
    pub const SELECTOR_SYNTAX: &str = "file.ymt@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.definitions", "engine.assets.graph", "engine.scene", "engine.assets.models", "engine.assets.materials", "engine.physics", "engine.ai", "engine.editor", "engine.streaming"];
}

pub mod ypdb {
    pub const EXTENSION: &str = "ypdb";
    pub const ASSET_KIND: &str = "pose_database";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YPDB;
    pub const PURPOSE: &str = "Pose Database";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models.skeletons";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ypdb";
    pub const SELECTOR_SYNTAX: &str = "file.ypdb@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.models.skeletons", "engine.assets.models", "engine.scene", "engine.render"];
}

pub mod ysc {
    pub const EXTENSION: &str = "ysc";
    pub const ASSET_KIND: &str = "script_module";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YSC;
    pub const PURPOSE: &str = "Opaque Script Module Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.scripting";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ysc";
    pub const SELECTOR_SYNTAX: &str = "file.ysc@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.scripting", "engine.scene", "engine.assets.definitions", "engine.assets.graph", "engine.ui", "engine.ai", "engine.editor", "engine.streaming"];
}

pub mod ytd {
    pub const EXTENSION: &str = "ytd";
    pub const ASSET_KIND: &str = "texture_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YTD;
    pub const PURPOSE: &str = "Texture Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ytd";
    pub const SELECTOR_SYNTAX: &str = "file.ytd@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets", "engine.materials", "engine.model", "engine.ui", "engine.render"];
}

pub mod ytf {
    pub const EXTENSION: &str = "ytf";
    pub const ASSET_KIND: &str = "unknown_y_file";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YTF;
    pub const PURPOSE: &str = "Rare / not fully documented Y-file";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.definitions";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ytf";
    pub const SELECTOR_SYNTAX: &str = "file.ytf@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.definitions", "engine.assets.graph", "engine.scene", "engine.assets.models", "engine.assets.materials", "engine.physics", "engine.ai", "engine.editor", "engine.streaming"];
}

pub mod ytyp {
    pub const EXTENSION: &str = "ytyp";
    pub const ASSET_KIND: &str = "generic_metadata_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YTYP;
    pub const PURPOSE: &str = "Generic XML Metadata Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.metadata";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ytyp";
    pub const SELECTOR_SYNTAX: &str = "file.ytyp@metadata_entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.metadata", "engine.assets.graph", "engine.scene", "engine.model", "engine.materials", "engine.physics", "engine.ai", "engine.editor", "engine.streaming", "engine.ui"];
}

pub mod yvr {
    pub const EXTENSION: &str = "yvr";
    pub const ASSET_KIND: &str = "vehicle_record_list";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YVR;
    pub const PURPOSE: &str = "Vehicle Record List";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.yvr";
    pub const SELECTOR_SYNTAX: &str = "file.yvr@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.models", "engine.physics", "engine.assets.materials", "engine.render"];
}

pub mod ywr {
    pub const EXTENSION: &str = "ywr";
    pub const ASSET_KIND: &str = "waypoint_record_list";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YWR;
    pub const PURPOSE: &str = "Waypoint Record List";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.maps";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ywr";
    pub const SELECTOR_SYNTAX: &str = "file.ywr@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &["engine.assets.maps", "engine.scene", "engine.assets.definitions", "engine.assets.graph", "engine.assets.models", "engine.assets.materials", "engine.assets.textures", "engine.physics", "engine.streaming", "engine.editor"];
}

pub fn specs() -> &'static [Nef8FormatSpec] {
    &[
        Nef8FormatSpec { extension: nemat::EXTENSION, asset_kind: nemat::ASSET_KIND, content_kind: Some(nemat::CONTENT_KIND), semantic_gateway: nemat::SEMANTIC_GATEWAY, handler_service: nemat::HANDLER_SERVICE, selector_syntax: Some(nemat::SELECTOR_SYNTAX), purpose: nemat::PURPOSE, consumer_domains: nemat::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: nepak::EXTENSION, asset_kind: nepak::ASSET_KIND, content_kind: None, semantic_gateway: nepak::SEMANTIC_GATEWAY, handler_service: nepak::HANDLER_SERVICE, selector_syntax: None, purpose: nepak::PURPOSE, consumer_domains: nepak::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: neui::EXTENSION, asset_kind: neui::ASSET_KIND, content_kind: Some(neui::CONTENT_KIND), semantic_gateway: neui::SEMANTIC_GATEWAY, handler_service: neui::HANDLER_SERVICE, selector_syntax: Some(neui::SELECTOR_SYNTAX), purpose: neui::PURPOSE, consumer_domains: neui::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: ybd::EXTENSION, asset_kind: ybd::ASSET_KIND, content_kind: Some(ybd::CONTENT_KIND), semantic_gateway: ybd::SEMANTIC_GATEWAY, handler_service: ybd::HANDLER_SERVICE, selector_syntax: Some(ybd::SELECTOR_SYNTAX), purpose: ybd::PURPOSE, consumer_domains: ybd::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: ybn::EXTENSION, asset_kind: ybn::ASSET_KIND, content_kind: Some(ybn::CONTENT_KIND), semantic_gateway: ybn::SEMANTIC_GATEWAY, handler_service: ybn::HANDLER_SERVICE, selector_syntax: Some(ybn::SELECTOR_SYNTAX), purpose: ybn::PURPOSE, consumer_domains: ybn::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: ycd::EXTENSION, asset_kind: ycd::ASSET_KIND, content_kind: Some(ycd::CONTENT_KIND), semantic_gateway: ycd::SEMANTIC_GATEWAY, handler_service: ycd::HANDLER_SERVICE, selector_syntax: Some(ycd::SELECTOR_SYNTAX), purpose: ycd::PURPOSE, consumer_domains: ycd::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: ydd::EXTENSION, asset_kind: ydd::ASSET_KIND, content_kind: Some(ydd::CONTENT_KIND), semantic_gateway: ydd::SEMANTIC_GATEWAY, handler_service: ydd::HANDLER_SERVICE, selector_syntax: Some(ydd::SELECTOR_SYNTAX), purpose: ydd::PURPOSE, consumer_domains: ydd::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: ydr::EXTENSION, asset_kind: ydr::ASSET_KIND, content_kind: Some(ydr::CONTENT_KIND), semantic_gateway: ydr::SEMANTIC_GATEWAY, handler_service: ydr::HANDLER_SERVICE, selector_syntax: Some(ydr::SELECTOR_SYNTAX), purpose: ydr::PURPOSE, consumer_domains: ydr::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: yed::EXTENSION, asset_kind: yed::ASSET_KIND, content_kind: Some(yed::CONTENT_KIND), semantic_gateway: yed::SEMANTIC_GATEWAY, handler_service: yed::HANDLER_SERVICE, selector_syntax: Some(yed::SELECTOR_SYNTAX), purpose: yed::PURPOSE, consumer_domains: yed::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: yfd::EXTENSION, asset_kind: yfd::ASSET_KIND, content_kind: Some(yfd::CONTENT_KIND), semantic_gateway: yfd::SEMANTIC_GATEWAY, handler_service: yfd::HANDLER_SERVICE, selector_syntax: Some(yfd::SELECTOR_SYNTAX), purpose: yfd::PURPOSE, consumer_domains: yfd::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: neftd::EXTENSION, asset_kind: neftd::ASSET_KIND, content_kind: Some(neftd::CONTENT_KIND), semantic_gateway: neftd::SEMANTIC_GATEWAY, handler_service: neftd::HANDLER_SERVICE, selector_syntax: Some(neftd::SELECTOR_SYNTAX), purpose: neftd::PURPOSE, consumer_domains: neftd::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: yld::EXTENSION, asset_kind: yld::ASSET_KIND, content_kind: Some(yld::CONTENT_KIND), semantic_gateway: yld::SEMANTIC_GATEWAY, handler_service: yld::HANDLER_SERVICE, selector_syntax: Some(yld::SELECTOR_SYNTAX), purpose: yld::PURPOSE, consumer_domains: yld::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: ymap::EXTENSION, asset_kind: ymap::ASSET_KIND, content_kind: Some(ymap::CONTENT_KIND), semantic_gateway: ymap::SEMANTIC_GATEWAY, handler_service: ymap::HANDLER_SERVICE, selector_syntax: Some(ymap::SELECTOR_SYNTAX), purpose: ymap::PURPOSE, consumer_domains: ymap::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: ymf::EXTENSION, asset_kind: ymf::ASSET_KIND, content_kind: Some(ymf::CONTENT_KIND), semantic_gateway: ymf::SEMANTIC_GATEWAY, handler_service: ymf::HANDLER_SERVICE, selector_syntax: Some(ymf::SELECTOR_SYNTAX), purpose: ymf::PURPOSE, consumer_domains: ymf::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: ymt::EXTENSION, asset_kind: ymt::ASSET_KIND, content_kind: Some(ymt::CONTENT_KIND), semantic_gateway: ymt::SEMANTIC_GATEWAY, handler_service: ymt::HANDLER_SERVICE, selector_syntax: Some(ymt::SELECTOR_SYNTAX), purpose: ymt::PURPOSE, consumer_domains: ymt::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: ypdb::EXTENSION, asset_kind: ypdb::ASSET_KIND, content_kind: Some(ypdb::CONTENT_KIND), semantic_gateway: ypdb::SEMANTIC_GATEWAY, handler_service: ypdb::HANDLER_SERVICE, selector_syntax: Some(ypdb::SELECTOR_SYNTAX), purpose: ypdb::PURPOSE, consumer_domains: ypdb::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: ysc::EXTENSION, asset_kind: ysc::ASSET_KIND, content_kind: Some(ysc::CONTENT_KIND), semantic_gateway: ysc::SEMANTIC_GATEWAY, handler_service: ysc::HANDLER_SERVICE, selector_syntax: Some(ysc::SELECTOR_SYNTAX), purpose: ysc::PURPOSE, consumer_domains: ysc::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: ytd::EXTENSION, asset_kind: ytd::ASSET_KIND, content_kind: Some(ytd::CONTENT_KIND), semantic_gateway: ytd::SEMANTIC_GATEWAY, handler_service: ytd::HANDLER_SERVICE, selector_syntax: Some(ytd::SELECTOR_SYNTAX), purpose: ytd::PURPOSE, consumer_domains: ytd::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: ytf::EXTENSION, asset_kind: ytf::ASSET_KIND, content_kind: Some(ytf::CONTENT_KIND), semantic_gateway: ytf::SEMANTIC_GATEWAY, handler_service: ytf::HANDLER_SERVICE, selector_syntax: Some(ytf::SELECTOR_SYNTAX), purpose: ytf::PURPOSE, consumer_domains: ytf::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: ytyp::EXTENSION, asset_kind: ytyp::ASSET_KIND, content_kind: Some(ytyp::CONTENT_KIND), semantic_gateway: ytyp::SEMANTIC_GATEWAY, handler_service: ytyp::HANDLER_SERVICE, selector_syntax: Some(ytyp::SELECTOR_SYNTAX), purpose: ytyp::PURPOSE, consumer_domains: ytyp::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: yvr::EXTENSION, asset_kind: yvr::ASSET_KIND, content_kind: Some(yvr::CONTENT_KIND), semantic_gateway: yvr::SEMANTIC_GATEWAY, handler_service: yvr::HANDLER_SERVICE, selector_syntax: Some(yvr::SELECTOR_SYNTAX), purpose: yvr::PURPOSE, consumer_domains: yvr::CONSUMER_DOMAINS },
        Nef8FormatSpec { extension: ywr::EXTENSION, asset_kind: ywr::ASSET_KIND, content_kind: Some(ywr::CONTENT_KIND), semantic_gateway: ywr::SEMANTIC_GATEWAY, handler_service: ywr::HANDLER_SERVICE, selector_syntax: Some(ywr::SELECTOR_SYNTAX), purpose: ywr::PURPOSE, consumer_domains: ywr::CONSUMER_DOMAINS },
    ]
}

pub fn descriptors() -> Vec<AssetFileTypeDescriptor> {
    specs().iter().copied().map(Nef8FormatSpec::descriptor).collect()
}

pub fn descriptor_for_extension(extension: &str) -> Option<AssetFileTypeDescriptor> {
    let key = AssetFileTypeDescriptor::extension_key(extension);
    specs()
        .iter()
        .copied()
        .find(|spec| AssetFileTypeDescriptor::extension_key(spec.extension) == key)
        .map(Nef8FormatSpec::descriptor)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptors_are_unique_and_valid() {
        let descriptors = descriptors();
        let mut extensions = std::collections::BTreeSet::new();
        assert!(descriptors.len() >= 20);
        for descriptor in descriptors {
            assert!(extensions.insert(descriptor.extension.clone()), "duplicate extension {}", descriptor.extension);
            if descriptor.extension != nepak::EXTENSION {
                assert!(descriptor.validate_generic_rules().is_ok(), "invalid descriptor {}", descriptor.extension);
            }
        }
    }

    #[test]
    fn ytd_descriptor_still_routes_to_texture_domain() {
        let descriptor = descriptor_for_extension("ytd").expect("ytd descriptor");
        assert_eq!(descriptor.extension, ytd::EXTENSION);
        assert_eq!(descriptor.content_kind, Some(ytd::CONTENT_KIND));
        assert_eq!(descriptor.semantic_gateway, ytd::SEMANTIC_GATEWAY);
    }
}

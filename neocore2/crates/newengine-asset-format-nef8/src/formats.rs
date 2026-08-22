//! Canonical format identities and semantic routing constants.
//!
//! Each namespace is intentionally data-only so downstream code can retain the
//! historical `newengine_asset_format_nef8::<format>::...` API through re-export.

pub mod nemat {
    pub const EXTENSION: &str = "nemat";
    pub const ASSET_KIND: &str = "material_library";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_NEMAT;
    pub const CONTENT_SCHEMA_VERSION: u16 = 1;
    /// Canonical authored XML schema emitted by the first-party NEMAT producer.
    /// This is distinct from the material-domain DTO schema.
    pub const AUTHORED_XML_SCHEMA: &str = "newengine.nemat.xmltype.v1";
    /// Existing assets authored before the XMLtype name was frozen remain readable.
    pub const LEGACY_AUTHORED_XML_SCHEMAS: &[&str] = &["newengine.nemat.material_library.v1"];
    pub const AUTHORED_XML_CONTRACT_SPEC: newengine_contract_api::ContractSpec =
        newengine_contract_api::ContractSpec::new(
            "asset.nemat.authored_xml",
            newengine_contract_api::ContractKind::Schema,
            newengine_contract_api::ContractVersion::major(1),
            newengine_contract_api::ContractCompatibility::Exact,
            "newengine-asset-format-nef8",
            Some(AUTHORED_XML_SCHEMA),
        );
    pub const CONTENT_SCHEMA_CONTRACT_SPEC: newengine_contract_api::ContractSpec =
        newengine_contract_api::ContractSpec::new(
            "asset.nemat.schema",
            newengine_contract_api::ContractKind::Schema,
            newengine_contract_api::ContractVersion::major(CONTENT_SCHEMA_VERSION),
            newengine_contract_api::ContractCompatibility::Exact,
            "newengine-asset-format-nef8",
            None,
        );
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

pub mod neitems {
    pub const EXTENSION: &str = "neitems";
    pub const ASSET_KIND: &str = "item_definition_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_NEITEMS;
    pub const PURPOSE: &str = "Authored item, weapon, loadout and inventory definitions";
    pub const SEMANTIC_GATEWAY: &str = "engine.gameplay.inventory";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.neitems";
    pub const SELECTOR_SYNTAX: &str = "file.neitems@item_or_loadout";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.gameplay.inventory",
        "engine.gameplay",
        "engine.ui",
        "engine.assets.graph",
        "engine.editor",
    ];
}

pub mod neui {
    pub const EXTENSION: &str = "neui";
    pub const ASSET_KIND: &str = "ui_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_NEUI;
    pub const CONTENT_SCHEMA_VERSION: u16 = 1;
    pub const CONTENT_SCHEMA_CONTRACT_SPEC: newengine_contract_api::ContractSpec =
        newengine_contract_api::ContractSpec::new(
            "asset.neui.schema",
            newengine_contract_api::ContractKind::Schema,
            newengine_contract_api::ContractVersion::major(CONTENT_SCHEMA_VERSION),
            newengine_contract_api::ContractCompatibility::Exact,
            "newengine-asset-format-nef8",
            None,
        );
    pub const PURPOSE: &str = "NewEngine UI Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.ui";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.neui";
    pub const SELECTOR_SYNTAX: &str = "file.neui@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.ui",
        "engine.assets.graph",
        "engine.ui",
        "engine.ui.text",
        "engine.assets.textures",
        "engine.render",
        "engine.editor",
    ];
}

pub mod ybd {
    pub const EXTENSION: &str = "ybd";
    pub const ASSET_KIND: &str = "bounds_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YBD;
    pub const PURPOSE: &str = "Bounds Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models.collisions";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ybd";
    pub const SELECTOR_SYNTAX: &str = "file.ybd@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.models.collisions",
        "engine.physics",
        "engine.assets.models",
        "engine.scene",
    ];
}

pub mod ybn {
    pub const EXTENSION: &str = "ybn";
    pub const ASSET_KIND: &str = "bounds_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YBN;
    pub const PURPOSE: &str = "Bounds / Collision";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models.collisions";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ybn";
    pub const SELECTOR_SYNTAX: &str = "file.ybn@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.models.collisions",
        "engine.physics",
        "engine.assets.models",
        "engine.scene",
    ];
}

pub mod ycd {
    pub const EXTENSION: &str = "ycd";
    pub const ASSET_KIND: &str = "clip_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YCD;
    pub const PURPOSE: &str = "Animation Clips / Clip Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models.skeletons";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ycd";
    pub const SELECTOR_SYNTAX: &str = "file.ycd@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.models.skeletons",
        "engine.assets.models",
        "engine.scene",
        "engine.render",
    ];
}

pub mod ydd {
    pub const EXTENSION: &str = "ydd";
    pub const ASSET_KIND: &str = "drawable_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YDD;
    /// Read compatibility for resident YDD ListFile envelopes. The binary body
    /// decoder intentionally remains backward-compatible with V2/V3 while V4
    /// is the current producer schema.
    pub const READABLE_CONTENT_SCHEMA_VERSIONS: &[u16] = &[2, 3, 4];
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
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.models",
        "engine.assets.materials",
        "engine.render",
    ];
}

pub mod yed {
    pub const EXTENSION: &str = "yed";
    pub const ASSET_KIND: &str = "expression_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YED;
    pub const PURPOSE: &str = "Expression Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models.skeletons";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.yed";
    pub const SELECTOR_SYNTAX: &str = "file.yed@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.models.skeletons",
        "engine.assets.models",
        "engine.scene",
        "engine.render",
    ];
}

pub mod yfd {
    pub const EXTENSION: &str = "yfd";
    pub const ASSET_KIND: &str = "frame_filter_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YFD;
    pub const PURPOSE: &str = "Frame Filter Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models.skeletons";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.yfd";
    pub const SELECTOR_SYNTAX: &str = "file.yfd@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.models.skeletons",
        "engine.assets.models",
        "engine.scene",
        "engine.render",
    ];
}

pub mod neftd {
    pub const EXTENSION: &str = "neftd";
    pub const ASSET_KIND: &str = "font_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_NEFTD;
    pub const PURPOSE: &str = "Font Dictionary / Typeface Family";
    pub const SEMANTIC_GATEWAY: &str = "engine.ui.text";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.neftd";
    pub const SELECTOR_SYNTAX: &str = "file.neftd@font_face";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.ui.text",
        "engine.ui",
        "engine.assets.ui",
        "engine.render",
        "engine.editor",
    ];
}

pub mod yft {
    pub const EXTENSION: &str = "yft";
    pub const ASSET_KIND: &str = "fragment";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YFT;
    pub const PURPOSE: &str = "Fragment / Vehicle Model";
    pub const SEMANTIC_GATEWAY: &str = "engine.model";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.yft";
    pub const SELECTOR_SYNTAX: &str = "file.yft@fragment_entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.model",
        "engine.assets.definitions",
        "engine.assets.graph",
        "engine.scene",
        "engine.materials",
        "engine.physics",
        "engine.render",
        "engine.editor",
    ];
}

pub mod yld {
    pub const EXTENSION: &str = "yld";
    pub const ASSET_KIND: &str = "cloth_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YLD;
    pub const PURPOSE: &str = "Cloth Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.yld";
    pub const SELECTOR_SYNTAX: &str = "file.yld@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.models.skeletons",
        "engine.assets.models",
        "engine.scene",
        "engine.render",
    ];
}

pub mod ymap {
    pub const EXTENSION: &str = "ymap";
    pub const ASSET_KIND: &str = "discrete_map_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YMAP;
    pub const PURPOSE: &str = "Discrete Map Index / Cell Placements";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.maps";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ymap";
    pub const SELECTOR_SYNTAX: &str = "file.ymap@map | file.ymap@cell/x/z";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.maps",
        "engine.scene",
        "engine.assets.definitions",
        "engine.assets.graph",
        "engine.assets.models",
        "engine.assets.materials",
        "engine.assets.textures",
        "engine.physics",
        "engine.streaming",
        "engine.editor",
    ];
}

pub mod ymf {
    pub const EXTENSION: &str = "ymf";
    pub const ASSET_KIND: &str = "manifest";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YMF;
    pub const PURPOSE: &str = "Manifest / Dependencies";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.graph";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ymf";
    pub const SELECTOR_SYNTAX: &str = "file.ymf@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.graph",
        "engine.assets.definitions",
        "engine.assets.maps",
        "engine.scene",
        "engine.assets.models",
        "engine.assets.materials",
        "engine.assets.textures",
    ];
}

pub mod ymt {
    pub const EXTENSION: &str = "ymt";
    pub const ASSET_KIND: &str = "metadata";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YMT;
    pub const PURPOSE: &str = "Metadata Container";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.definitions";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ymt";
    pub const SELECTOR_SYNTAX: &str = "file.ymt@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.definitions",
        "engine.assets.graph",
        "engine.scene",
        "engine.assets.models",
        "engine.assets.materials",
        "engine.physics",
        "engine.ai",
        "engine.editor",
        "engine.streaming",
    ];
}

pub mod ypdb {
    pub const EXTENSION: &str = "ypdb";
    pub const ASSET_KIND: &str = "pose_database";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YPDB;
    pub const PURPOSE: &str = "Pose Database";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models.skeletons";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ypdb";
    pub const SELECTOR_SYNTAX: &str = "file.ypdb@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.models.skeletons",
        "engine.assets.models",
        "engine.scene",
        "engine.render",
    ];
}

pub mod ysc {
    pub const EXTENSION: &str = "ysc";
    pub const ASSET_KIND: &str = "script_module";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YSC;
    pub const PURPOSE: &str = "Opaque Script Module";
    pub const SEMANTIC_GATEWAY: &str = "engine.scripting";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ysc";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.scripting",
        "engine.scene",
        "engine.assets.definitions",
        "engine.assets.graph",
        "engine.ui",
        "engine.ai",
        "engine.editor",
        "engine.streaming",
    ];
}

pub mod ytd {
    pub const EXTENSION: &str = "ytd";
    pub const ASSET_KIND: &str = "texture_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YTD;
    /// Canonical schema emitted by the current first-party YTD producer.
    /// Historical schema=2 envelopes are migration inputs, not a second current producer contract.
    pub const CONTENT_SCHEMA_VERSION: u16 = 1;
    /// V2 remains a readable migration envelope for resident production assets;
    /// only V1 is emitted by the current first-party producer.
    pub const READABLE_CONTENT_SCHEMA_VERSIONS: &[u16] = &[1, 2];
    pub const CONTENT_SCHEMA_CONTRACT_SPEC: newengine_contract_api::ContractSpec =
        newengine_contract_api::ContractSpec::new(
            "asset.ytd.schema",
            newengine_contract_api::ContractKind::Schema,
            newengine_contract_api::ContractVersion::major(CONTENT_SCHEMA_VERSION),
            newengine_contract_api::ContractCompatibility::Exact,
            "newengine-asset-format-nef8",
            None,
        );
    pub const PURPOSE: &str = "Texture Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ytd";
    pub const SELECTOR_SYNTAX: &str = "file.ytd@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets",
        "engine.materials",
        "engine.model",
        "engine.ui",
        "engine.render",
    ];
}

pub mod ytf {
    pub const EXTENSION: &str = "ytf";
    pub const ASSET_KIND: &str = "unknown_y_file";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YTF;
    pub const PURPOSE: &str = "Rare / not fully documented Y-file";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.definitions";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ytf";
    pub const SELECTOR_SYNTAX: &str = "file.ytf@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.definitions",
        "engine.assets.graph",
        "engine.scene",
        "engine.assets.models",
        "engine.assets.materials",
        "engine.physics",
        "engine.ai",
        "engine.editor",
        "engine.streaming",
    ];
}

pub mod ytyp {
    pub const EXTENSION: &str = "ytyp";
    pub const ASSET_KIND: &str = "archetype_metadata_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YTYP;
    pub const CONTENT_SCHEMA_VERSION: u16 = 1;
    pub const PROPERTIES_SCHEMA_ID: &str = "newengine.ytyp.properties.v1";
    pub const CONTENT_SCHEMA_CONTRACT_SPEC: newengine_contract_api::ContractSpec =
        newengine_contract_api::ContractSpec::new(
            "asset.ytyp.schema",
            newengine_contract_api::ContractKind::Schema,
            newengine_contract_api::ContractVersion::major(CONTENT_SCHEMA_VERSION),
            newengine_contract_api::ContractCompatibility::Exact,
            "newengine-asset-format-nef8",
            Some(PROPERTIES_SCHEMA_ID),
        );
    pub const PURPOSE: &str = "Y-Type Properties / Archetype Metadata Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.definitions";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ytyp";
    pub const SELECTOR_SYNTAX: &str = "file.ytyp@metadata_entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.definitions",
        "engine.assets.graph",
        "engine.scene",
        "engine.model",
        "engine.materials",
        "engine.physics",
        "engine.ai",
        "engine.editor",
        "engine.streaming",
        "engine.ui",
    ];
}

pub mod ytyd {
    pub const EXTENSION: &str = "ytyd";
    pub const ASSET_KIND: &str = "uv_layout_dictionary";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YTYD;
    pub const PURPOSE: &str = "UV Layout / Unwrap Dictionary";
    pub const SEMANTIC_GATEWAY: &str = "engine.model";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ytyd";
    pub const SELECTOR_SYNTAX: &str = "file.ytyd@uv_layout_entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.model",
        "engine.assets.definitions",
        "engine.assets.graph",
        "engine.scene",
        "engine.materials",
        "engine.render",
        "engine.editor",
    ];
}

pub mod yvr {
    pub const EXTENSION: &str = "yvr";
    pub const ASSET_KIND: &str = "vehicle_record_list";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YVR;
    pub const PURPOSE: &str = "Vehicle Record List";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.models";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.yvr";
    pub const SELECTOR_SYNTAX: &str = "file.yvr@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.models",
        "engine.physics",
        "engine.assets.materials",
        "engine.render",
    ];
}

pub mod ywr {
    pub const EXTENSION: &str = "ywr";
    pub const ASSET_KIND: &str = "waypoint_record_list";
    pub const CONTENT_KIND: u32 = newengine_assets_api::LIST_FILE_CONTENT_KIND_YWR;
    pub const PURPOSE: &str = "Waypoint Record List";
    pub const SEMANTIC_GATEWAY: &str = "engine.assets.maps";
    pub const HANDLER_SERVICE: &str = "asset.codec.listfile.ywr";
    pub const SELECTOR_SYNTAX: &str = "file.ywr@entry";
    pub const CONSUMER_DOMAINS: &[&str] = &[
        "engine.assets.maps",
        "engine.scene",
        "engine.assets.definitions",
        "engine.assets.graph",
        "engine.assets.models",
        "engine.assets.materials",
        "engine.assets.textures",
        "engine.physics",
        "engine.streaming",
        "engine.editor",
    ];
}

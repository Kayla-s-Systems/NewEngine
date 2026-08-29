use super::helpers::{normalize_name, sanitize_tag};
use super::*;

pub(crate) struct NeUiTagSpec {
    pub kind: UiRuntimeNodeKind,
    pub implicit_tags: &'static [&'static str],
    pub add_source_tag: bool,
    pub add_normalized_tag: bool,
}

struct TagAliasGroup {
    aliases: &'static [&'static str],
    spec: NeUiTagSpec,
}

pub(super) const METADATA_ELEMENTS: &[&str] = &[
    "entries",
    "entry",
    "surface",
    "dependencies",
    "themeref",
    "componentref",
    "textureref",
    "fontref",
    "soundref",
    "bindinggraph",
    "statesource",
    "bind",
    "actionmap",
    "action",
    "event",
    "payload",
    "slot",
    "uinodenavigationdocument",
    "page",
    "footer",
    "line",
    "navleft",
    "navright",
    "back",
];

pub(super) const STRUCTURAL_ATTRS: &[&str] = &[
    "id",
    "name",
    "class",
    "role",
    "text",
    "label",
    "title",
    "detail",
    "subtitle",
    "value",
    "icon",
    "texture",
    "font",
    "font_token",
    "tooltip",
    "hidden",
    "visible",
    "enabled",
    "interactive",
    "tone",
    "action",
    "action_id",
    "command",
    "use",
    "component",
    "template",
];

const TAG_ALIASES: &[TagAliasGroup] = &[
    TagAliasGroup {
        aliases: &["surface"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Surface,
            implicit_tags: &[UI_COMPONENT_SURFACE],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &[
            "panel",
            "card",
            "statuscard",
            "metriccard",
            "warningcard",
            "plugincard",
            "propertycard",
        ],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Panel,
            implicit_tags: &[],
            add_source_tag: true,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["stack"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Stack,
            implicit_tags: &[UI_COMPONENT_STACK],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["row"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Row,
            implicit_tags: &[UI_COMPONENT_ROW],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["column", "col"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Column,
            implicit_tags: &["column"],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["grid"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Grid,
            implicit_tags: &[UI_COMPONENT_GRID],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["text", "label"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Text,
            implicit_tags: &[UI_COMPONENT_TEXT],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["button"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Button,
            implicit_tags: &[UI_COMPONENT_BUTTON],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &[
            "action",
            "option",
            "item",
            "selectitem",
            "dropdownitem",
            "menuitem",
        ],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Action,
            implicit_tags: &[UI_COMPONENT_ACTION, "select-option", "option"],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["input", "textinput", "field", "search"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Input,
            implicit_tags: &[UI_COMPONENT_INPUT],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["checkbox", "check"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Checkbox,
            implicit_tags: &[UI_COMPONENT_CHECKBOX],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["toggle", "switch"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Toggle,
            implicit_tags: &[UI_COMPONENT_TOGGLE],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["slider", "progress", "progressbar"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Slider,
            implicit_tags: &[UI_COMPONENT_SLIDER],
            add_source_tag: false,
            add_normalized_tag: true,
        },
    },
    TagAliasGroup {
        aliases: &["scrollbar"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::ScrollBar,
            implicit_tags: &[UI_COMPONENT_SCROLL_BAR],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["select", "dropdown", "combobox"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Select,
            implicit_tags: &[UI_COMPONENT_SELECT],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["separator", "divider"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Separator,
            implicit_tags: &[UI_COMPONENT_SEPARATOR],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["list", "propertygrid"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::List,
            implicit_tags: &[UI_COMPONENT_LIST],
            add_source_tag: true,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["tree"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Tree,
            implicit_tags: &[UI_COMPONENT_TREE],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["split", "splitter"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Split,
            implicit_tags: &["split"],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["viewport"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Viewport,
            implicit_tags: &[UI_COMPONENT_VIEWPORT],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["image", "texture", "externaltexture", "icon"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::ExternalTexture,
            implicit_tags: &[UI_COMPONENT_EXTERNAL_TEXTURE],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
    TagAliasGroup {
        aliases: &["spacer"],
        spec: NeUiTagSpec {
            kind: UiRuntimeNodeKind::Spacer,
            implicit_tags: &[UI_COMPONENT_SPACER],
            add_source_tag: false,
            add_normalized_tag: false,
        },
    },
];

pub(crate) fn is_metadata_element(name: &str) -> bool {
    let normalized = normalize_name(name);
    METADATA_ELEMENTS.contains(&normalized.as_str())
}

pub(crate) fn kind_for_tag(name: &str) -> (UiRuntimeNodeKind, Vec<String>) {
    let normalized = normalize_name(name);
    let source_tag = sanitize_tag(name);

    for group in TAG_ALIASES {
        if group.aliases.contains(&normalized.as_str()) {
            let mut tags = group
                .spec
                .implicit_tags
                .iter()
                .map(|tag| (*tag).to_owned())
                .collect::<Vec<_>>();
            if group.spec.add_source_tag && !source_tag.is_empty() {
                tags.push(source_tag.clone());
            }
            if group.spec.add_normalized_tag && !normalized.is_empty() {
                tags.push(normalized.clone());
            }
            tags.sort();
            tags.dedup();
            return (group.spec.kind, tags);
        }
    }

    (
        UiRuntimeNodeKind::Panel,
        vec!["custom".to_owned(), source_tag],
    )
}

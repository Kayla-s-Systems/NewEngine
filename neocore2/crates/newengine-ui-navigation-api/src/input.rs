use serde::{Deserialize, Serialize};

use crate::{UiNodeActionRoute, UiNodeFeedbackEvent, UiNodeTransition};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiNodeSelectionState {
    pub page: String,
    #[serde(default)]
    pub selected_index: usize,
    #[serde(default)]
    pub hovered_index: Option<usize>,
}

#[derive(Debug, Clone, Default)]
pub struct UiNodeHitTestState {
    pub hovered_index: Option<usize>,
    pub pointer_primary_pressed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UiNodeNavigationInput {
    pub nav_x: i8,
    pub nav_y: i8,
    pub accept: bool,
    pub back: bool,
    pub hit_test: Option<UiNodeHitTestState>,
}

#[derive(Debug, Clone)]
pub struct UiNodeRouteDispatch {
    pub route: UiNodeActionRoute,
    pub source_item_id: Option<String>,
    pub source_label: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UiNodeNavigationOutput {
    pub selection_changed: bool,
    pub route_dispatches: Vec<UiNodeRouteDispatch>,
    pub feedback: Vec<UiNodeFeedbackEvent>,
    pub transition: Option<UiNodeTransition>,
    pub close_requested: bool,
}

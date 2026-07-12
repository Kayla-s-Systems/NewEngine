use std::collections::BTreeMap;

use crate::{
    UiNodeActionRoute, UiNodeNavigationDocument, UiNodeNavigationInput, UiNodeNavigationItem,
    UiNodeNavigationOutput, UiNodeNavigationPage, UiNodeRouteDispatch, UiNodeSelectionState,
    UiNodeTransition, UiNodeTransitionKind,
};

#[derive(Debug, Clone)]
pub struct UiNodeNavigationRuntime {
    document: UiNodeNavigationDocument,
    current_page: String,
    selected_by_page: BTreeMap<String, usize>,
    hovered_index: Option<usize>,
}

impl UiNodeNavigationRuntime {
    pub fn new(document: UiNodeNavigationDocument) -> Result<Self, String> {
        document.validate()?;
        let current_page = document.root_page.clone();
        Ok(Self {
            document,
            current_page,
            selected_by_page: BTreeMap::new(),
            hovered_index: None,
        })
    }

    #[inline]
    pub fn document(&self) -> &UiNodeNavigationDocument {
        &self.document
    }

    #[inline]
    pub fn reset_to_root(&mut self) {
        self.current_page = self.document.root_page.clone();
        self.hovered_index = None;
    }

    #[inline]
    pub fn current_page_id(&self) -> &str {
        &self.current_page
    }

    #[inline]
    pub fn current_page(&self) -> Option<&UiNodeNavigationPage> {
        self.document.page(&self.current_page)
    }

    #[inline]
    pub fn current_items(&self) -> &[UiNodeNavigationItem] {
        self.current_page()
            .map(|page| page.items.as_slice())
            .unwrap_or(&[])
    }

    #[inline]
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    #[inline]
    pub fn selected_index(&self) -> usize {
        *self.selected_by_page.get(&self.current_page).unwrap_or(&0)
    }

    #[inline]
    pub fn selection_state(&self) -> UiNodeSelectionState {
        UiNodeSelectionState {
            page: self.current_page.clone(),
            selected_index: self.selected_index(),
            hovered_index: self.hovered_index,
        }
    }

    pub fn handle_input(&mut self, input: UiNodeNavigationInput) -> UiNodeNavigationOutput {
        let mut output = UiNodeNavigationOutput::default();
        let item_count = self.current_items().len();
        if item_count == 0 {
            return output;
        }

        if input.back {
            self.activate_back(&mut output);
            return output;
        }

        if let Some(hit) = input.hit_test {
            self.hovered_index = hit.hovered_index.filter(|index| *index < item_count);
            if let Some(hovered) = self.hovered_index {
                if hovered != self.selected_index() {
                    self.set_selected_index(hovered);
                    output.selection_changed = true;
                }
            }
            if hit.pointer_primary_pressed && self.hovered_index.is_some() {
                self.activate_selected(&mut output);
                return output;
            }
        } else {
            self.hovered_index = None;
        }

        if input.nav_y != 0 {
            let direction = if input.nav_y > 0 { 1 } else { -1 };
            if self.move_selection(direction) {
                output.selection_changed = true;
            }
        }

        if input.nav_x < 0 {
            self.dispatch_selected_nav_route(UiNodeNavigationDirection::Left, &mut output);
            return output;
        }
        if input.nav_x > 0 {
            self.dispatch_selected_nav_route(UiNodeNavigationDirection::Right, &mut output);
            return output;
        }

        if input.accept {
            self.activate_selected(&mut output);
        }

        output
    }

    fn activate_selected(&mut self, output: &mut UiNodeNavigationOutput) {
        let Some(item) = self.current_items().get(self.selected_index()).cloned() else {
            return;
        };
        let Some(route) = item.action.clone() else {
            return;
        };
        self.dispatch_route(route, Some(item), output);
    }

    fn dispatch_selected_nav_route(
        &mut self,
        direction: UiNodeNavigationDirection,
        output: &mut UiNodeNavigationOutput,
    ) {
        let Some(item) = self.current_items().get(self.selected_index()).cloned() else {
            return;
        };
        let route = match direction {
            UiNodeNavigationDirection::Left => item.nav_left.clone(),
            UiNodeNavigationDirection::Right => item.nav_right.clone(),
        };
        let Some(route) = route else {
            return;
        };
        self.dispatch_route(route, Some(item), output);
    }

    fn activate_back(&mut self, output: &mut UiNodeNavigationOutput) {
        if let Some(route) = self.current_page().and_then(|page| page.back_route.clone()) {
            self.dispatch_route(route, None, output);
            return;
        }
        let transition = if self.current_page_id() == self.document.root_page {
            UiNodeTransition::close()
        } else if let Some(parent) = self
            .current_page()
            .and_then(|page| page.parent_page.clone())
        {
            UiNodeTransition::open_page(parent)
        } else {
            UiNodeTransition::close()
        };
        self.apply_transition(&transition, output);
    }

    fn dispatch_route(
        &mut self,
        route: UiNodeActionRoute,
        item: Option<UiNodeNavigationItem>,
        output: &mut UiNodeNavigationOutput,
    ) {
        if let Some(feedback) = route.feedback.clone() {
            output.feedback.push(feedback);
        }
        if let Some(transition) = route.transition.clone() {
            self.apply_transition(&transition, output);
        }
        output.route_dispatches.push(UiNodeRouteDispatch {
            route,
            source_item_id: item.as_ref().map(|item| item.id.clone()),
            source_label: item.as_ref().map(|item| item.label.clone()),
        });
    }

    fn apply_transition(
        &mut self,
        transition: &UiNodeTransition,
        output: &mut UiNodeNavigationOutput,
    ) {
        output.transition = Some(transition.clone());
        match transition.kind {
            UiNodeTransitionKind::None => {}
            UiNodeTransitionKind::OpenPage => {
                if let Some(page) = transition.page.as_deref() {
                    if self.document.page(page).is_some() {
                        self.current_page = page.to_owned();
                        self.hovered_index = None;
                        if transition.reset_selection {
                            self.set_selected_index(0);
                        }
                    }
                }
            }
            UiNodeTransitionKind::Back => {
                if let Some(parent) = self
                    .current_page()
                    .and_then(|page| page.parent_page.clone())
                {
                    if self.document.page(&parent).is_some() {
                        self.current_page = parent;
                        self.hovered_index = None;
                    }
                } else {
                    output.close_requested = true;
                }
            }
            UiNodeTransitionKind::Close => {
                output.close_requested = true;
                self.current_page = self.document.root_page.clone();
                self.hovered_index = None;
                if transition.reset_selection {
                    self.set_selected_index(0);
                }
            }
        }
    }

    fn move_selection(&mut self, delta: i32) -> bool {
        let len = self.current_items().len();
        if len == 0 {
            return false;
        }
        let current = self.selected_index() as i32;
        let next = (current + delta).rem_euclid(len as i32) as usize;
        if next == self.selected_index() {
            return false;
        }
        self.set_selected_index(next);
        true
    }

    #[inline]
    fn set_selected_index(&mut self, value: usize) {
        let max = self.current_items().len().saturating_sub(1);
        self.selected_by_page
            .insert(self.current_page.clone(), value.min(max));
    }
}

#[derive(Debug, Clone, Copy)]
enum UiNodeNavigationDirection {
    Left,
    Right,
}

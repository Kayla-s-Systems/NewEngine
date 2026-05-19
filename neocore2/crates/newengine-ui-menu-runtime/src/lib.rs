#![forbid(unsafe_op_in_unsafe_fn)]

use newengine_ui_navigation_api::{
    MenuActionRoute, MenuDocument, MenuFeedbackEvent, MenuItem, MenuPage, MenuSelectionState,
    MenuTransition, MenuTransitionKind,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct MenuHitTestState {
    pub hovered_index: Option<usize>,
    pub pointer_primary_pressed: bool,
}

#[derive(Debug, Clone, Default)]
pub struct MenuRuntimeInput {
    pub nav_x: i8,
    pub nav_y: i8,
    pub accept: bool,
    pub back: bool,
    pub hit_test: Option<MenuHitTestState>,
}

#[derive(Debug, Clone)]
pub struct MenuRouteDispatch {
    pub route: MenuActionRoute,
    pub source_item_id: Option<String>,
    pub source_label: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MenuRuntimeOutput {
    pub selection_changed: bool,
    pub route_dispatches: Vec<MenuRouteDispatch>,
    pub feedback: Vec<MenuFeedbackEvent>,
    pub transition: Option<MenuTransition>,
    pub close_requested: bool,
}

#[derive(Debug, Clone)]
pub struct MenuRuntime {
    document: MenuDocument,
    current_page: String,
    selected_by_page: BTreeMap<String, usize>,
    hovered_index: Option<usize>,
}

impl MenuRuntime {
    pub fn new(document: MenuDocument) -> Result<Self, String> {
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
    pub fn document(&self) -> &MenuDocument {
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
    pub fn current_page(&self) -> Option<&MenuPage> {
        self.document.page(&self.current_page)
    }

    #[inline]
    pub fn current_items(&self) -> &[MenuItem] {
        self.current_page().map(|page| page.items.as_slice()).unwrap_or(&[])
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
    pub fn selection_state(&self) -> MenuSelectionState {
        MenuSelectionState {
            page: self.current_page.clone(),
            selected_index: self.selected_index(),
            hovered_index: self.hovered_index,
        }
    }

    pub fn handle_input(&mut self, input: MenuRuntimeInput) -> MenuRuntimeOutput {
        let mut output = MenuRuntimeOutput::default();
        let item_count = self.current_items().len();
        if item_count == 0 {
            return output;
        }

        if input.back {
            self.activate_back(&mut output);
            return output;
        }

        if let Some(hit) = input.hit_test {
            self.hovered_index = hit.hovered_index.filter(|idx| *idx < item_count);
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
            let dir = if input.nav_y > 0 { 1 } else { -1 };
            if self.move_selection(dir) {
                output.selection_changed = true;
            }
        }

        if input.nav_x < 0 {
            self.dispatch_selected_nav_route(Direction::Left, &mut output);
            return output;
        }
        if input.nav_x > 0 {
            self.dispatch_selected_nav_route(Direction::Right, &mut output);
            return output;
        }

        if input.accept {
            self.activate_selected(&mut output);
        }

        output
    }

    fn activate_selected(&mut self, output: &mut MenuRuntimeOutput) {
        let Some(item) = self.current_items().get(self.selected_index()).cloned() else { return; };
        let Some(route) = item.action.clone() else { return; };
        self.dispatch_route(route, Some(item), output);
    }

    fn dispatch_selected_nav_route(&mut self, direction: Direction, output: &mut MenuRuntimeOutput) {
        let Some(item) = self.current_items().get(self.selected_index()).cloned() else { return; };
        let route = match direction {
            Direction::Left => item.nav_left.clone(),
            Direction::Right => item.nav_right.clone(),
        };
        let Some(route) = route else { return; };
        self.dispatch_route(route, Some(item), output);
    }

    fn activate_back(&mut self, output: &mut MenuRuntimeOutput) {
        if let Some(route) = self.current_page().and_then(|page| page.back_route.clone()) {
            self.dispatch_route(route, None, output);
            return;
        }
        let transition = if self.current_page_id() == self.document.root_page {
            MenuTransition::close()
        } else if let Some(parent) = self.current_page().and_then(|page| page.parent_page.clone()) {
            MenuTransition::open_page(parent)
        } else {
            MenuTransition::close()
        };
        self.apply_transition(&transition, output);
    }

    fn dispatch_route(
        &mut self,
        route: MenuActionRoute,
        item: Option<MenuItem>,
        output: &mut MenuRuntimeOutput,
    ) {
        if let Some(feedback) = route.feedback.clone() {
            output.feedback.push(feedback);
        }
        if let Some(transition) = route.transition.clone() {
            self.apply_transition(&transition, output);
        }
        output.route_dispatches.push(MenuRouteDispatch {
            route,
            source_item_id: item.as_ref().map(|item| item.id.clone()),
            source_label: item.as_ref().map(|item| item.label.clone()),
        });
    }

    fn apply_transition(&mut self, transition: &MenuTransition, output: &mut MenuRuntimeOutput) {
        output.transition = Some(transition.clone());
        match transition.kind {
            MenuTransitionKind::None => {}
            MenuTransitionKind::OpenPage => {
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
            MenuTransitionKind::Back => {
                if let Some(parent) = self.current_page().and_then(|page| page.parent_page.clone()) {
                    if self.document.page(&parent).is_some() {
                        self.current_page = parent;
                        self.hovered_index = None;
                    }
                } else {
                    output.close_requested = true;
                }
            }
            MenuTransitionKind::Close => {
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
enum Direction {
    Left,
    Right,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> MenuDocument {
        MenuDocument::from_json_str(r#"{
          "id":"engine.pause_menu",
          "surface_id":"engine.pause_menu",
          "root_page":"root",
          "pages":[
            {"id":"root","items":[
              {"id":"settings","label":"Settings","action":{"id":"open.settings","source":"settings","target":"MenuRuntime","event":"menu.open_page","transition":{"kind":"open_page","page":"settings"}}}
            ]},
            {"id":"settings","parent_page":"root","items":[{"id":"back","label":"Back"}]}
          ]
        }"#).unwrap()
    }

    #[test]
    fn route_transition_opens_page() {
        let mut menu = MenuRuntime::new(doc()).unwrap();
        let out = menu.handle_input(MenuRuntimeInput { accept: true, ..Default::default() });
        assert_eq!(menu.current_page_id(), "settings");
        assert_eq!(out.route_dispatches.len(), 1);
    }
}

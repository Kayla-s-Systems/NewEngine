impl SceneBridge {
    pub fn apply_editor_selection_actions(
        &self,
        frame: &UiEventDispatchFrame,
        additive: bool,
    ) -> bool {
        let mut applied = false;
        for action in &frame.actions {
            if action.trigger != UiNodeEventTrigger::Click {
                continue;
            }
            let Some(entity_key) =
                selection_entity_key_from_action(action.action_id.as_str(), &action.payload)
            else {
                continue;
            };
            if let Some(entity) = self.entity_by_stable_key(entity_key) {
                if additive {
                    self.toggle_selection(entity);
                } else {
                    self.set_selection(Some(entity));
                }
                newengine_ulog_api::ulog::info!(
                    "editor selection: selected entity={:?} stable_key={} via action_id='{}' surface='{}' node='{}' route='engine.editor.selection.select_entity'",
                    entity,
                    entity_key,
                    action.action_id,
                    action.surface_id,
                    action.node_id
                );
                applied = true;
            } else {
                newengine_ulog_api::ulog::warn!(
                    "editor selection: action_id='{}' requested missing entity stable_key={} surface='{}' node='{}'",
                    action.action_id,
                    entity_key,
                    action.surface_id,
                    action.node_id
                );
            }
        }
        applied
    }

    pub fn apply_in_game_editor_actions(&self, frame: &UiEventDispatchFrame) -> bool {
        let mut applied = false;
        for action in &frame.actions {
            if action.trigger == UiNodeEventTrigger::Click {
                match action.action_id.as_str() {
                    IN_GAME_EDITOR_TOGGLE_ACTION => {
                        self.toggle_in_game_editor();
                        applied = true;
                        continue;
                    }
                    IN_GAME_EDITOR_CLOSE_ACTION => {
                        self.set_in_game_editor_enabled(false);
                        applied = true;
                        continue;
                    }
                    IN_GAME_EDITOR_SAVE_ACTION if self.in_game_editor_enabled() => {
                        match self.save_authored_project_world() {
                            Ok(count) => newengine_ulog_api::ulog::info!(
                                "in-game editor: project save complete placements={count}"
                            ),
                            Err(error) => newengine_ulog_api::ulog::error!(
                                "in-game editor: project save failed err='{}'",
                                error
                            ),
                        }
                        applied = true;
                        continue;
                    }
                    _ => {}
                }
            }

            if !self.in_game_editor_enabled() || action.trigger != UiNodeEventTrigger::ValueChanged
            {
                continue;
            }
            let Some(field) = TransformEditField::parse(action.action_id.as_str()) else {
                continue;
            };
            let Some(value) = action_payload_f32(&action.payload) else {
                continue;
            };
            if self.apply_selected_transform_field(field, value) {
                applied = true;
            }
        }
        applied
    }

    pub fn apply_editor_actor_actions(&self, frame: &UiEventDispatchFrame) -> bool {
        let mut applied = false;
        for action in &frame.actions {
            if action.trigger != UiNodeEventTrigger::Click {
                continue;
            }
            match action.action_id.as_str() {
                "editor.actor.duplicate" => {
                    applied |= !self.duplicate_selected_actors().is_empty();
                }
                "editor.actor.delete" => {
                    applied |= self.delete_selected_actors() > 0;
                }
                _ => {}
            }
        }
        applied
    }
}

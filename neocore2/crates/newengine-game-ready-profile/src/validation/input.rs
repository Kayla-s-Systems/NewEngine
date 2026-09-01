use super::*;

struct HardwareUiInputSource<'a>(&'a newengine_ui_api::UiInputFrame);

impl newengine_input_actions_api::InputFrameSource for HardwareUiInputSource<'_> {
    fn is_key_down(&self, key: u32) -> bool {
        self.0.is_key_down(key)
    }
    fn is_key_pressed(&self, key: u32) -> bool {
        self.0.is_key_pressed(key)
    }
    fn is_key_released(&self, key: u32) -> bool {
        self.0.is_key_released(key)
    }
    fn is_mouse_down(&self, button: u32) -> bool {
        self.0.is_mouse_down(button)
    }
    fn is_mouse_pressed(&self, button: u32) -> bool {
        self.0.is_mouse_pressed(button)
    }
    fn is_mouse_released(&self, button: u32) -> bool {
        self.0.is_mouse_released(button)
    }
    fn has_gamepad_connected(&self) -> bool {
        self.0.has_gamepad_connected()
    }
    fn is_gamepad_button_down(&self, button: &str) -> bool {
        self.0.is_gamepad_button_down(button)
    }
    fn is_gamepad_button_pressed(&self, button: &str) -> bool {
        self.0.is_gamepad_button_pressed(button)
    }
    fn is_gamepad_button_released(&self, button: &str) -> bool {
        self.0.is_gamepad_button_released(button)
    }
    fn gamepad_axis(&self, axis: &str) -> f32 {
        self.0.gamepad_axis(axis)
    }
}

impl GameReadyValidationModule {
    fn input_call(method: &str, payload: &serde_json::Value) -> Result<serde_json::Value, String> {
        if !newengine_plugin_host::has_service(newengine_input_api::ENGINE_INPUT_SERVICE_ID) {
            return Err(format!(
                "required input gateway '{}' is unavailable",
                newengine_input_api::ENGINE_INPUT_SERVICE_ID
            ));
        }
        let bytes = serde_json::to_vec(payload).map_err(|error| error.to_string())?;
        let host = newengine_plugin_host::default_host_api();
        let response = (host.call_service_v1)(
            RString::from(newengine_input_api::ENGINE_INPUT_SERVICE_ID),
            MethodName::from(method),
            Blob::from(bytes),
        )
        .into_result()
        .map_err(|error| error.to_string())?
        .into_vec();
        if response.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_slice(&response).map_err(|error| error.to_string())
    }

    fn input_ingest(topic: &str, data: serde_json::Value) -> Result<(), String> {
        Self::input_call(
            newengine_input_api::INPUT_METHOD_INGEST_JSON,
            &serde_json::json!({ "topic": topic, "data": data }),
        )?;
        Ok(())
    }

    fn input_state() -> Result<serde_json::Value, String> {
        Self::input_call(
            newengine_input_api::INPUT_METHOD_STATE_JSON,
            &serde_json::Value::Null,
        )
    }

    fn gamepad_device(connected: bool) -> Result<(), String> {
        Self::input_ingest(
            "test.device",
            serde_json::json!({
                "id": "game-ready-controller",
                "kind": "gamepad",
                "connected": connected,
            }),
        )
    }

    fn gamepad_button(button: &str, pressed: bool) -> Result<(), String> {
        Self::input_ingest(
            "test.gamepad.button",
            serde_json::json!({
                "id": "game-ready-controller",
                "button": button,
                "state": if pressed { "pressed" } else { "released" },
            }),
        )
    }

    pub(super) fn wait(&mut self, frames: u32) {
        self.delay_frames = frames;
    }

    pub(super) fn controller_update<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
    ) -> Result<bool, String> {
        if self.delay_frames > 0 {
            self.delay_frames -= 1;
            return Ok(false);
        }
        let state_id = Self::flow_state_id(ctx);
        let state_id = state_id.as_deref();
        match self.step {
            0 if state_id == Some("main_menu") => {
                Self::gamepad_device(true)?;
                self.step = 1;
                self.wait(3);
            }
            1 if state_id == Some("main_menu") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::DPAD_DOWN, true)?;
                self.step = 2;
                self.wait(3);
            }
            2 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::DPAD_DOWN, false)?;
                self.step = 3;
                self.wait(3);
            }
            3 if state_id == Some("main_menu") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::SOUTH, true)?;
                self.step = 4;
                self.wait(3);
            }
            4 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::SOUTH, false)?;
                self.step = 5;
            }
            5 if state_id == Some("gameplay") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::START, true)?;
                self.step = 6;
                self.wait(3);
            }
            6 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::START, false)?;
                self.step = 7;
            }
            7 if state_id == Some("pause") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::DPAD_DOWN, true)?;
                self.step = 8;
                self.wait(3);
            }
            8 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::DPAD_DOWN, false)?;
                self.step = 9;
                self.wait(3);
            }
            9 if state_id == Some("pause") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::DPAD_DOWN, true)?;
                self.step = 10;
                self.wait(3);
            }
            10 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::DPAD_DOWN, false)?;
                self.step = 11;
                self.wait(3);
            }
            11 if state_id == Some("pause") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::SOUTH, true)?;
                self.step = 12;
                self.wait(3);
            }
            12 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::SOUTH, false)?;
                self.step = 13;
            }
            13 if state_id == Some("pause_settings") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::EAST, true)?;
                self.step = 14;
                self.wait(3);
            }
            14 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::EAST, false)?;
                self.step = 15;
            }
            15 if state_id == Some("pause") => {
                Self::gamepad_button(newengine_input_api::gamepad_button::START, true)?;
                self.step = 16;
                self.wait(3);
            }
            16 => {
                Self::gamepad_button(newengine_input_api::gamepad_button::START, false)?;
                self.step = 17;
            }
            17 if state_id == Some("gameplay") => {
                Self::gamepad_device(false)?;
                newengine_ulog_api::ulog::info!(
                    "game-ready validation: controller-only flow complete path='main_menu->gameplay->pause->pause_settings->pause->gameplay' input='virtual gamepad only'"
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }

    pub(super) fn hardware_controller_update<E: Send + 'static>(
        &mut self,
        ctx: &ModuleCtx<'_, E>,
    ) -> Result<bool, String> {
        use newengine_input_api::{gamepad_axis, gamepad_button};
        use newengine_input_profile_gameready::action;

        let Some(input) = ctx.resources().get::<newengine_ui_api::UiInputFrame>() else {
            return Ok(false);
        };

        if !self.hardware_controller.physical_confirmed {
            if input.gamepad_connected == 0 {
                if !self.hardware_controller.waiting_logged {
                    self.hardware_controller.waiting_logged = true;
                    newengine_ulog_api::ulog::info!(
                        "game-ready hardware gamepad: waiting for physical XInput controller virtual_devices='rejected'"
                    );
                }
                return Ok(false);
            }
            let snapshot = Self::input_state()?;
            let physical = snapshot["devices"]
                .as_object()
                .and_then(|devices| {
                    devices.iter().find(|(_, device)| {
                        device["kind"].as_str() == Some("gamepad")
                            && device["connected"].as_bool() == Some(true)
                            && device["virtual"].as_bool() != Some(true)
                    })
                })
                .map(|(id, device)| {
                    (
                        id.clone(),
                        device["name"].as_str().unwrap_or("unknown").to_owned(),
                    )
                });
            let Some((id, name)) = physical else {
                return Err(format!(
                    "UiInputFrame reports {} connected gamepad(s), but engine.input exposes no connected non-virtual gamepad",
                    input.gamepad_connected
                ));
            };
            self.hardware_controller.physical_confirmed = true;
            newengine_ulog_api::ulog::info!(
                "game-ready hardware gamepad: physical controller accepted id='{}' name='{}' backend='engine.input/xinput'",
                id,
                name,
            );
        }

        let actions =
            newengine_input_bindings_runtime::resolve_input_actions(&HardwareUiInputSource(input));
        let commands = actions.command_actions();
        let gameplay_active = Self::gameplay_active(ctx);

        if gameplay_active && !self.hardware_controller.movement_seen {
            let raw = [
                input.gamepad_axis(gamepad_axis::LEFT_STICK_X),
                input.gamepad_axis(gamepad_axis::LEFT_STICK_Y),
            ];
            if raw[0].abs().max(raw[1].abs()) >= 0.55 {
                if actions.move_axis[0].abs().max(actions.move_axis[2].abs()) <= 0.05 {
                    return Err(format!(
                        "physical left stick telemetry did not resolve to movement raw={raw:?} semantic={:?}",
                        actions.move_axis
                    ));
                }
                self.hardware_controller.movement_seen = true;
                newengine_ulog_api::ulog::info!(
                    "game-ready hardware gamepad: movement PASS raw=({:.3},{:.3}) semantic=({:.3},{:.3},{:.3})",
                    raw[0], raw[1], actions.move_axis[0], actions.move_axis[1], actions.move_axis[2]
                );
            }
        }

        if gameplay_active && !self.hardware_controller.look_seen {
            let raw = [
                input.gamepad_axis(gamepad_axis::RIGHT_STICK_X),
                input.gamepad_axis(gamepad_axis::RIGHT_STICK_Y),
            ];
            if raw[0].abs().max(raw[1].abs()) >= 0.55 {
                if actions.look_axis[0].abs().max(actions.look_axis[1].abs()) <= 0.05 {
                    return Err(format!(
                        "physical right stick telemetry did not resolve to look raw={raw:?} semantic={:?}",
                        actions.look_axis
                    ));
                }
                self.hardware_controller.look_seen = true;
                newengine_ulog_api::ulog::info!(
                    "game-ready hardware gamepad: camera look PASS raw=({:.3},{:.3}) semantic=({:.3},{:.3})",
                    raw[0], raw[1], actions.look_axis[0], actions.look_axis[1]
                );
            }
        }

        if gameplay_active
            && !self.hardware_controller.fire_seen
            && input.is_gamepad_button_down(gamepad_button::RIGHT_TRIGGER_2)
        {
            if !commands.is_held(action::PLAYER_FIRE_PRIMARY) {
                return Err("physical RT did not resolve to held player.fire.primary".to_owned());
            }
            self.hardware_controller.fire_seen = true;
            newengine_ulog_api::ulog::info!("game-ready hardware gamepad: fire PASS control='RT'");
        }

        if gameplay_active
            && !self.hardware_controller.aim_seen
            && input.is_gamepad_button_down(gamepad_button::LEFT_TRIGGER_2)
        {
            if !commands.is_held(action::PLAYER_AIM) {
                return Err("physical LT did not resolve to held player.aim".to_owned());
            }
            self.hardware_controller.aim_seen = true;
            newengine_ulog_api::ulog::info!("game-ready hardware gamepad: aim PASS control='LT'");
        }

        if gameplay_active && input.is_gamepad_button_pressed(gamepad_button::LEFT_TRIGGER) {
            if !commands.is_pressed(action::INVENTORY_TOGGLE) {
                return Err("physical LB did not resolve to player.inventory.toggle".to_owned());
            }
            self.hardware_controller.inventory_toggle_presses = self
                .hardware_controller
                .inventory_toggle_presses
                .saturating_add(1);
            newengine_ulog_api::ulog::info!(
                "game-ready hardware gamepad: inventory toggle semantic PASS control='LB' press={}",
                self.hardware_controller.inventory_toggle_presses
            );
        }

        if gameplay_active && input.is_gamepad_button_pressed(gamepad_button::RIGHT_TRIGGER) {
            if !commands.is_pressed(action::CHARACTER_SELECT_TOGGLE) {
                return Err(
                    "physical RB did not resolve to player.character.select.toggle".to_owned(),
                );
            }
            self.hardware_controller.character_toggle_presses = self
                .hardware_controller
                .character_toggle_presses
                .saturating_add(1);
            newengine_ulog_api::ulog::info!(
                "game-ready hardware gamepad: character menu toggle semantic PASS control='RB' press={}",
                self.hardware_controller.character_toggle_presses
            );
        }
        if input.is_gamepad_button_released(gamepad_button::RIGHT_TRIGGER) {
            if !commands.is_released(action::CHARACTER_SELECT_TOGGLE) {
                return Err(
                    "physical RB release did not resolve to character toggle release".to_owned(),
                );
            }
            self.hardware_controller.character_toggle_releases = self
                .hardware_controller
                .character_toggle_releases
                .saturating_add(1);
        }

        let pause_state_active = ctx
            .resources()
            .get::<UiPresentationFlowState>()
            .is_some_and(|flow| {
                flow.state_id == "pause"
                    || flow.state_id == "pause_settings"
                    || flow.state_id.starts_with("__northstar_shared_pause.")
            });

        if pause_state_active
            && (input.is_gamepad_button_pressed(gamepad_button::DPAD_DOWN)
                || input.is_gamepad_button_pressed(gamepad_button::DPAD_UP)
                || input.is_gamepad_button_pressed(gamepad_button::DPAD_LEFT)
                || input.is_gamepad_button_pressed(gamepad_button::DPAD_RIGHT))
        {
            if actions.ui_nav == [0, 0] {
                return Err("physical D-pad did not resolve to ui.nav semantic action".to_owned());
            }
            if !self.hardware_controller.ui_navigation_seen {
                self.hardware_controller.ui_navigation_seen = true;
                newengine_ulog_api::ulog::info!(
                    "game-ready hardware gamepad: UI navigation PASS nav=({},{})",
                    actions.ui_nav[0],
                    actions.ui_nav[1]
                );
            }
        }
        if pause_state_active && input.is_gamepad_button_pressed(gamepad_button::SOUTH) {
            if !actions.ui_accept {
                return Err("physical South/A did not resolve to ui.accept".to_owned());
            }
            if !self.hardware_controller.ui_accept_seen {
                self.hardware_controller.ui_accept_seen = true;
                newengine_ulog_api::ulog::info!(
                    "game-ready hardware gamepad: UI accept PASS control='A/South'"
                );
            }
        }

        if input.is_gamepad_button_pressed(gamepad_button::START) {
            if !commands.is_pressed(action::UI_NAVIGATION_TOGGLE) {
                return Err("physical Start did not resolve to engine.ui.primary.toggle".to_owned());
            }
            self.hardware_controller.pause_toggle_presses = self
                .hardware_controller
                .pause_toggle_presses
                .saturating_add(1);
            newengine_ulog_api::ulog::info!(
                "game-ready hardware gamepad: pause toggle semantic PASS control='Start' press={}",
                self.hardware_controller.pause_toggle_presses
            );
        }

        if let Some(flow) = ctx.resources().get::<UiPresentationFlowState>() {
            let pause = flow.state_id == "pause"
                || flow.state_id == "pause_settings"
                || flow.state_id.starts_with("__northstar_shared_pause.");
            if pause && !self.hardware_controller.pause_open_seen {
                self.hardware_controller.pause_open_seen = true;
                newengine_ulog_api::ulog::info!(
                    "game-ready hardware gamepad: pause presentation PASS state='{}'",
                    flow.state_id
                );
            }
            if self.hardware_controller.pause_open_seen
                && self.hardware_controller.pause_toggle_presses >= 1
                && flow.state_id == "gameplay"
                && !self.hardware_controller.pause_resume_seen
            {
                self.hardware_controller.pause_resume_seen = true;
                newengine_ulog_api::ulog::info!(
                    "game-ready hardware gamepad: pause resume PASS state='gameplay'"
                );
            }
        }

        if self.hardware_controller.complete() {
            newengine_ulog_api::ulog::info!(
                "game-ready validation: physical gamepad complete backend='xinput' virtual=false movement=true look=true fire=true aim=true inventory_roundtrip=true character_menu_roundtrip=true ui_navigation=true ui_accept=true pause_roundtrip=true"
            );
            return Ok(true);
        }
        Ok(false)
    }

    fn expect_device(
        snapshot: &serde_json::Value,
        id: &str,
        kind: &str,
        connected: bool,
    ) -> Result<(), String> {
        let device = &snapshot["devices"][id];
        if device["kind"].as_str() != Some(kind)
            || device["connected"].as_bool() != Some(connected)
            || device["virtual"].as_bool() != Some(true)
        {
            return Err(format!(
                "device snapshot mismatch id='{id}' expected kind='{kind}' connected={connected} actual={device}"
            ));
        }
        Ok(())
    }

    pub(super) fn hotplug(&self) -> Result<(), String> {
        for (id, kind) in [
            ("keyboard0", "keyboard"),
            ("mouse0", "mouse"),
            ("game-ready-controller", "gamepad"),
        ] {
            Self::input_ingest(
                "test.device",
                serde_json::json!({ "id": id, "kind": kind, "connected": true }),
            )?;
        }
        let connected = Self::input_state()?;
        Self::expect_device(&connected, "virtual:keyboard0", "keyboard", true)?;
        Self::expect_device(&connected, "virtual:mouse0", "mouse", true)?;
        Self::expect_device(&connected, "virtual:game-ready-controller", "gamepad", true)?;

        Self::input_ingest(
            "test.gamepad.axis",
            serde_json::json!({
                "id": "game-ready-controller",
                "axis": newengine_input_api::gamepad_axis::LEFT_STICK_X,
                "value": 0.75,
            }),
        )?;
        Self::gamepad_button(newengine_input_api::gamepad_button::SOUTH, true)?;
        let pressed = Self::input_state()?;
        let pad = &pressed["gamepads"]["virtual:game-ready-controller"];
        if pad["connected"].as_bool() != Some(true)
            || pad["buttons"][newengine_input_api::gamepad_button::SOUTH].as_f64() != Some(1.0)
            || !pad["buttons_pressed"]
                .as_array()
                .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("South")))
            || pad["axes"][newengine_input_api::gamepad_axis::LEFT_STICK_X].as_f64() != Some(0.75)
        {
            return Err(format!(
                "virtual gamepad edge/axis snapshot mismatch: {pad}"
            ));
        }
        let consumed = Self::input_state()?;
        if !consumed["gamepads"]["virtual:game-ready-controller"]["buttons_pressed"]
            .as_array()
            .is_some_and(Vec::is_empty)
        {
            return Err("gamepad pressed edge was not one-shot".to_owned());
        }
        Self::gamepad_button(newengine_input_api::gamepad_button::SOUTH, false)?;
        let released = Self::input_state()?;
        if !released["gamepads"]["virtual:game-ready-controller"]["buttons_released"]
            .as_array()
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("South")))
        {
            return Err("gamepad release edge missing".to_owned());
        }

        for (id, kind) in [
            ("keyboard0", "keyboard"),
            ("mouse0", "mouse"),
            ("game-ready-controller", "gamepad"),
        ] {
            Self::input_ingest(
                "test.device",
                serde_json::json!({ "id": id, "kind": kind, "connected": false }),
            )?;
        }
        let disconnected = Self::input_state()?;
        Self::expect_device(&disconnected, "virtual:keyboard0", "keyboard", false)?;
        Self::expect_device(&disconnected, "virtual:mouse0", "mouse", false)?;
        Self::expect_device(
            &disconnected,
            "virtual:game-ready-controller",
            "gamepad",
            false,
        )?;
        newengine_ulog_api::ulog::info!(
            "game-ready validation: device hot-plug complete devices=3 lifecycle='connect->input-edge->disconnect'"
        );
        Ok(())
    }
}

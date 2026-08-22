use super::*;

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

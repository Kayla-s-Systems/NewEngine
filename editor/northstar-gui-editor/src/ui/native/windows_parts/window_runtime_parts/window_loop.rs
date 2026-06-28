use super::*;

pub fn run(startup: &EditorStartupModel) -> Result<(), String> {
    let root_text = startup.root.display().to_string();
    let active_theme = load_editor_settings_theme(&root_text);
    let expanded_paths = vec![root_text.clone()];
    let tree_nodes = load_tree_nodes(Path::new(&root_text), &expanded_paths);
    let _ = UI_STATE.set(Mutex::new(UiState {
        root: root_text.clone(),
        selected_path: root_text.clone(),
        expanded_paths,
        provider_count: startup.provider_count,
        capability_count: startup.capability_count,
        format_type_count: startup.format_type_count,
        preview_provider_count: startup.preview_provider_count,
        provider_ids: startup.provider_ids.clone(),
        format_type_ids: startup.format_type_ids.clone(),
        tool_routes: startup.tool_routes.clone(),
        selected_row: 0,
        hover_row: None,
        selected_tree: 0,
        tree_scroll_rows: 0,
        scroll_rows: 0,
        inspector_scroll_rows: 0,
        hover_panel: HoverPanel::None,
        menu_hover: None,
        menu_active: None,
        menu_open: None,
        menu_item_hover: None,
        toolbar_hover: None,
        toolbar_active: None,
        filter_focus: false,
        filter_query: String::new(),
        view_mode: menu_model::default_view_mode(),
        modal_dialog: None,
        modal_hwnd: 0,
        modal_dragging: false,
        modal_drag_dx: 0,
        modal_drag_dy: 0,
        pending_load_tools_request: false,
        status: "Ready. Select a directory or package in the tree.".to_owned(),
        tree_nodes,
        preview_path: None,
        preview_name: String::new(),
        preview_kind: String::new(),
        preview_provider: String::new(),
        preview_size: None,
        preview_type_id: None,
        preview_content_kind: None,
        preview_surface: None,
        preview_lines: Vec::new(),
        active_document: None,
        cached_spans: Vec::new(),
        xml_path: None,
        xml_lines: Vec::new(),
        xml_cursor_line: 0,
        xml_cursor_col: 0,
        xml_search_query: String::new(),
        xml_search_focus: false,
        xml_dirty: false,
        caret_visible: true,
        modal_text_selection_dragging: false,
        modal_text_selection_drag_anchor: None,
        editor_theme: builtin_editor_color_dictionary(&active_theme),
    }));

    unsafe {
        let h_instance = GetModuleHandleW(null());
        if h_instance.is_null() {
            return Err("GetModuleHandleW failed".to_owned());
        }
        let class_name = to_wide("NorthStarGuiEditorWindow");
        let title = to_wide("NorthStar GUI Editor");
        let wnd_class = WndClassW {
            style: CS_DBLCLKS,
            lpfn_wnd_proc: Some(window_proc),
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance,
            h_icon: null_mut(),
            h_cursor: LoadCursorW(null_mut(), IDC_ARROW as Lpcwstr),
            hbr_background: (COLOR_WINDOW + 1) as Hbrush,
            lpsz_menu_name: null(),
            lpsz_class_name: class_name.as_ptr(),
        };
        RegisterClassW(&wnd_class);
        let modal_class_name = to_wide("NorthStarGuiEditorModalWindow");
        let modal_wnd_class = WndClassW {
            style: CS_DBLCLKS,
            lpfn_wnd_proc: Some(modal_window_proc),
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_instance,
            h_icon: null_mut(),
            h_cursor: LoadCursorW(null_mut(), IDC_ARROW as Lpcwstr),
            hbr_background: (COLOR_WINDOW + 1) as Hbrush,
            lpsz_menu_name: null(),
            lpsz_class_name: modal_class_name.as_ptr(),
        };
        RegisterClassW(&modal_wnd_class);
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1280,
            820,
            null_mut(),
            null_mut(),
            h_instance,
            null_mut(),
        );
        if hwnd.is_null() {
            return Err("CreateWindowExW failed".to_owned());
        }
        SetTimer(hwnd, CARET_TIMER_ID, CARET_BLINK_MS, null());
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        let mut msg: Msg = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

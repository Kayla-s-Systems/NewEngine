#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalDialogKind {
    About,
    LoadTools,
    Doctor,
    Message,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModalDialogModel {
    pub kind: ModalDialogKind,
    pub title: String,
    pub message: String,
    pub primary_action: String,
}

impl ModalDialogModel {
    pub fn about() -> Self {
        Self {
            kind: ModalDialogKind::About,
            title: "About NorthStar GUI Editor".to_owned(),
            message: "Provider-driven asset editor host. Formats and preview/editor routes are loaded from self-describing runtime tools.".to_owned(),
            primary_action: "OK".to_owned(),
        }
    }

    pub fn load_tools() -> Self {
        Self {
            kind: ModalDialogKind::LoadTools,
            title: "Load Tools".to_owned(),
            message: "Runtime tool loading is provider-driven. Use tools-load-dir --dir <path> or the future folder picker to mount a directory of self-describing tools.".to_owned(),
            primary_action: "OK".to_owned(),
        }
    }

    pub fn doctor() -> Self {
        Self {
            kind: ModalDialogKind::Doctor,
            title: "Tools Doctor".to_owned(),
            message: "Tools Doctor checks discovery, self-description commands, format routes and preview/edit capabilities. CLI: tools-doctor.".to_owned(),
            primary_action: "OK".to_owned(),
        }
    }

    pub fn message(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: ModalDialogKind::Message,
            title: title.into(),
            message: message.into(),
            primary_action: "OK".to_owned(),
        }
    }
}

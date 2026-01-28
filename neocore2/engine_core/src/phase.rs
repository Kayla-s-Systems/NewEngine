#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FramePhase {
    BeginFrame,
    Input,

    FixedUpdate,
    Update,
    LateUpdate,

    Extract,
    Prepare,
    Render,
    Present,

    EndFrame,
}

impl FramePhase {
    pub fn as_str(self) -> &'static str {
        match self {
            FramePhase::BeginFrame => "BeginFrame",
            FramePhase::Input => "Input",
            FramePhase::FixedUpdate => "FixedUpdate",
            FramePhase::Update => "Update",
            FramePhase::LateUpdate => "LateUpdate",
            FramePhase::Extract => "Extract",
            FramePhase::Prepare => "Prepare",
            FramePhase::Render => "Render",
            FramePhase::Present => "Present",
            FramePhase::EndFrame => "EndFrame",
        }
    }
}
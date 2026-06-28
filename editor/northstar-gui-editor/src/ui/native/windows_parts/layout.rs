use super::*;

#[derive(Clone, Copy)]
pub(super) struct Layout {
    pub(super) left: Rect,
    pub(super) center: Rect,
    pub(super) right: Rect,
    pub(super) status: Rect,
}

#[derive(Debug, Clone)]
pub(super) enum UiUpdateRequest {
    None,
    Region(Rect),
    Regions(Vec<Rect>),
    Layout,
    Full,
}

impl UiUpdateRequest {
    pub(super) fn push_region(&mut self, rect: Rect) {
        match self {
            UiUpdateRequest::None => *self = UiUpdateRequest::Region(rect),
            UiUpdateRequest::Region(existing) => {
                *self = UiUpdateRequest::Regions(vec![*existing, rect]);
            }
            UiUpdateRequest::Regions(regions) => regions.push(rect),
            UiUpdateRequest::Layout | UiUpdateRequest::Full => {}
        }
    }
}

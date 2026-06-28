#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceClient {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalHit {
    None,
    TitleBar,
    Close,
    Primary,
    Body,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModalSurfaceState {
    offset_x: i32,
    offset_y: i32,
    dragging: bool,
    drag_dx: i32,
    drag_dy: i32,
}

impl Default for ModalSurfaceState {
    fn default() -> Self {
        Self {
            offset_x: 0,
            offset_y: 0,
            dragging: false,
            drag_dx: 0,
            drag_dy: 0,
        }
    }
}

impl ModalSurfaceState {
    pub const WIDTH: i32 = 520;
    pub const HEIGHT: i32 = 220;

    pub fn rect(&self, client: SurfaceClient) -> SurfaceRect {
        let left = self.centered_left(client) + self.offset_x;
        let top = self.centered_top(client) + self.offset_y;
        SurfaceRect {
            left,
            top,
            right: left + Self::WIDTH,
            bottom: top + Self::HEIGHT,
        }
    }

    pub fn title_rect(&self, client: SurfaceClient) -> SurfaceRect {
        let rect = self.rect(client);
        SurfaceRect {
            left: rect.left + 1,
            top: rect.top + 1,
            right: rect.right - 1,
            bottom: rect.top + 38,
        }
    }

    pub fn close_rect(&self, client: SurfaceClient) -> SurfaceRect {
        let rect = self.rect(client);
        SurfaceRect {
            left: rect.right - 36,
            top: rect.top + 7,
            right: rect.right - 10,
            bottom: rect.top + 31,
        }
    }

    pub fn primary_rect(&self, client: SurfaceClient) -> SurfaceRect {
        let rect = self.rect(client);
        SurfaceRect {
            left: rect.right - 112,
            top: rect.bottom - 44,
            right: rect.right - 18,
            bottom: rect.bottom - 16,
        }
    }

    pub fn hit_test(&self, client: SurfaceClient, x: i32, y: i32) -> ModalHit {
        if contains(self.close_rect(client), x, y) {
            ModalHit::Close
        } else if contains(self.primary_rect(client), x, y) {
            ModalHit::Primary
        } else if contains(self.title_rect(client), x, y) {
            ModalHit::TitleBar
        } else if contains(self.rect(client), x, y) {
            ModalHit::Body
        } else {
            ModalHit::None
        }
    }

    pub fn start_drag(&mut self, client: SurfaceClient, x: i32, y: i32) -> bool {
        if self.hit_test(client, x, y) != ModalHit::TitleBar {
            return false;
        }
        let rect = self.rect(client);
        self.dragging = true;
        self.drag_dx = x - rect.left;
        self.drag_dy = y - rect.top;
        true
    }

    pub fn drag_to(&mut self, client: SurfaceClient, x: i32, y: i32) -> bool {
        if !self.dragging {
            return false;
        }
        self.offset_x = x - self.drag_dx - self.centered_left(client);
        self.offset_y = y - self.drag_dy - self.centered_top(client);
        true
    }

    pub fn finish_drag(&mut self) -> bool {
        let was_dragging = self.dragging;
        self.dragging = false;
        was_dragging
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    fn centered_left(&self, client: SurfaceClient) -> i32 {
        (client.width - Self::WIDTH) / 2
    }

    fn centered_top(&self, client: SurfaceClient) -> i32 {
        (client.height - Self::HEIGHT) / 2
    }
}

pub fn contains(rect: SurfaceRect, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> SurfaceClient {
        SurfaceClient {
            width: 1280,
            height: 720,
        }
    }

    #[test]
    fn centers_modal_in_client() {
        let surface = ModalSurfaceState::default();
        assert_eq!(
            surface.rect(client()),
            SurfaceRect {
                left: 380,
                top: 250,
                right: 900,
                bottom: 470
            }
        );
    }

    #[test]
    fn hit_tests_modal_zones() {
        let surface = ModalSurfaceState::default();
        let client = client();
        assert_eq!(surface.hit_test(client, 870, 260), ModalHit::Close);
        assert_eq!(surface.hit_test(client, 800, 440), ModalHit::Primary);
        assert_eq!(surface.hit_test(client, 400, 260), ModalHit::TitleBar);
        assert_eq!(surface.hit_test(client, 400, 330), ModalHit::Body);
        assert_eq!(surface.hit_test(client, 10, 10), ModalHit::None);
    }

    #[test]
    fn drag_updates_offset_until_finish() {
        let client = client();
        let mut surface = ModalSurfaceState::default();
        assert!(surface.start_drag(client, 400, 260));
        assert!(surface.is_dragging());
        assert!(surface.drag_to(client, 450, 300));
        assert_eq!(
            surface.rect(client),
            SurfaceRect {
                left: 430,
                top: 290,
                right: 950,
                bottom: 510
            }
        );
        assert!(surface.finish_drag());
        assert!(!surface.is_dragging());
        assert!(!surface.drag_to(client, 500, 500));
        surface.reset();
        assert_eq!(
            surface.rect(client),
            SurfaceRect {
                left: 380,
                top: 250,
                right: 900,
                bottom: 470
            }
        );
    }

    #[test]
    fn drag_cannot_start_from_body_or_buttons() {
        let client = client();
        let mut surface = ModalSurfaceState::default();
        assert!(!surface.start_drag(client, 800, 440));
        assert!(!surface.start_drag(client, 400, 330));
        assert!(!surface.is_dragging());
    }
}

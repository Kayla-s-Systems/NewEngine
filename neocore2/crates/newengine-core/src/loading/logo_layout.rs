use super::boot_frame::{BootRect, BootViewport};
use super::profile::MAX_LOADING_LOGOS;

/// Computes a deterministic, centered logo grid shared by boot and retained-UI presenters.
///
/// A single logo preserves the original NorthStar placement. Multi-logo manifests use a
/// bounded 1-4 column grid above the loading status area and retain authoring order.
pub fn layout_logo_rects(viewport: BootViewport, logo_count: usize) -> Vec<BootRect> {
    let safe_w = finite_positive(viewport.width, 1.0);
    let safe_h = finite_positive(viewport.height, 1.0);
    let logo_count = logo_count.min(MAX_LOADING_LOGOS);
    if logo_count == 0 {
        return Vec::new();
    }

    if logo_count == 1 {
        let shortest_side = safe_w.min(safe_h);
        let logo_size_max = (shortest_side - 32.0).clamp(1.0, 360.0);
        let logo_size_min = 180.0_f32.min(logo_size_max);
        let logo_size = (shortest_side * 0.28).clamp(logo_size_min, logo_size_max);
        return vec![BootRect::new(
            ((safe_w - logo_size) * 0.5).max(0.0),
            ((safe_h - logo_size) * 0.42).max(0.0),
            logo_size,
            logo_size,
        )];
    }

    let columns = match logo_count {
        0..=3 => logo_count,
        4..=6 => 3,
        _ => 4,
    };
    let rows = logo_count.div_ceil(columns);
    let shortest_side = safe_w.min(safe_h);
    let gap = (shortest_side * 0.025).clamp(0.0, 24.0);
    let side_margin = (safe_w * 0.04).clamp(0.0, 32.0);
    let top_margin = (safe_h * 0.03).clamp(0.0, 24.0);

    let bar_y_max = (safe_h - 90.0).max(0.0);
    let bar_y_min = 420.0_f32.min(bar_y_max);
    let bar_y = (safe_h * 0.72).clamp(bar_y_min, bar_y_max);
    let content_bottom = (bar_y - 116.0).max(top_margin + 0.01);

    let available_w = (safe_w - side_margin * 2.0).max(0.01);
    let available_h = (content_bottom - top_margin).max(0.01);
    let gaps_w = gap * columns.saturating_sub(1) as f32;
    let gaps_h = gap * rows.saturating_sub(1) as f32;
    let size_by_width = ((available_w - gaps_w).max(0.01) / columns as f32).max(0.01);
    let size_by_height = ((available_h - gaps_h).max(0.01) / rows as f32).max(0.01);
    let logo_size = size_by_width.min(size_by_height).min(240.0);

    let grid_w = logo_size * columns as f32 + gaps_w;
    let grid_h = logo_size * rows as f32 + gaps_h;
    let origin_x = ((safe_w - grid_w) * 0.5).max(0.0);
    let origin_y = top_margin + ((available_h - grid_h).max(0.0) * 0.5);

    (0..logo_count)
        .map(|index| {
            let row = index / columns;
            let column = index % columns;
            BootRect::new(
                origin_x + column as f32 * (logo_size + gap),
                origin_y + row as f32 * (logo_size + gap),
                logo_size,
                logo_size,
            )
        })
        .collect()
}

#[inline]
fn finite_positive(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_logo_preserves_legacy_centered_square() {
        let rects = layout_logo_rects(BootViewport::default(), 1);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].w, rects[0].h);
        assert!(rects[0].x > 0.0);
        assert!(rects[0].y > 0.0);
    }

    #[test]
    fn multi_logo_grid_is_bounded_and_non_overlapping() {
        let viewport = BootViewport::default();
        let rects = layout_logo_rects(viewport, MAX_LOADING_LOGOS + 4);
        assert_eq!(rects.len(), MAX_LOADING_LOGOS);

        for (index, rect) in rects.iter().enumerate() {
            assert!(rect.x >= 0.0 && rect.y >= 0.0);
            assert!(rect.w > 0.0 && rect.h > 0.0);
            assert!(rect.x + rect.w <= viewport.width + f32::EPSILON);
            assert!(rect.y + rect.h <= viewport.height + f32::EPSILON);

            for other in rects.iter().skip(index + 1) {
                let overlaps = rect.x < other.x + other.w
                    && rect.x + rect.w > other.x
                    && rect.y < other.y + other.h
                    && rect.y + rect.h > other.y;
                assert!(!overlaps);
            }
        }
    }
}

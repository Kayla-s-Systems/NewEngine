#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorref_parser_converts_rgb_to_windows_bgr_layout() {
        assert_eq!(parse_colorref("#112233"), Some(COLORREF(0x00332211)));
        assert_eq!(parse_colorref("112233"), None);
    }

    #[test]
    fn media_foundation_path_strips_extended_dos_namespace() {
        assert_eq!(
            normalize_media_foundation_path(r"\\?\C:\NorthStar\logo.mp4"),
            r"C:\NorthStar\logo.mp4"
        );
    }

    #[test]
    fn media_foundation_path_converts_extended_unc_namespace() {
        assert_eq!(
            normalize_media_foundation_path(r"\\?\UNC\server\share\logo.mp4"),
            r"\\server\share\logo.mp4"
        );
    }

    #[test]
    fn aspect_fit_preserves_square_video_inside_widescreen_window() {
        assert_eq!(aspect_fit(1280, 720, 960, 960), (280, 0, 720, 720));
    }

    #[test]
    fn hundred_nanosecond_timestamps_convert_without_float_drift() {
        assert_eq!(duration_from_hns(10_000_000), Duration::from_secs(1));
        assert_eq!(duration_from_hns(-1), Duration::ZERO);
    }
}

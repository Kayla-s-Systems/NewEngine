// Test-module forwarding path for generated/path-mounted ytyp_metadata.rs.
// Some generated revisions declare plain `mod tests;`, while hand-authored revisions may
// use an explicit `#[path = "ytyp_metadata/tests.rs"]`. Keep one authoritative test body.
include!("ytyp_metadata/tests.rs");

use acip_sidecar::extract::{run_helper, ExtractKind, ExtractRequest};
use serial_test::serial;
use std::time::Duration;

#[test]
#[serial]
fn extractor_large_output_via_tempfile() {
    std::env::set_var("ACIP_EXTRACTOR_BIN", env!("CARGO_BIN_EXE_acip-extract"));
    std::env::set_var("ACIP_EXTRACTOR_SELFTEST_LARGE", "1");

    let req = ExtractRequest {
        kind: ExtractKind::Pdf,
        content_type: None,
        max_pages: None,
        dpi: None,
        max_output_chars: Some(2_000_000),
    };

    let resp = run_helper(&req, b"%PDF-1.4\n", Duration::from_secs(10))
        .expect("expected large response");

    assert!(resp.ok);
    assert_eq!(resp.text.len(), 7_000_000);

    std::env::remove_var("ACIP_EXTRACTOR_SELFTEST_LARGE");
    std::env::remove_var("ACIP_EXTRACTOR_BIN");
}

use vietqr_service::core::deeplink::*;

#[test]
fn test_bank_app_registry_counts() {
    assert_eq!(BANK_APP_DEEPLINKS.len(), 35);
    assert_eq!(selectable_bank_app_deeplinks().len(), 33);
}

#[test]
fn test_find_bank_app_deeplink() {
    let mb = find_bank_app_deeplink("mb").expect("should find mb");
    assert_eq!(mb.app_name, "MB Bank");
    assert!(mb.autofill);
    assert_eq!(mb.ios_scheme, Some("mbbank://"));

    let tpb = find_bank_app_deeplink("TPB").expect("should find tpbank");
    assert_eq!(tpb.app_name, "TPBank Mobile");
    assert_eq!(tpb.android_scheme, "hydro");

    let vcb = find_bank_app_deeplink("vietcombank").expect("should find vcb");
    assert_eq!(vcb.app_name, "VCB Digibank");
}

#[test]
fn test_build_android_intent_uri() {
    let mb = find_bank_app_deeplink("mb").unwrap();
    let intent = build_android_intent_uri(mb, "https://example.com/fallback");
    assert!(intent.starts_with("intent://#Intent;scheme=mbbank;package=com.mbmobile;"));
    assert!(intent.contains("S.browser_fallback_url=https%3A%2F%2Fexample.com%2Ffallback"));
}

#[test]
fn test_bank_app_launch_url_cross_bank() {
    let launch_url = bank_app_launch_url_for_app(
        None,
        "tcb",
        "VCB",
        "001100223344",
        150000,
        Some("Don hang 1001"),
        Some("Phạm Thành Nam"),
    )
    .expect("generate launch url");

    assert!(launch_url.starts_with("/qr/pay?app=tcb&ba=001100223344@vcb&am=150000"));
    assert!(launch_url.contains("&tn=Don%20hang%201001"));
    assert!(launch_url.contains("&bn=PHAM%20THANH%20NAM"));
}

#[test]
fn test_parse_ba_param() {
    let parsed = parse_ba_param("69238686888@tpb");
    assert_eq!(parsed, Some(("69238686888".to_string(), "TPB".to_string())));
}

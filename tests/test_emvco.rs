use vietqr_service::core::emvco::*;

#[test]
fn test_bank_code_to_bin_mappings() {
    assert_eq!(bank_code_to_bin("TPB"), Some("970423"));
    assert_eq!(bank_code_to_bin("tpbank"), Some("970423"));
    assert_eq!(bank_code_to_bin("970423"), Some("970423"));
    assert_eq!(bank_code_to_bin("VCB"), Some("970436"));
    assert_eq!(bank_code_to_bin("vietcombank"), Some("970436"));
    assert_eq!(bank_code_to_bin("MB"), Some("970422"));
    assert_eq!(bank_code_to_bin("mbbank"), Some("970422"));
    assert_eq!(bank_code_to_bin("TCB"), Some("970407"));
    assert_eq!(bank_code_to_bin("techcombank"), Some("970407"));
    assert_eq!(bank_code_to_bin("BIDV"), Some("970418"));
    assert_eq!(bank_code_to_bin("VPB"), Some("970432"));
    assert_eq!(bank_code_to_bin("ACB"), Some("970416"));
}

#[test]
fn test_strip_vietnamese_accents() {
    assert_eq!(strip_vietnamese_accents("ỦNG HỘ NAM"), "UNG HO NAM");
    assert_eq!(strip_vietnamese_accents("Đặng Văn Lâm"), "Dang Van Lam");
    assert_eq!(
        strip_vietnamese_accents("Nguyễn Thị Thùy Dung"),
        "Nguyen Thi Thuy Dung"
    );
}

#[test]
fn test_sanitize_memo_and_name() {
    let clean_memo = sanitize_vietqr_memo("Tiền cà phê ☕️ test emoji & ký tự @#$%^&*()_+", 50);
    assert_eq!(clean_memo, "Tien ca phe test emoji ky tu _");

    let clean_name = sanitize_vietqr_name("Phạm Thành Nam");
    assert_eq!(clean_name, "PHAM THANH NAM");
}

#[test]
fn test_build_vietqr_payload_validity() {
    let payload = build_vietqr_payload(
        "TPB",
        "69238686888",
        50000,
        Some("UNG HO PHAM THANH NAM"),
        Some("PHAM THANH NAM"),
    )
    .expect("should generate payload");

    assert!(payload.starts_with("000201010212"));
    assert!(payload.contains("970423")); // TPBank BIN
    assert!(payload.contains("69238686888")); // Account
    assert!(payload.contains("50000")); // Amount
    assert!(payload.contains("UNG HO PHAM THANH NAM"));
    assert!(payload.ends_with(&format_crc16(&payload.as_bytes()[..payload.len() - 4])));
}

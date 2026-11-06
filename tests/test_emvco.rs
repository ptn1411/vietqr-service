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

#[test]
fn test_large_amounts_formatting_and_emvco_payload() {
    // 500 million VND
    assert_eq!(format_vnd_amount(500_000_000), "500.000.000 VND");

    // 10 billion VND (10 tỷ)
    assert_eq!(format_vnd_amount(10_000_000_000), "10.000.000.000 VND");

    // 999 billion 999 million 999 thousand 999 VND
    assert_eq!(
        format_vnd_amount(999_999_999_999),
        "999.999.999.999 VND"
    );

    // Negative and zero amounts return empty string
    assert_eq!(format_vnd_amount(0), "");
    assert_eq!(format_vnd_amount(-50_000), "");

    // Test EMVCo payload with 10 billion VND
    let p_10b = build_vietqr_payload(
        "TPB",
        "69238686888",
        10_000_000_000,
        Some("THANH TOAN 10 TY"),
        Some("PHAM THANH NAM"),
    )
    .expect("payload for 10 billion VND should succeed");

    assert!(p_10b.contains("541110000000000")); // Tag 54, length 11, value 10000000000
    assert!(p_10b.starts_with("000201010212")); // Dynamic QR
    let crc_10b = &p_10b[p_10b.len() - 4..];
    assert_eq!(crc_10b, format_crc16(&p_10b.as_bytes()[..p_10b.len() - 4]));

    // Test EMVCo payload with 500 billion VND
    let p_500b = build_vietqr_payload(
        "VCB",
        "0011002233445",
        500_000_000_000,
        Some("HOP DONG KINH DOANH"),
        None,
    )
    .expect("payload for 500 billion VND should succeed");

    assert!(p_500b.contains("5412500000000000")); // Tag 54, length 12
    let crc_500b = &p_500b[p_500b.len() - 4..];
    assert_eq!(crc_500b, format_crc16(&p_500b.as_bytes()[..p_500b.len() - 4]));

    // Test zero amount generates Static QR (Tag 01 = '11', no Tag 54 between 53 and 58)
    let p_zero = build_vietqr_payload("MB", "0987654321", 0, None, None)
        .expect("payload with zero amount should succeed");
    assert!(p_zero.starts_with("000201010211")); // Static QR
    assert!(p_zero.contains("53037045802VN")); // Tag 53 immediately followed by Tag 58 (no Tag 54)
}

#[test]
fn test_special_characters_in_transfer_memo() {
    // 1. Complex Vietnamese accents and diacritics
    let memo_vn = "Ủng hộ quỹ vaccine Đắk Lắk & TP. Hồ Chí Minh 2026!";
    let clean_vn = sanitize_vietqr_memo(memo_vn, 50);
    assert_eq!(clean_vn, "Ung ho quy vaccine Dak Lak TP Ho Chi Minh 2026");

    // 2. Heavy special characters, symbols, brackets, math operators
    let memo_symbols = "DH#12345! @shop (VIP) [T5] {ABC} $100% ^&* += ~ \\ / | ?";
    let clean_symbols = sanitize_vietqr_memo(memo_symbols, 50);
    assert_eq!(clean_symbols, "DH12345 shop VIP T5 ABC 100");

    // 3. Emojis, hearts, stickers
    let memo_emoji = "Cà phê sáng ☕️❤️🔥🚀🎉💸 chuyển khoản gấp!";
    let clean_emoji = sanitize_vietqr_memo(memo_emoji, 50);
    assert_eq!(clean_emoji, "Ca phe sang chuyen khoan gap");

    // 4. Safe punctuation retention: `-` and `_` are preserved
    let memo_safe_punct = "ORDER-2026_11-A_99";
    let clean_safe = sanitize_vietqr_memo(memo_safe_punct, 50);
    assert_eq!(clean_safe, "ORDER-2026_11-A_99");

    // 5. Code injection / XSS / SQL injection payloads sanitized into harmless alphanumeric
    let memo_xss = "<script>alert('XSS')</script> SELECT * FROM 'users'; --";
    let clean_xss = sanitize_vietqr_memo(memo_xss, 50);
    assert_eq!(clean_xss, "scriptalertXSSscript SELECT FROM users --");

    // 6. Whitespace normalization: tabs, newlines, multiple consecutive spaces
    let memo_whitespace = "  DH101 \n\t  -  \r\n  TIEN_PHONG   ";
    let clean_ws = sanitize_vietqr_memo(memo_whitespace, 50);
    assert_eq!(clean_ws, "DH101 - TIEN_PHONG");

    // 7. Pure special characters string becomes empty string without panicking
    let memo_pure_symbols = "!@#$%^&*()+={}[]|\\:;\"'<>,.?/";
    let clean_empty = sanitize_vietqr_memo(memo_pure_symbols, 50);
    assert_eq!(clean_empty, "");
}

#[test]
fn test_long_memo_truncation_and_boundaries() {
    // 1. Long memo with 100+ characters truncated to exactly 50 chars
    let memo_100 = "Chuyển tiền mua máy tính xách tay ThinkPad T14s Gen 4 AMD Ryzen 7 PRO 7840U 32GB RAM màn hình 2.8K OLED";
    let clean_100 = sanitize_vietqr_memo(memo_100, 50);
    assert!(clean_100.len() <= 50, "Length was {} > 50", clean_100.len());
    assert_eq!(clean_100, "Chuyen tien mua may tinh xach tay ThinkPad T14s Ge");
    assert_eq!(clean_100.len(), 50);

    // 2. Extremely long string (1000 characters) - DoS attack simulation
    let memo_1000 = "DON HANG SIEU CAP VIP ".repeat(50);
    let clean_1000 = sanitize_vietqr_memo(&memo_1000, 50);
    assert_eq!(clean_1000.len(), 50);

    // 3. Custom max_len parameter clamping
    let memo_short_limit = sanitize_vietqr_memo("DON HANG SO 123456789", 10);
    assert_eq!(memo_short_limit, "DON HANG S");
    assert_eq!(memo_short_limit.len(), 10);

    // 4. Clamping upper bound: even if max_len = 1000 is requested, max limit is 50
    let memo_over_limit = sanitize_vietqr_memo(&memo_1000, 1000);
    assert!(memo_over_limit.len() <= 50);

    // 5. Build full EMVCo payload with a very long memo and verify Tag 62 Subtag 08 format
    let payload = build_vietqr_payload(
        "BIDV",
        "1234567890",
        250_000,
        Some(memo_100),
        Some("NGUYEN VAN A"),
    )
    .expect("payload with long memo should succeed");

    // Tag 62 should wrap Sub-tag 08 with exact length:
    // Subtag 08 len = 50 chars -> "0850" + content (total 54 chars)
    // Tag 62 len = 54 -> "6254"
    assert!(payload.contains("62540850Chuyen tien mua may tinh xach tay ThinkPad T14s Ge"));

    // Check CRC16 validity
    let crc = &payload[payload.len() - 4..];
    assert_eq!(crc, format_crc16(&payload.as_bytes()[..payload.len() - 4]));
}

#[test]
fn test_beneficiary_name_with_special_characters_and_length() {
    // 1. Vietnamese accents in name
    let name_vn = "Trần Thị Quỳnh Như";
    assert_eq!(sanitize_vietqr_name(name_vn), "TRAN THI QUYNH NHU");

    // 2. Name with special characters, numbers and punctuation
    let name_symbols = "Nguyễn Văn Đạt (GĐ) #01 - CTCP";
    assert_eq!(sanitize_vietqr_name(name_symbols), "NGUYEN VAN DAT GD 01 CTCP");

    // 3. Name exceeding EMVCo Tag 59 limit (25 chars) is truncated safely
    let name_long = "NGUYỄN PHẠM HOÀNG ĐẶNG BẢO LONG QUÂN";
    let clean_name = sanitize_vietqr_name(name_long);
    assert!(clean_name.len() <= 25, "Name length was {} > 25", clean_name.len());
    assert_eq!(clean_name, "NGUYEN PHAM HOANG DANG BA");
    assert_eq!(clean_name.len(), 25);
}

#[test]
fn test_extreme_inputs_combined_integrity() {
    // Huge amount (50 billion) + Extreme special chars memo + Long name
    let payload = build_vietqr_payload(
        "TPB",
        "69238686888",
        50_000_000_000,
        Some("DH#99999-VIP! @@@ Cà phê ☕️ + Ăn trưa 🥩 & Mua sắm siêu dài qua 50 ký tự để kiểm tra..."),
        Some("LÊ HOÀNG BẢO ANH THƯ TRẦN GIA"),
    )
    .expect("combined extreme input should generate valid payload");

    assert!(payload.starts_with("000201010212"));
    assert!(payload.contains("970423")); // TPBank BIN
    assert!(payload.contains("541150000000000")); // 50 billion VND
    assert!(payload.contains("5925LE HOANG BAO ANH THU TRA")); // Tag 59, 25 chars
    assert!(payload.contains("62540850DH99999-VIP Ca phe An trua Mua sam sieu dai qua 50")); // Tag 62, 54 chars (subtag 08 len 50)
    assert_eq!(
        &payload[payload.len() - 4..],
        format_crc16(&payload.as_bytes()[..payload.len() - 4])
    );
}

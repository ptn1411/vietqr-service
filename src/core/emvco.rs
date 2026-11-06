use anyhow::{Result, bail};

/// Format CRC16-CCITT (False/0xFFFF) as uppercase 4-digit hexadecimal string.
pub fn format_crc16(data: &[u8]) -> String {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc <<= 1;
            }
        }
    }
    format!("{:04X}", crc)
}

/// Convert accented Vietnamese characters into unaccented uppercase/lowercase ASCII equivalents.
pub fn strip_vietnamese_accents(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let mapped = match c {
            'a' | 'á' | 'à' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ắ' | 'ằ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ấ'
            | 'ầ' | 'ẩ' | 'ẫ' | 'ậ' => 'a',
            'A' | 'Á' | 'À' | 'Ả' | 'Ã' | 'Ạ' | 'Ă' | 'Ắ' | 'Ằ' | 'Ẳ' | 'Ẵ' | 'Ặ' | 'Â' | 'Ấ'
            | 'Ầ' | 'Ẩ' | 'Ẫ' | 'Ậ' => 'A',
            'd' | 'đ' => 'd',
            'D' | 'Đ' => 'D',
            'e' | 'é' | 'è' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ế' | 'ề' | 'ể' | 'ễ' | 'ệ' => 'e',
            'E' | 'É' | 'È' | 'Ẻ' | 'Ẽ' | 'Ẹ' | 'Ê' | 'Ế' | 'Ề' | 'Ể' | 'Ễ' | 'Ệ' => 'E',
            'i' | 'í' | 'ì' | 'ỉ' | 'ĩ' | 'ị' => 'i',
            'I' | 'Í' | 'Ì' | 'Ỉ' | 'Ĩ' | 'Ị' => 'I',
            'o' | 'ó' | 'ò' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ố' | 'ồ' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ớ'
            | 'ờ' | 'ở' | 'ỡ' | 'ợ' => 'o',
            'O' | 'Ó' | 'Ò' | 'Ỏ' | 'Õ' | 'Ọ' | 'Ô' | 'Ố' | 'Ồ' | 'Ổ' | 'Ỗ' | 'Ộ' | 'Ơ' | 'Ớ'
            | 'Ờ' | 'Ở' | 'Ỡ' | 'Ợ' => 'O',
            'u' | 'ú' | 'ù' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ứ' | 'ừ' | 'ử' | 'ữ' | 'ự' => 'u',
            'U' | 'Ú' | 'Ù' | 'Ủ' | 'Ũ' | 'Ụ' | 'Ư' | 'Ứ' | 'Ừ' | 'Ử' | 'Ữ' | 'Ự' => 'U',
            'y' | 'ý' | 'ỳ' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
            'Y' | 'Ý' | 'Ỳ' | 'Ỷ' | 'Ỹ' | 'Ỵ' => 'Y',
            other => other,
        };
        out.push(mapped);
    }
    out
}

/// Sanitize transfer memo according to VietQR / NAPAS guidelines:
/// Strips Vietnamese accents, retains alphanumeric and safe punctuation (`-`, `_`, space),
/// collapses multiple spaces and truncates to `max_len`.
pub fn sanitize_vietqr_memo(memo: &str, max_len: usize) -> String {
    let unaccented = strip_vietnamese_accents(memo);
    let mut cleaned = String::with_capacity(unaccented.len());
    for c in unaccented.chars() {
        if c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '_' {
            cleaned.push(c);
        }
    }
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    let joined = words.join(" ");
    let limit = max_len.clamp(1, 50);
    if joined.len() > limit {
        joined[..limit].trim_end().to_string()
    } else {
        joined
    }
}

/// Sanitize account beneficiary name:
/// Strips accents, forces uppercase ASCII, limits to 25 chars per EMVCo Tag 59 guidelines.
pub fn sanitize_vietqr_name(name: &str) -> String {
    let unaccented = strip_vietnamese_accents(name).to_ascii_uppercase();
    let mut cleaned = String::with_capacity(unaccented.len());
    for c in unaccented.chars() {
        if c.is_ascii_alphanumeric() || c == ' ' {
            cleaned.push(c);
        }
    }
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    let joined = words.join(" ");
    if joined.len() > 25 {
        joined[..25].trim_end().to_string()
    } else {
        joined
    }
}

/// Sanitize account number by removing spaces, hyphens, periods.
pub fn sanitize_vietqr_account(account: &str) -> String {
    account
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

/// Map bank short code or alias to 6-digit NAPAS BIN for 65+ Vietnamese financial institutions.
pub fn bank_code_to_bin(code: &str) -> Option<&'static str> {
    let clean = code.trim().to_ascii_uppercase();
    match clean.as_str() {
        "970436" | "VCB" | "VIETCOMBANK" => Some("970436"),
        "970415" | "CTG" | "ICB" | "VIETINBANK" => Some("970415"),
        "970418" | "BID" | "BIDV" => Some("970418"),
        "970405" | "AGRI" | "AGRIBANK" | "VARB" | "VBA" | "VBARD" => Some("970405"),
        "970407" | "TCB" | "TECHCOMBANK" => Some("970407"),
        "970416" | "ACB" => Some("970416"),
        "970432" | "VPB" | "VPBANK" => Some("970432"),
        "970422" | "MB" | "MBB" | "MBBANK" => Some("970422"),
        "970423" | "TIENPHONG" | "TPB" | "TPBANK" => Some("970423"),
        "970403" | "SACOMBANK" | "STB" => Some("970403"),
        "970437" | "HDB" | "HDBANK" => Some("970437"),
        "970441" | "VIB" => Some("970441"),
        "970443" | "SHB" => Some("970443"),
        "970426" | "MARITIMEBANK" | "MSB" => Some("970426"),
        "970448" | "OCB" => Some("970448"),
        "970431" | "EIB" | "EXIMBANK" => Some("970431"),
        "970449" | "LIENVIETPOSTBANK" | "LPB" | "LPBANK" => Some("970449"),
        "970440" | "SEAB" | "SEABANK" => Some("970440"),
        "970454" | "BVBANK" | "VCCB" | "VIETCAPITALBANK" => Some("970454"),
        "546034" | "CAKE" => Some("546034"),
        "963388" | "TIMO" => Some("963388"),
        "970429" | "SCB" => Some("970429"),
        "970425" | "ABB" | "ABBANK" => Some("970425"),
        "970409" | "BAB" | "BACABANK" => Some("970409"),
        "970438" | "BAOVIET" | "BAOVIETBANK" | "BVB" => Some("970438"),
        "970444" | "CBB" | "CBBANK" => Some("970444"),
        "970408" | "GPB" | "GPBANK" => Some("970408"),
        "970452" | "KIENLONGBANK" | "KLB" => Some("970452"),
        "970428" | "NAB" | "NAMA" | "NAMABANK" => Some("970428"),
        "970419" | "NCB" => Some("970419"),
        "970414" | "MBV" | "OCEANBANK" => Some("970414"),
        "970430" | "PGB" | "PGBANK" => Some("970430"),
        "970412" | "PVCOMBANK" | "PVCB" => Some("970412"),
        "970400" | "SAIGONBANK" | "SGB" | "SGICB" => Some("970400"),
        "970427" | "VAB" | "VIETA" | "VIETABANK" => Some("970427"),
        "970433" | "VIETBANK" => Some("970433"),
        "970446" | "COOPBANK" => Some("970446"),
        "546035" | "UBANK" => Some("546035"),
        "970406" | "VIKKI" => Some("970406"),
        "999888" | "VBSP" => Some("999888"),
        "971005" | "VIETTEL" | "VIETTELMONEY" | "VIETTELPAY" | "VTLMONEY" => Some("971005"),
        "971011" | "VNPT" | "VNPTMONEY" => Some("971011"),
        "971025" | "MOMO" => Some("971025"),
        "971101" | "ZALO" | "ZALOPAY" => Some("971101"),
        "971133" | "PVDB" => Some("971133"),
        "977777" | "MAFC" => Some("977777"),
        "422589" | "CIMB" => Some("422589"),
        "796500" | "DBS" => Some("796500"),
        "970442" | "HLBVN" | "HONGLLEONG" => Some("970442"),
        "458761" | "HSBC" => Some("458761"),
        "970455" | "IBKHN" => Some("970455"),
        "970456" | "IBKHCM" => Some("970456"),
        "970434" | "IVB" | "INDOVINABANK" => Some("970434"),
        "801011" | "NHB" | "NONGHYUP" => Some("801011"),
        "970439" | "PBVN" | "PUBLICBANK" => Some("970439"),
        "970410" | "SCVN" | "STANDARDCHARTERED" => Some("970410"),
        "970424" | "SHBVN" | "SHINHAN" | "SHINHANBANK" => Some("970424"),
        "970458" | "UNITEDOVERSEAS" | "UOB" => Some("970458"),
        "970421" | "VRB" => Some("970421"),
        "970457" | "WOORI" | "WOORIBANK" | "WVN" => Some("970457"),
        "970462" | "KBHN" => Some("970462"),
        "970463" | "KBHCM" => Some("970463"),
        "668888" | "KBANK" => Some("668888"),
        "533948" | "CITIBANK" => Some("533948"),
        "970466" | "KEBHANAHCM" => Some("970466"),
        "970467" | "KEBHANAHN" => Some("970467"),
        _ => None,
    }
}

/// Normalize bank code to standard display/lookup key.
pub fn normalize_bank_code_for_qr(code: &str) -> &'static str {
    let clean = code.trim().to_ascii_uppercase();
    match clean.as_str() {
        "970436" | "VCB" | "VIETCOMBANK" => "VCB",
        "970415" | "CTG" | "ICB" | "VIETINBANK" => "ICB",
        "970418" | "BID" | "BIDV" => "BIDV",
        "970422" | "MB" | "MBB" | "MBBANK" => "MB",
        "970407" | "TCB" | "TECHCOMBANK" => "TCB",
        "970416" | "ACB" => "ACB",
        "970432" | "VPB" | "VPBANK" => "VPB",
        "970423" | "TIENPHONG" | "TPB" | "TPBANK" => "TPB",
        "970403" | "SACOMBANK" | "STB" => "STB",
        "970405" | "AGRI" | "AGRIBANK" | "VARB" | "VBA" | "VBARD" => "VBA",
        "970441" | "VIB" => "VIB",
        "970443" | "SHB" => "SHB",
        "970426" | "MARITIMEBANK" | "MSB" => "MSB",
        "970448" | "OCB" => "OCB",
        "970437" | "HDB" | "HDBANK" => "HDB",
        "970449" | "LIENVIETPOSTBANK" | "LPB" | "LPBANK" => "LPB",
        "970431" | "EIB" | "EXIMBANK" => "EIB",
        "970440" | "SEAB" | "SEABANK" => "SEAB",
        "970454" | "BVBANK" | "VCCB" | "VIETCAPITALBANK" => "VCCB",
        "546034" | "CAKE" => "CAKE",
        "963388" | "TIMO" => "TIMO",
        "970429" | "SCB" => "SCB",
        "970425" | "ABB" | "ABBANK" => "ABB",
        "970409" | "BAB" | "BACABANK" => "BAB",
        "970438" | "BAOVIET" | "BAOVIETBANK" | "BVB" => "BVB",
        "970444" | "CBB" | "CBBANK" => "CBB",
        "970408" | "GPB" | "GPBANK" => "GPB",
        "970452" | "KIENLONGBANK" | "KLB" => "KLB",
        "970428" | "NAB" | "NAMA" | "NAMABANK" => "NAB",
        "970419" | "NCB" => "NCB",
        "970414" | "MBV" | "OCEANBANK" => "MBV",
        "970430" | "PGB" | "PGBANK" => "PGB",
        "970412" | "PVCOMBANK" | "PVCB" => "PVCB",
        "970400" | "SAIGONBANK" | "SGB" | "SGICB" => "SGB",
        "970427" | "VAB" | "VIETA" | "VIETABANK" => "VAB",
        "970433" | "VIETBANK" => "VIETBANK",
        "970446" | "COOPBANK" => "COOPBANK",
        "546035" | "UBANK" => "Ubank",
        "970406" | "VIKKI" => "Vikki",
        "999888" | "VBSP" => "VBSP",
        "971005" | "VIETTEL" | "VIETTELMONEY" | "VIETTELPAY" | "VTLMONEY" => "VTLMONEY",
        "971011" | "VNPT" | "VNPTMONEY" => "VNPTMONEY",
        "971025" | "MOMO" => "MOMO",
        "971101" | "ZALO" | "ZALOPAY" => "ZALOPAY",
        "422589" | "CIMB" => "CIMB",
        "796500" | "DBS" => "DBS",
        "458761" | "HSBC" => "HSBC",
        "970458" | "UNITEDOVERSEAS" | "UOB" => "UOB",
        "970457" | "WOORI" | "WOORIBANK" | "WVN" => "WVN",
        _ => "VIETQR",
    }
}

/// Helper function to build EMVCo TLV element: Tag (2 digits) + Length (2 digits) + Value
#[inline]
pub fn emvco_tlv(tag: &str, val: &str) -> String {
    format!("{}{:02}{}", tag, val.len(), val)
}

/// Build standard VietQR (NAPAS247) payload string with CRC16 checksum.
pub fn build_vietqr_payload(
    bank_code: &str,
    account: &str,
    amount: i64,
    memo: Option<&str>,
    account_name: Option<&str>,
) -> Result<String> {
    let bin = bank_code_to_bin(bank_code)
        .ok_or_else(|| anyhow::anyhow!("Unsupported or unknown bank code: {bank_code}"))?;

    let clean_acc = sanitize_vietqr_account(account);
    if clean_acc.is_empty() {
        bail!("Account number cannot be empty");
    }

    // EMVCo Tag 38 (Merchant Account Information)
    let tag_guid = emvco_tlv("00", "A000000727");
    let tag_bin = emvco_tlv("00", bin);
    let tag_acc = emvco_tlv("01", &clean_acc);
    let tag_napas_sub = format!("{}{}", tag_bin, tag_acc);
    let tag_napas = emvco_tlv("01", &tag_napas_sub);
    let tag_service = emvco_tlv("02", "QRIBFTTA");
    let tag38_val = format!("{}{}{}", tag_guid, tag_napas, tag_service);
    let tag38 = emvco_tlv("38", &tag38_val);

    let tag00 = emvco_tlv("00", "01"); // Format Indicator: 01
    let tag01 = if amount > 0 {
        emvco_tlv("01", "12") // Dynamic QR (one-time specific amount)
    } else {
        emvco_tlv("01", "11") // Static QR
    };
    let tag53 = emvco_tlv("53", "704"); // Currency: VND (704)
    let tag58 = emvco_tlv("58", "VN"); // Country code: VN

    let tag54 = if amount > 0 {
        emvco_tlv("54", &amount.to_string())
    } else {
        String::new()
    };

    let tag59 = if let Some(raw_name) = account_name {
        let clean_name = sanitize_vietqr_name(raw_name);
        if !clean_name.is_empty() {
            emvco_tlv("59", &clean_name)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let tag62 = if let Some(m) = memo {
        let clean_memo = sanitize_vietqr_memo(m, 50);
        if !clean_memo.is_empty() {
            let sub08 = emvco_tlv("08", &clean_memo);
            emvco_tlv("62", &sub08)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let payload_prefix = format!(
        "{}{}{}{}{}{}{}{}6304",
        tag00, tag01, tag38, tag53, tag54, tag58, tag59, tag62
    );

    let crc = format_crc16(payload_prefix.as_bytes());
    Ok(format!("{}{}", payload_prefix, crc))
}

/// Extract bank BIN (6 digits) from EMVCo Tag 38 -> Sub-tag 01 -> Sub-tag 00 if present.
pub fn extract_bin_from_emvco(qr_data: &str) -> Option<&str> {
    let mut rest = qr_data.trim();
    while rest.len() >= 4 {
        let tag = rest.get(..2)?;
        let len: usize = rest.get(2..4)?.parse().ok()?;
        rest = rest.get(4..)?;
        let val = rest.get(..len)?;
        rest = rest.get(len..)?;
        if tag == "38" {
            let mut t38 = val;
            while t38.len() >= 4 {
                let Some(st) = t38.get(..2) else { break };
                let Some(slen) = t38.get(2..4).and_then(|s| s.parse::<usize>().ok()) else {
                    break;
                };
                let Some(after_hdr) = t38.get(4..) else {
                    break;
                };
                let Some(sval) = after_hdr.get(..slen) else {
                    break;
                };
                t38 = after_hdr.get(slen..).unwrap_or("");
                if st == "01" {
                    let mut t01 = sval;
                    while t01.len() >= 4 {
                        let Some(sst) = t01.get(..2) else { break };
                        let Some(sslen) = t01.get(2..4).and_then(|s| s.parse::<usize>().ok())
                        else {
                            break;
                        };
                        let Some(after_sub) = t01.get(4..) else {
                            break;
                        };
                        let Some(ssval) = after_sub.get(..sslen) else {
                            break;
                        };
                        t01 = after_sub.get(sslen..).unwrap_or("");
                        if sst == "00" && !ssval.trim().is_empty() {
                            return Some(ssval.trim());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Automatically determine short bank brand label from EMVCo content.
pub fn detect_qr_center_label(content: &str) -> &'static str {
    let trimmed = content.trim();
    if trimmed.starts_with("000201") {
        if let Some(bin) = extract_bin_from_emvco(trimmed) {
            return normalize_bank_code_for_qr(bin);
        }
        return "VIETQR";
    }
    "PAY"
}

/// Return brand RGB color tuple for the bank code.
pub fn qr_center_badge_brand_color(label: &str) -> (u8, u8, u8) {
    match label.trim().to_ascii_uppercase().as_str() {
        "VCB" => (0, 114, 54), // Vietcombank Forest Green (#007236)
        "MB" | "ACB" | "ICB" | "VIB" | "SHB" | "STB" | "BVB" | "CBB" | "GPB" | "KLB"
        | "MBV" | "PVCB" | "SGB" | "VAB" | "UOB" | "WVN" | "DBS" | "CIMB" => {
            (24, 73, 214) // Royal Banking Blue (#1849D6)
        }
        "TCB" | "VBA" | "MSB" | "HDB" | "SCB" | "LPB" | "NCB" | "HSBC" => {
            (209, 25, 32) // Techcombank / Agribank Crimson Red (#D11920)
        }
        "BIDV" | "ABB" | "SEAB" => (0, 107, 104), // BIDV Emerald Teal (#006B68)
        "VPB" | "OCB" | "BAB" => (0, 138, 69),    // VPBank / OCB Vibrant Green (#008A45)
        "TPB" | "TIMO" => (109, 40, 217),         // TPBank / Timo Royal Purple (#6D28D9)
        "CAKE" | "MOMO" => (216, 27, 96),         // MoMo / Cake Magenta Pink (#D81B60)
        "EIB" | "VCCB" | "NAB" | "PGB" => (2, 132, 199), // Eximbank / Nam A Ocean Blue (#0284C7)
        _ => (5, 122, 85),                        // Default VietQR Emerald (#057A55)
    }
}

/// Return full friendly Vietnamese bank name for display.
pub fn bank_display_name(code: &str) -> &'static str {
    let clean = code.trim().to_ascii_uppercase();
    match clean.as_str() {
        "970423" | "TIENPHONG" | "TPB" | "TPBANK" => "TPBank (Tien Phong)",
        "970436" | "VCB" | "VIETCOMBANK" => "Vietcombank (Ngoai Thuong)",
        "970422" | "MB" | "MBB" | "MBBANK" => "MB Bank (Quan Doi)",
        "970407" | "TCB" | "TECHCOMBANK" => "Techcombank (Ky Thuong)",
        "970418" | "BID" | "BIDV" => "BIDV (Dau tu & Phat trien)",
        "970415" | "CTG" | "ICB" | "VIETINBANK" => "VietinBank (Cong Thuong)",
        "970432" | "VPB" | "VPBANK" => "VPBank (Viet Nam Thinh Vuong)",
        "970416" | "ACB" => "ACB (A Chau)",
        "970405" | "AGRI" | "AGRIBANK" | "VARB" | "VBA" | "VBARD" => "Agribank (Nong Nghiep & PTNT)",
        "970441" | "VIB" => "VIB (Quoc Te)",
        "970443" | "SHB" => "SHB (Sai Gon - Ha Noi)",
        "970403" | "SACOMBANK" | "STB" => "Sacombank (Sai Gon Thuong Tin)",
        "970426" | "MARITIMEBANK" | "MSB" => "MSB (Hang Hai)",
        "970448" | "OCB" => "OCB (Phuong Dong)",
        "970437" | "HDB" | "HDBANK" => "HDBank (Phat trien TP.HCM)",
        "970449" | "LIENVIETPOSTBANK" | "LPB" | "LPBANK" => "LPBank (Loc Phat)",
        "970440" | "SEAB" | "SEABANK" => "SeABank (Dong Nam A)",
        "546034" | "CAKE" => "CAKE by VPBank",
        "963388" | "TIMO" => "Timo by BVBank",
        "970454" | "BVBANK" | "VCCB" | "VIETCAPITALBANK" => "BVBank (Ban Viet)",
        "970425" | "ABB" | "ABBANK" => "ABBank (An Binh)",
        "970431" | "EIB" | "EXIMBANK" => "Eximbank (Xuat Nhap Khau)",
        "970428" | "NAB" | "NAMA" | "NAMABANK" => "Nam A Bank (Nam A)",
        "970419" | "NCB" => "NCB (Quoc Dan)",
        "970438" | "BAOVIET" | "BAOVIETBANK" | "BVB" => "BaoViet Bank (Bao Viet)",
        "970452" | "KIENLONGBANK" | "KLB" => "KienlongBank (Kien Long)",
        "970412" | "PVCOMBANK" | "PVCB" => "PVcomBank (Dai Chung)",
        "970400" | "SAIGONBANK" | "SGB" | "SGICB" => "Saigonbank (Sai Gon Cong Thuong)",
        "970427" | "VAB" | "VIETA" | "VIETABANK" => "VietABank (Viet A)",
        "970457" | "WOORI" | "WOORIBANK" | "WVN" => "Woori Bank Vietnam",
        "970424" | "SHBVN" | "SHINHAN" | "SHINHANBANK" => "Shinhan Bank Vietnam",
        "458761" | "HSBC" => "HSBC Vietnam",
        "422589" | "CIMB" => "CIMB Bank Vietnam",
        _ => "Ngan Hang 24/7 (VietQR)",
    }
}

/// Format bank account string with readable spaces or optional masking.
pub fn format_bank_account_spaced(account: &str, full_acc: bool) -> String {
    let clean = sanitize_vietqr_account(account);
    if clean.is_empty() {
        return String::new();
    }
    if !full_acc && clean.len() >= 8 {
        let prefix = &clean[..4];
        let suffix = &clean[clean.len() - 3..];
        return format!("{} **** {}", prefix, suffix);
    }
    // Group characters into readable 4-digit chunks
    let mut chunks = Vec::new();
    let mut chars = clean.chars().peekable();
    while chars.peek().is_some() {
        let chunk: String = chars.by_ref().take(4).collect();
        chunks.push(chunk);
    }
    chunks.join(" ")
}

/// Format amount into standard Vietnamese currency string (e.g. `50.000 VND`).
pub fn format_vnd_amount(amount: i64) -> String {
    if amount <= 0 {
        return String::new();
    }
    let s = amount.to_string();
    let mut res = String::new();
    let len = s.len();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            res.push('.');
        }
        res.push(c);
    }
    format!("{} VND", res)
}

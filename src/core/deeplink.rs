use serde::Serialize;
use urlencoding::encode;

use crate::core::emvco::{normalize_bank_code_for_qr, sanitize_vietqr_account, sanitize_vietqr_name};

#[derive(Debug, Clone, Serialize)]
pub struct BankAppDeeplinkSpec {
    pub app_id: &'static str,
    pub app_name: &'static str,
    pub bank_name: &'static str,
    pub ios_scheme: Option<&'static str>,
    pub android_intent_host: &'static str,
    pub android_scheme: &'static str,
    pub android_package: &'static str,
    pub autofill: bool,
}

pub const BANK_APP_DEEPLINKS: [BankAppDeeplinkSpec; 35] = [
    BankAppDeeplinkSpec {
        app_id: "mb",
        app_name: "MB Bank",
        bank_name: "Ngân hàng TMCP Quân đội",
        ios_scheme: Some("mbbank://"),
        android_intent_host: "",
        android_scheme: "mbbank",
        android_package: "com.mbmobile",
        autofill: true,
    },
    BankAppDeeplinkSpec {
        app_id: "vcb",
        app_name: "VCB Digibank",
        bank_name: "Ngân hàng TMCP Ngoại Thương Việt Nam",
        ios_scheme: Some("vietcombankmobile://"),
        android_intent_host: "",
        android_scheme: "vietcombankmobile",
        android_package: "com.VCB",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "tcb",
        app_name: "Techcombank Mobile",
        bank_name: "Ngân hàng TMCP Kỹ thương Việt Nam",
        ios_scheme: Some("tcb://"),
        android_intent_host: "applink",
        android_scheme: "tcb",
        android_package: "vn.com.techcombank.bb.app",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "bidv",
        app_name: "BIDV SmartBanking",
        bank_name: "Ngân hàng TMCP Đầu tư và Phát triển Việt Nam",
        ios_scheme: Some("bidv.smartbanking.partner://"),
        android_intent_host: "payment",
        android_scheme: "bidv.smartbanking.partner",
        android_package: "com.vnpay.bidv",
        autofill: true,
    },
    BankAppDeeplinkSpec {
        app_id: "tpb",
        app_name: "TPBank Mobile",
        bank_name: "Ngân hàng TMCP Tiên Phong",
        ios_scheme: Some("hydro://"),
        android_intent_host: "onboarding",
        android_scheme: "hydro",
        android_package: "com.tpb.mb.gprsandroid",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "icb",
        app_name: "VietinBank iPay",
        bank_name: "Ngân hàng TMCP Công thương Việt Nam",
        ios_scheme: Some("vietinbankipay://"),
        android_intent_host: "",
        android_scheme: "vietinbankipay",
        android_package: "com.vietinbank.ipay",
        autofill: true,
    },
    BankAppDeeplinkSpec {
        app_id: "vpb",
        app_name: "VPBank NEO",
        bank_name: "Ngân hàng TMCP Việt Nam Thịnh Vượng",
        ios_scheme: Some("vpbankneo://"),
        android_intent_host: "",
        android_scheme: "vpbankneo",
        android_package: "com.vnpay.vpbankonline",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "acb",
        app_name: "ACB ONE",
        bank_name: "Ngân hàng TMCP Á Châu",
        ios_scheme: Some("acbone://"),
        android_intent_host: "",
        android_scheme: "acbone",
        android_package: "mobile.acb.com.vn",
        autofill: true,
    },
    BankAppDeeplinkSpec {
        app_id: "vba",
        app_name: "Agribank Plus",
        bank_name: "Ngân hàng Nông nghiệp và Phát triển Nông thôn Việt Nam",
        ios_scheme: Some("agribankmobile://"),
        android_intent_host: "",
        android_scheme: "agribankmobile",
        android_package: "com.vnpay.Agribank3g",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "vib-2",
        app_name: "MyVIB",
        bank_name: "Ngân hàng TMCP Quốc tế Việt Nam",
        ios_scheme: Some("myvib2://"),
        android_intent_host: "myvib2.com.vn/data",
        android_scheme: "myvib2",
        android_package: "com.vib.myvib2",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "ocb",
        app_name: "OCB OMNI",
        bank_name: "Ngân hàng TMCP Phương Đông",
        ios_scheme: Some("newomni-app://"),
        android_intent_host: "",
        android_scheme: "newomni-app",
        android_package: "vn.com.ocb.awe",
        autofill: true,
    },
    BankAppDeeplinkSpec {
        app_id: "vib",
        app_name: "MyVIB (Android)",
        bank_name: "Ngân hàng TMCP Quốc tế Việt Nam",
        ios_scheme: Some("myvib2://"),
        android_intent_host: "vib.com.vn/data",
        android_scheme: "myvib",
        android_package: "com.vib.myvib2",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "shb",
        app_name: "SHB SAHA",
        bank_name: "Ngân hàng TMCP Sài Gòn - Hà Nội",
        ios_scheme: Some("shbmobile://"),
        android_intent_host: "",
        android_scheme: "shbmobile",
        android_package: "vn.shb.saha.mbanking",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "lpb",
        app_name: "LPBank",
        bank_name: "Ngân hàng TMCP Bưu Điện Liên Việt",
        ios_scheme: Some("lv24h://"),
        android_intent_host: "",
        android_scheme: "lv24h",
        android_package: "vn.com.lpb.lienviet24h",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "seab",
        app_name: "SeAMobile",
        bank_name: "Ngân hàng TMCP Đông Nam Á",
        ios_scheme: Some("seamobile://"),
        android_intent_host: "app",
        android_scheme: "seamobile",
        android_package: "vn.com.seabank.mb1",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "scb",
        app_name: "SCB Mobile Banking",
        bank_name: "Ngân hàng TMCP Sài Gòn",
        ios_scheme: Some("scbmobilebanking://"),
        android_intent_host: "",
        android_scheme: "scbmobilebanking",
        android_package: "com.vnpay.SCB",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "cake",
        app_name: "CAKE by VPBank",
        bank_name: "Ngân hàng số CAKE by VPBank",
        ios_scheme: Some("cake.vn://"),
        android_intent_host: "",
        android_scheme: "cake.vn",
        android_package: "xyz.be.cake",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "hdb",
        app_name: "HDBank",
        bank_name: "Ngân hàng TMCP Phát triển TP.HCM",
        ios_scheme: Some("hdbankmobile://"),
        android_intent_host: "",
        android_scheme: "hdbankmobile",
        android_package: "com.vnpay.hdbank",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "timo",
        app_name: "Timo by BVBank",
        bank_name: "Ngân hàng TMCP Bản Việt",
        ios_scheme: Some("plus://"),
        android_intent_host: "",
        android_scheme: "plus",
        android_package: "io.lifestyle.plus",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "shbvn",
        app_name: "Shinhan SOL Vietnam",
        bank_name: "Ngân hàng TNHH MTV Shinhan Việt Nam",
        ios_scheme: Some("shinhanglbvnbank://"),
        android_intent_host: "",
        android_scheme: "shinhanglbvnbank",
        android_package: "com.shinhan.global.vn.bank",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "nab",
        app_name: "Nam A Bank Open Banking",
        bank_name: "Ngân hàng TMCP Nam Á",
        ios_scheme: None,
        android_intent_host: "nab/softotp",
        android_scheme: "deeplinkapp",
        android_package: "ops.namabank.com.vn",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "abb",
        app_name: "ABBank",
        bank_name: "Ngân hàng TMCP An Bình",
        ios_scheme: Some("abbankmobile://"),
        android_intent_host: "",
        android_scheme: "abbankmobile",
        android_package: "vn.abbank.retail",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "eib",
        app_name: "Eximbank EDigi",
        bank_name: "Ngân hàng TMCP Xuất Nhập khẩu Việt Nam",
        ios_scheme: Some("eximbankmobile://"),
        android_intent_host: "",
        android_scheme: "eximbankmobile",
        android_package: "com.vnpay.EximBankOmni",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "coopbank",
        app_name: "Co-opBank Mobile Banking",
        bank_name: "Ngân hàng Hợp tác xã Việt Nam",
        ios_scheme: Some("coopbankmobile://"),
        android_intent_host: "",
        android_scheme: "coopbankmobile",
        android_package: "com.vnpay.coopbank",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "pvcb",
        app_name: "PVConnect",
        bank_name: "Ngân hàng TMCP Đại Chúng Việt Nam",
        ios_scheme: Some("pvcombankapp://"),
        android_intent_host: "",
        android_scheme: "pvcombankapp",
        android_package: "com.pvcombank.retail",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "wvn",
        app_name: "Woori WON Vietnam",
        bank_name: "Ngân hàng TNHH MTV Woori Việt Nam",
        ios_scheme: Some("wvbs://"),
        android_intent_host: "",
        android_scheme: "wvbs",
        android_package: "vn.com.woori.smart",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "klb",
        app_name: "KienlongBank Plus",
        bank_name: "Ngân hàng TMCP Kiên Long",
        ios_scheme: Some("kienlongbankmobilebanking://"),
        android_intent_host: "ksbank.co",
        android_scheme: "ksbank",
        android_package: "com.sunshine.ksbank",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "bvb",
        app_name: "BAOVIET Smart",
        bank_name: "Ngân hàng TMCP Bảo Việt",
        ios_scheme: Some("baovietmobile://"),
        android_intent_host: "",
        android_scheme: "baovietmobile",
        android_package: "com.vnpay.bvbank",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "vab",
        app_name: "Vietbank Digital Plus",
        bank_name: "Ngân hàng TMCP Việt Á",
        ios_scheme: Some("vabmobilebanking://"),
        android_intent_host: "",
        android_scheme: "vabmobilebanking",
        android_package: "com.vn.vietbankomni",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "ncb",
        app_name: "NCB iziMobile",
        bank_name: "Ngân hàng TMCP Quốc Dân",
        ios_scheme: Some("ncbizimobile://"),
        android_intent_host: "",
        android_scheme: "ncbizimobile",
        android_package: "com.ncb.bank",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "acb-biz",
        app_name: "ACB ONE Biz",
        bank_name: "Ngân hàng TMCP Á Châu",
        ios_scheme: Some("abaapp://"),
        android_intent_host: "",
        android_scheme: "abaapp",
        android_package: "com.acb.acbb.prod",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "oceanbank",
        app_name: "Ocean Bank Mobile Banking",
        bank_name: "Ngân hàng Thương mại TNHH MTV Đại Dương",
        ios_scheme: Some("oceanbankmobilebanking://"),
        android_intent_host: "",
        android_scheme: "oceanbankmobilebanking",
        android_package: "com.aci.ocean.mobile",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "pbvn",
        app_name: "PB engage VN",
        bank_name: "Ngân hàng TNHH MTV Public Việt Nam",
        ios_scheme: Some("publicbankmobile://"),
        android_intent_host: "",
        android_scheme: "publicbankmobile",
        android_package: "com.vnpay.publicbank",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "sgicb",
        app_name: "SAIGONBANK Smart Banking",
        bank_name: "Ngân hàng TMCP Sài Gòn Công Thương",
        ios_scheme: Some("saigonbankmobilebanking://"),
        android_intent_host: "",
        android_scheme: "Sgbmobile",
        android_package: "com.vnpay.sgbank",
        autofill: false,
    },
    BankAppDeeplinkSpec {
        app_id: "cimb",
        app_name: "OCTO by CIMB Vietnam",
        bank_name: "Ngân hàng TNHH MTV CIMB Việt Nam",
        ios_scheme: Some("cimb://"),
        android_intent_host: "",
        android_scheme: "cimb",
        android_package: "vn.cimbbank.octo",
        autofill: false,
    },
];

pub fn find_bank_app_deeplink(app_id: &str) -> Option<&'static BankAppDeeplinkSpec> {
    let clean = app_id.trim().to_ascii_lowercase();
    if let Some(found) = BANK_APP_DEEPLINKS.iter().find(|s| s.app_id == clean) {
        return Some(found);
    }
    let norm = normalize_bank_code_for_qr(&clean).to_ascii_lowercase();
    BANK_APP_DEEPLINKS.iter().find(|s| s.app_id == norm)
}

pub fn build_android_intent_uri(spec: &BankAppDeeplinkSpec, fallback_url: &str) -> String {
    format!(
        "intent://{host}#Intent;scheme={scheme};package={pkg};S.browser_fallback_url={fb};end",
        host = spec.android_intent_host,
        scheme = spec.android_scheme,
        pkg = spec.android_package,
        fb = encode(fallback_url),
    )
}

pub fn parse_ba_param(ba: &str) -> Option<(String, String)> {
    let (acc_raw, bank_raw) = ba.trim().split_once('@')?;
    let clean_acc = sanitize_vietqr_account(acc_raw);
    let bank_norm = normalize_bank_code_for_qr(bank_raw);
    if clean_acc.is_empty() || bank_norm.is_empty() {
        return None;
    }
    Some((clean_acc, bank_norm.to_string()))
}

pub fn selectable_bank_app_deeplinks() -> Vec<&'static BankAppDeeplinkSpec> {
    BANK_APP_DEEPLINKS
        .iter()
        .filter(|s| s.app_id != "vib" && s.app_id != "acb-biz")
        .collect()
}

pub fn bank_app_launch_url_for_app(
    public_origin: Option<&str>,
    target_app_id: &str,
    receiving_bank_code: &str,
    receiving_account: &str,
    amount: i64,
    memo: Option<&str>,
    account_name: Option<&str>,
) -> Option<String> {
    let spec = find_bank_app_deeplink(target_app_id)?;
    let clean_acc = sanitize_vietqr_account(receiving_account);
    if clean_acc.is_empty() {
        return None;
    }
    let receiving_norm = normalize_bank_code_for_qr(receiving_bank_code).to_ascii_lowercase();
    let base = public_origin.unwrap_or("").trim_end_matches('/');

    let mut hub_url = format!(
        "{base}/qr/pay?app={app}&ba={acc}@{bank}&am={am}",
        app = spec.app_id,
        acc = encode(&clean_acc),
        bank = encode(&receiving_norm),
        am = amount.max(0)
    );
    if let Some(m) = memo {
        if !m.trim().is_empty() {
            hub_url.push_str("&tn=");
            hub_url.push_str(&encode(m.trim()));
        }
    }
    if let Some(n) = account_name {
        let clean_n = sanitize_vietqr_name(n);
        if !clean_n.is_empty() {
            hub_url.push_str("&bn=");
            hub_url.push_str(&encode(&clean_n));
        }
    }
    Some(hub_url)
}

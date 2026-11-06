use urlencoding::encode;

use crate::core::deeplink::{
    BANK_APP_DEEPLINKS, build_android_intent_uri,
};
use crate::core::emvco::{
    normalize_bank_code_for_qr, sanitize_vietqr_account, sanitize_vietqr_memo,
    sanitize_vietqr_name,
};
use crate::ui::PAY_HUB_HTML;

pub fn escape_html_text(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn format_vnd_dots(amount: i64) -> String {
    let s = amount.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3 + 4);
    let bytes = s.as_bytes();
    let len = bytes.len();
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(b as char);
    }
    out.push_str(" đ");
    out
}

pub fn render_interactive_bank_pay_hub_html(
    bank_code: &str,
    account: &str,
    amount: i64,
    memo: Option<&str>,
    account_name: Option<&str>,
    auto_app: Option<&str>,
) -> String {
    let norm_bank = normalize_bank_code_for_qr(bank_code);
    let clean_acc = sanitize_vietqr_account(account);
    let clean_memo = memo
        .map(|m| sanitize_vietqr_memo(m, 50))
        .unwrap_or_default();
    let clean_name = account_name.map(sanitize_vietqr_name).unwrap_or_default();

    let mut qr_img_url = format!(
        "/api/qr.png?bank={}&acc={}&amount={}",
        encode(norm_bank),
        encode(&clean_acc),
        amount.max(0)
    );
    if !clean_memo.is_empty() {
        qr_img_url.push_str("&memo=");
        qr_img_url.push_str(&encode(&clean_memo));
    }
    if !clean_name.is_empty() {
        qr_img_url.push_str("&name=");
        qr_img_url.push_str(&encode(&clean_name));
    }

    let mut self_return_base = format!(
        "/qr/pay?ba={}%40{}&am={}",
        encode(&clean_acc),
        encode(norm_bank),
        amount.max(0)
    );
    if !clean_memo.is_empty() {
        self_return_base.push_str("&tn=");
        self_return_base.push_str(&encode(&clean_memo));
    }
    if !clean_name.is_empty() {
        self_return_base.push_str("&bn=");
        self_return_base.push_str(&encode(&clean_name));
    }

    let mut bank_buttons_html = String::with_capacity(4096);
    for spec in &BANK_APP_DEEPLINKS {
        if spec.app_id == "vib" || spec.app_id == "acb-biz" {
            continue;
        }
        let ios_scheme = spec.ios_scheme.unwrap_or("");
        let fallback_url = format!("{self_return_base}#qr-card");
        let android_intent = build_android_intent_uri(spec, &fallback_url);
        let badge_tag = if spec.autofill {
            r#"<span class="tag-auto">Tự điền</span>"#
        } else {
            ""
        };
        let self_app_url = format!("{self_return_base}&app={app}", app = spec.app_id);
        bank_buttons_html.push_str(&format!(
            r#"<button type="button" class="bank-btn" data-app="{app_id}" data-name="{app_name}" data-ios="{ios}" data-android="{android}" data-autofill="{autofill}" data-link="{link}">
  <span class="bank-code">{code}</span>
  <span class="bank-meta">
    <span class="bank-title">{app_name}</span>
    {badge_tag}
  </span>
</button>"#,
            app_id = spec.app_id,
            code = spec.app_id.to_ascii_uppercase(),
            app_name = escape_html_text(spec.app_name),
            ios = escape_html_text(ios_scheme),
            android = escape_html_text(&android_intent),
            autofill = if spec.autofill { "1" } else { "0" },
            link = escape_html_text(&self_app_url),
            badge_tag = badge_tag,
        ));
    }

    let formatted_amount = if amount > 0 {
        format_vnd_dots(amount)
    } else {
        "Tùy chọn".to_string()
    };
    let holder_row = if !clean_name.is_empty() {
        format!(
            r#"<div class="info-row"><span class="info-lbl">Chủ tài khoản</span><span class="info-val">{}</span></div>"#,
            escape_html_text(&clean_name)
        )
    } else {
        String::new()
    };
    let memo_row = if !clean_memo.is_empty() {
        format!(
            r#"<div class="info-row"><span class="info-lbl">Nội dung CK</span><span class="info-val mono">{memo} <button type="button" class="copy-pill" data-copy="{memo}">Sao chép</button></span></div>"#,
            memo = escape_html_text(&clean_memo)
        )
    } else {
        String::new()
    };

    let auto_app_attr = escape_html_text(auto_app.unwrap_or(""));

    PAY_HUB_HTML
        .replace("{{AUTO_APP}}", &auto_app_attr)
        .replace("{{BANK}}", &escape_html_text(norm_bank))
        .replace("{{ACCOUNT}}", &escape_html_text(&clean_acc))
        .replace("{{AMOUNT}}", &escape_html_text(&formatted_amount))
        .replace("{{RAW_AMOUNT}}", &amount.max(0).to_string())
        .replace("{{QR_IMG_URL}}", &escape_html_text(&qr_img_url))
        .replace("{{HOLDER_ROW}}", &holder_row)
        .replace("{{MEMO_ROW}}", &memo_row)
        .replace("{{BANK_BUTTONS}}", &bank_buttons_html)
}

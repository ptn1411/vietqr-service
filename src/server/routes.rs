use axum::extract::{Multipart, Query};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::core::canvas::{
    CenterLogo, QrFooterOptions, QrRenderOptions, QrTemplate, render_vietqr_with_options,
};
use crate::core::deeplink::{
    bank_app_launch_url_for_app, parse_ba_param, selectable_bank_app_deeplinks,
};
use crate::core::emvco::{
    build_vietqr_payload, normalize_bank_code_for_qr, sanitize_vietqr_account,
    sanitize_vietqr_memo, sanitize_vietqr_name,
};
use crate::server::web_hub::render_interactive_bank_pay_hub_html;
use crate::ui::STUDIO_HTML;

#[derive(Debug, Deserialize)]
pub struct QrQueryParams {
    pub bank: Option<String>,
    pub acc: Option<String>,
    pub amount: Option<i64>,
    pub memo: Option<String>,
    pub des: Option<String>,
    pub name: Option<String>,
    pub template: Option<String>,
    pub showinfo: Option<bool>,
    pub fullacc: Option<bool>,
    pub download: Option<bool>,
    pub footer1: Option<String>,
    pub footer2: Option<String>,
    pub badge: Option<String>,
    pub scale: Option<usize>,
    pub square: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PayQueryParams {
    pub app: Option<String>,
    pub ba: Option<String>,
    pub bank: Option<String>,
    pub acc: Option<String>,
    pub am: Option<i64>,
    pub amount: Option<i64>,
    pub tn: Option<String>,
    pub memo: Option<String>,
    pub des: Option<String>,
    pub bn: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PayApiResponse {
    pub payload: String,
    pub qr_image_url: String,
    pub launch_url: Option<String>,
}

pub fn app_router() -> Router {
    Router::new()
        .route("/", get(studio_handler))
        .route("/api/qr.png", get(qr_png_get_handler).post(qr_png_post_handler))
        .route("/qr.png", get(qr_png_get_handler))
        .route("/img", get(qr_png_get_handler).post(qr_png_post_handler))
        .route("/api/banks", get(banks_list_handler))
        .route("/api/pay", get(pay_api_handler))
        .route("/qr/pay", get(pay_hub_html_handler))
}

async fn studio_handler() -> Html<&'static str> {
    Html(STUDIO_HTML)
}

async fn banks_list_handler() -> impl IntoResponse {
    Json(selectable_bank_app_deeplinks())
}

async fn qr_png_get_handler(Query(params): Query<QrQueryParams>) -> Response {
    let bank_raw = params.bank.as_deref().unwrap_or("TPB");
    let bank = normalize_bank_code_for_qr(bank_raw);
    let acc = sanitize_vietqr_account(params.acc.as_deref().unwrap_or("69238686888"));
    let amount = params.amount.unwrap_or(0);
    let memo = params.memo.as_deref().or(params.des.as_deref());
    let name = params.name.as_deref();

    let payload = match build_vietqr_payload(bank, &acc, amount, memo, name) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Failed to build VietQR payload: {e}"),
            )
                .into_response();
        }
    };

    let scale = params.scale.unwrap_or(8);
    let is_square = params.square.unwrap_or(false);
    let show_info = params.showinfo.unwrap_or(true);
    let full_acc = params.fullacc.unwrap_or(true);
    let template = if is_square {
        QrTemplate::QrOnly
    } else {
        QrTemplate::from_str_opt(params.template.as_deref())
    };

    let logo = if let Some(ref badge) = params.badge {
        CenterLogo::TextBadge(badge)
    } else {
        CenterLogo::AutoBadge(bank)
    };

    let footer = QrFooterOptions {
        line1: params.footer1.as_deref(),
        line2: params.footer2.as_deref(),
        is_square,
    };

    let render_opts = QrRenderOptions {
        payload: &payload,
        logo,
        template,
        scale,
        show_info,
        full_acc,
        bank_code: bank,
        account: &acc,
        account_name: name,
        amount,
        memo,
        footer,
    };

    match render_vietqr_with_options(render_opts) {
        Ok(png_bytes) => {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=300"),
            );
            if params.download.unwrap_or(false) {
                let filename = format!("vietqr-{}-{}.png", bank.to_ascii_lowercase(), acc);
                if let Ok(val) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
                    headers.insert(header::CONTENT_DISPOSITION, val);
                }
            }
            (StatusCode::OK, headers, png_bytes).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Canvas render error: {e}"),
        )
            .into_response(),
    }
}

async fn qr_png_post_handler(mut multipart: Multipart) -> Response {
    let mut bank = "TPB".to_string();
    let mut acc = "69238686888".to_string();
    let mut amount: i64 = 0;
    let mut memo: Option<String> = None;
    let mut name: Option<String> = None;
    let mut template_str: Option<String> = None;
    let mut show_info: bool = true;
    let mut full_acc: bool = true;
    let mut download: bool = false;
    let mut footer1: Option<String> = None;
    let mut footer2: Option<String> = None;
    let mut badge: Option<String> = None;
    let mut scale: usize = 8;
    let mut square: bool = false;
    let mut logo_bytes: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let field_name = field.name().unwrap_or_default().to_string();
        if field_name == "logo" {
            if let Ok(bytes) = field.bytes().await {
                if !bytes.is_empty() {
                    logo_bytes = Some(bytes.to_vec());
                }
            }
        } else if let Ok(text) = field.text().await {
            match field_name.as_str() {
                "bank" => bank = text,
                "acc" => acc = text,
                "amount" => amount = text.parse().unwrap_or(0),
                "memo" | "des" => memo = Some(text),
                "name" => name = Some(text),
                "template" => template_str = Some(text),
                "showinfo" => show_info = text.eq_ignore_ascii_case("true") || text == "1",
                "fullacc" => full_acc = text.eq_ignore_ascii_case("true") || text == "1",
                "download" => download = text.eq_ignore_ascii_case("true") || text == "1",
                "footer1" => footer1 = Some(text),
                "footer2" => footer2 = Some(text),
                "badge" => badge = Some(text),
                "scale" => scale = text.parse().unwrap_or(8),
                "square" => square = text.eq_ignore_ascii_case("true") || text == "1",
                _ => {}
            }
        }
    }

    let norm_bank = normalize_bank_code_for_qr(&bank);
    let clean_acc = sanitize_vietqr_account(&acc);

    let payload = match build_vietqr_payload(
        norm_bank,
        &clean_acc,
        amount,
        memo.as_deref(),
        name.as_deref(),
    ) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Failed to build VietQR payload: {e}"),
            )
                .into_response();
        }
    };

    let template = if square {
        QrTemplate::QrOnly
    } else {
        QrTemplate::from_str_opt(template_str.as_deref())
    };

    let logo = if let Some(ref img_bytes) = logo_bytes {
        CenterLogo::ImageLogo(img_bytes)
    } else if let Some(ref b) = badge {
        CenterLogo::TextBadge(b)
    } else {
        CenterLogo::AutoBadge(norm_bank)
    };

    let footer = QrFooterOptions {
        line1: footer1.as_deref(),
        line2: footer2.as_deref(),
        is_square: square,
    };

    let render_opts = QrRenderOptions {
        payload: &payload,
        logo,
        template,
        scale,
        show_info,
        full_acc,
        bank_code: norm_bank,
        account: &clean_acc,
        account_name: name.as_deref(),
        amount,
        memo: memo.as_deref(),
        footer,
    };

    match render_vietqr_with_options(render_opts) {
        Ok(png_bytes) => {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=300"),
            );
            if download {
                let filename = format!("vietqr-{}-{}.png", norm_bank.to_ascii_lowercase(), clean_acc);
                if let Ok(val) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
                    headers.insert(header::CONTENT_DISPOSITION, val);
                }
            }
            (StatusCode::OK, headers, png_bytes).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Canvas render error: {e}"),
        )
            .into_response(),
    }
}

async fn pay_api_handler(Query(params): Query<PayQueryParams>) -> Response {
    let (clean_acc, norm_bank) = if let Some(ref ba) = params.ba {
        parse_ba_param(ba).unwrap_or(("69238686888".to_string(), "TPB".to_string()))
    } else {
        let b = params.bank.as_deref().unwrap_or("TPB");
        let a = params.acc.as_deref().unwrap_or("69238686888");
        (
            sanitize_vietqr_account(a),
            normalize_bank_code_for_qr(b).to_string(),
        )
    };

    let amount = params.am.or(params.amount).unwrap_or(0);
    let memo = params.tn.as_deref().or(params.memo.as_deref()).or(params.des.as_deref());
    let name = params.bn.as_deref().or(params.name.as_deref());

    let payload = match build_vietqr_payload(&norm_bank, &clean_acc, amount, memo, name) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Failed to build payload: {e}"),
            )
                .into_response();
        }
    };

    let mut qr_image_url = format!(
        "/api/qr.png?bank={}&acc={}&amount={}",
        urlencoding::encode(&norm_bank),
        urlencoding::encode(&clean_acc),
        amount
    );
    if let Some(m) = memo {
        qr_image_url.push_str("&memo=");
        qr_image_url.push_str(&urlencoding::encode(&sanitize_vietqr_memo(m, 50)));
    }
    if let Some(n) = name {
        qr_image_url.push_str("&name=");
        qr_image_url.push_str(&urlencoding::encode(&sanitize_vietqr_name(n)));
    }

    let launch_url = params.app.as_deref().and_then(|app_id| {
        bank_app_launch_url_for_app(None, app_id, &norm_bank, &clean_acc, amount, memo, name)
    });

    Json(PayApiResponse {
        payload,
        qr_image_url,
        launch_url,
    })
    .into_response()
}

async fn pay_hub_html_handler(Query(params): Query<PayQueryParams>) -> Html<String> {
    let (clean_acc, norm_bank) = if let Some(ref ba) = params.ba {
        parse_ba_param(ba).unwrap_or(("69238686888".to_string(), "TPB".to_string()))
    } else {
        let b = params.bank.as_deref().unwrap_or("TPB");
        let a = params.acc.as_deref().unwrap_or("69238686888");
        (
            sanitize_vietqr_account(a),
            normalize_bank_code_for_qr(b).to_string(),
        )
    };

    let amount = params.am.or(params.amount).unwrap_or(0);
    let memo = params.tn.as_deref().or(params.memo.as_deref()).or(params.des.as_deref());
    let name = params.bn.as_deref().or(params.name.as_deref());

    let html = render_interactive_bank_pay_hub_html(
        &norm_bank,
        &clean_acc,
        amount,
        memo,
        name,
        params.app.as_deref(),
    );
    Html(html)
}

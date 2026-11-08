use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::util::ServiceExt;
use vietqr_service::server::routes::app_router;

#[tokio::test]
async fn test_studio_endpoint_returns_html() {
    let app = app_router();
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/html"));

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8_lossy(&body);
    assert!(body_str.contains("VietQR Studio"));
}

#[tokio::test]
async fn test_banks_endpoint_returns_json() {
    let app = app_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/banks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let banks: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert_eq!(banks.len(), 33);
}

#[tokio::test]
async fn test_qr_png_get_endpoint() {
    let app = app_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/qr.png?bank=TPB&acc=69238686888&amount=50000&memo=TEST&square=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers().get("content-type").unwrap().to_str().unwrap();
    assert_eq!(content_type, "image/png");

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert!(bytes.len() < 8192, "QR PNG should be < 8 KB, got {}", bytes.len());

    let img = image::load_from_memory(&bytes).expect("valid PNG");
    assert_eq!(img.width(), img.height());
}

#[tokio::test]
async fn test_pay_api_endpoint() {
    let app = app_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/pay?bank=TPB&acc=69238686888&amount=50000&memo=DH1001&app=vcb")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["payload"].as_str().unwrap().starts_with("000201"));
    assert!(json["launch_url"].as_str().unwrap().contains("/qr/pay?app=vcb"));
}

#[tokio::test]
async fn test_qr_pay_hub_html_endpoint() {
    let app = app_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/qr/pay?ba=69238686888@tpb&am=50000&tn=TEST%20ORDER")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Thanh to&aacute;n VietQR") || html.contains("Thanh toán VietQR"));
    assert!(html.contains("69238686888"));
    assert!(html.contains("TEST ORDER"));
}

#[tokio::test]
async fn test_qr_png_post_multipart_with_logo() {
    let app = app_router();

    // Build a multipart form boundary and body
    let boundary = "----WebKitFormBoundaryXyZ123";
    let mut body_bytes = Vec::new();

    // field: bank
    body_bytes.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"bank\"\r\n\r\nTPB\r\n").as_bytes());
    // field: acc
    body_bytes.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"acc\"\r\n\r\n69238686888\r\n").as_bytes());
    // field: amount
    body_bytes.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"amount\"\r\n\r\n50000\r\n").as_bytes());
    // field: footer1
    body_bytes.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"footer1\"\r\n\r\nUNG HO TAC GIA\r\n").as_bytes());

    // field: logo (dummy 1x1 PNG)
    let dummy_png = [
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
        8, 2, 0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207,
        192, 0, 0, 3, 1, 1, 0, 24, 221, 141, 176, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    body_bytes.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"logo\"; filename=\"logo.png\"\r\nContent-Type: image/png\r\n\r\n").as_bytes());
    body_bytes.extend_from_slice(&dummy_png);
    body_bytes.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let req = Request::builder()
        .method("POST")
        .uri("/api/qr.png")
        .header("content-type", format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body_bytes))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers().get("content-type").unwrap().to_str().unwrap();
    assert_eq!(content_type, "image/png");

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let img = image::load_from_memory(&bytes).expect("valid PNG with uploaded logo");
    assert!(img.height() > img.width());
}

#[tokio::test]
async fn test_sepay_compatible_img_endpoint() {
    let app = app_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/img?bank=TPB&acc=69238686888&amount=50000&des=DONATION&template=compact")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers().get("content-type").unwrap().to_str().unwrap();
    assert_eq!(content_type, "image/png");

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let img = image::load_from_memory(&bytes).expect("valid PNG from /img");
    assert!(img.width() > 0);
}

#[tokio::test]
async fn test_sepay_template_standee() {
    let app = app_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/img?bank=TPB&acc=69238686888&amount=50000&name=PHAM%20THANH%20NAM&des=UNGHO&template=standee")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let img = image::load_from_memory(&bytes).expect("valid Standee PNG");
    // Standee is tall vertical card (height > width)
    assert!(img.height() > img.width());
    assert!(img.height() as f32 / img.width() as f32 >= 1.3);
}

#[tokio::test]
async fn test_sepay_download_header() {
    let app = app_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/img?bank=TPB&acc=69238686888&download=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let disposition = response
        .headers()
        .get("content-disposition")
        .expect("Content-Disposition should be set")
        .to_str()
        .unwrap();
    assert!(disposition.contains("attachment"));
    assert!(disposition.contains("vietqr-tpb-69238686888.png"));
}

#[tokio::test]
async fn test_sepay_template_qronly_and_showinfo() {
    let app = app_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/qr.png?bank=VCB&acc=0011001234567&template=qronly")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let img = image::load_from_memory(&bytes).expect("valid QROnly PNG");
    assert_eq!(img.width(), img.height()); // QROnly is square 1:1
}

#[tokio::test]
async fn test_sepay_ptn_full_color_template() {
    let app = app_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/img?bank=TPB&acc=69238686888&amount=500000&name=PHAM%20THANH%20NAM&des=UNGHO&template=ptn")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers().get("content-type").unwrap().to_str().unwrap();
    assert_eq!(content_type, "image/png");

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let img = image::load_from_memory(&bytes).expect("valid PTN/SePay PNG");
    assert_eq!(img.width(), 540);
    assert_eq!(img.height(), 687);
}

#[tokio::test]
async fn test_api_endpoints_with_large_amount_and_special_char_memo() {
    let app = app_router();
    // 10 billion VND with special characters, emojis, and 100+ chars in memo
    let uri = "/api/pay?bank=TPB&acc=69238686888&amount=10000000000&app=vcb&memo=DH%2312345%20%40shop%20%28VIP%29%20Ca%20phe%20%E2%98%95%EF%B8%8F%20%26%20Mua%20hang%20sieu%20dai%20vuot%20qua%20gioi%20han%2050%20ky%20tu%20cua%20he%20thong%20vietqr";
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let payload = json["payload"].as_str().unwrap();
    assert!(payload.starts_with("000201010212"));
    assert!(payload.contains("541110000000000")); // 10 billion
    assert!(payload.contains("69238686888"));
    // Memo should be safely sanitized without emojis and truncated to <= 50 chars
    assert!(payload.contains("62540850DH12345 shop VIP Ca phe Mua hang sieu dai vuot qua"));
    assert!(json["launch_url"].as_str().unwrap().contains("am=10000000000"));
}

#[tokio::test]
async fn test_sepay_img_endpoint_with_huge_amount_and_long_memo() {
    let app = app_router();
    // 500 billion VND with long special char description
    let uri = "/img?bank=TPB&acc=69238686888&amount=500000000000&name=PHAM%20THANH%20NAM&des=Mua%20bat%20dong%20san%20so%201%20Ha%20Noi%20500%20ty%21%20%40%23%24%25%5E%26&template=ptn";
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers().get("content-type").unwrap().to_str().unwrap();
    assert_eq!(content_type, "image/png");

    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let img = image::load_from_memory(&bytes).expect("valid high-amount PTN PNG");
    assert_eq!(img.width(), 540);
    assert_eq!(img.height(), 687);
}

#[tokio::test]
async fn test_qr_pay_hub_xss_safety_with_special_chars() {
    let app = app_router();
    // Attempt XSS and malicious characters in memo, amount, and name
    let uri = "/qr/pay?ba=69238686888@tpb&am=10000000000&tn=%3Cscript%3Ealert%28%27XSS%27%29%3C%2Fscript%3E&bn=%3Cb%3ENAM%3C%2Fb%3E";
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8_lossy(&body);

    // Verify raw unescaped script tag does NOT exist in HTML
    assert!(!html.contains("<script>alert('XSS')</script>"));
    // Verify formatted large amount is shown
    assert!(html.contains("10.000.000.000 đ"));
}

#[tokio::test]
async fn test_img_endpoint_with_badge_none() {
    let app = app_router();
    let uri = "/img?bank=TPB&acc=69238686888&amount=50000&badge=none&template=qronly";
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let img = image::load_from_memory(&bytes).expect("valid clean QR PNG without logo");
    assert_eq!(img.width(), img.height());
}

#[tokio::test]
async fn test_img_endpoint_halftone_template() {
    let app = app_router();
    let uri = "/img?bank=TPB&acc=69238686888&amount=500000&template=halftone&dither=atkinson&contrast=1.35";
    let response = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let img = image::load_from_memory(&bytes).expect("valid Halftone QR PNG");
    assert!(img.height() > img.width());
}

#[tokio::test]
async fn test_halftone_preset_import_and_get_render() {
    use vietqr_service::core::halftone::{HalftoneParams, create_and_register_preset};

    let preset_json =
        create_and_register_preset(None, HalftoneParams::default(), false).unwrap();
    let body_json = serde_json::to_vec(&preset_json).unwrap();

    let app = app_router();
    let import_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/halftone/import")
                .header("content-type", "application/json")
                .body(Body::from(body_json))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(import_res.status(), StatusCode::OK);

    let uri = format!(
        "/img?bank=TPB&acc=69238686888&amount=99000&template=halftone&preset={}",
        preset_json.preset_id
    );
    let get_res = app
        .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(get_res.status(), StatusCode::OK);
    let bytes = to_bytes(get_res.into_body(), usize::MAX).await.unwrap();
    let img = image::load_from_memory(&bytes).expect("valid PNG rendered from preset ID");
    assert!(img.height() > img.width());
}

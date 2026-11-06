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

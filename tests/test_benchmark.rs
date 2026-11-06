use std::sync::Arc;
use std::time::Instant;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use tower::util::ServiceExt;
use vietqr_service::core::canvas::*;
use vietqr_service::core::emvco::*;
use vietqr_service::server::routes::app_router;

#[derive(Debug)]
pub struct LatencyStats {
    pub name: &'static str,
    pub count: usize,
    pub total_secs: f64,
    pub rps: f64,
    pub min: f64,
    pub p50: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
    pub p99_9: f64,
    pub max: f64,
    pub unit: &'static str,
}

impl LatencyStats {
    pub fn print_table_row(&self) {
        println!(
            "{:<38} | {:>7} | {:>10.1} | {:>7.2} | {:>7.2} | {:>7.2} | {:>7.2} | {:>7.2} | {:>7.2} | {:>7.2} {}",
            self.name,
            self.count,
            self.rps,
            self.min,
            self.p50,
            self.p90,
            self.p95,
            self.p99,
            self.p99_9,
            self.max,
            self.unit
        );
    }
}

pub fn print_table_header() {
    println!("\n=========================================================================================================================");
    println!("                                      VIETQR SERVICE - PERFORMANCE & P99 BENCHMARK                                      ");
    println!("=========================================================================================================================");
    println!(
        "{:<38} | {:>7} | {:>10} | {:>7} | {:>7} | {:>7} | {:>7} | {:>7} | {:>7} | {:>7} {}",
        "Benchmark Target", "Samples", "Req/sec", "Min", "p50", "p90", "p95", "p99", "p99.9", "Max", "Unit"
    );
    println!("-------------------------------------------------------------------------------------------------------------------------");
}

pub fn print_table_footer() {
    println!("=========================================================================================================================\n");
}

fn compute_stats(name: &'static str, mut samples: Vec<f64>, total_secs: f64, unit: &'static str) -> LatencyStats {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = samples.len();
    assert!(n > 0);
    let min = samples[0];
    let max = samples[n - 1];
    let p50 = samples[(n * 50) / 100];
    let p90 = samples[(n * 90) / 100];
    let p95 = samples[(n * 95) / 100];
    let p99 = samples[(n * 99) / 100];
    let p99_9 = samples[((n * 999) / 1000).min(n - 1)];
    let rps = n as f64 / total_secs;

    LatencyStats {
        name,
        count: n,
        total_secs,
        rps,
        min,
        p50,
        p90,
        p95,
        p99,
        p99_9,
        max,
        unit,
    }
}

#[tokio::test]
#[ignore = "High-iteration performance benchmark; run explicitly with: cargo test --release --test test_benchmark -- --ignored --nocapture"]
async fn run_full_p99_performance_benchmark() {
    print_table_header();

    // 1. Pure EMVCo String + CRC16 Assembly Benchmark (20,000 iterations)
    let emvco_samples_count = 20_000;
    let mut emvco_latencies_us = Vec::with_capacity(emvco_samples_count);
    let start_emvco = Instant::now();

    for i in 0..emvco_samples_count {
        let t0 = Instant::now();
        let payload = build_vietqr_payload(
            "TPB",
            "69238686888",
            500_000 + (i as i64),
            Some("UNG HO PHAM THANH NAM - BENCHMARK TEST P99"),
            Some("PHAM THANH NAM"),
        )
        .expect("payload build ok");
        assert!(!payload.is_empty());
        let elapsed_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
        emvco_latencies_us.push(elapsed_us);
    }
    let total_emvco_secs = start_emvco.elapsed().as_secs_f64();
    let emvco_stats = compute_stats("1. EMVCo Payload + CRC16 (CPU)", emvco_latencies_us, total_emvco_secs, "µs");
    emvco_stats.print_table_row();

    // 2. Standard QR PNG (Indexed Palette 1-byte, Zlib 9) (1,000 iterations)
    let qr_samples_count = 1_000;
    let mut qr_latencies_ms = Vec::with_capacity(qr_samples_count);
    let payload = build_vietqr_payload(
        "TPB",
        "69238686888",
        500_000,
        Some("UNG HO PHAM THANH NAM"),
        Some("PHAM THANH NAM"),
    )
    .unwrap();

    let start_qr = Instant::now();
    for _ in 0..qr_samples_count {
        let t0 = Instant::now();
        let opts = QrRenderOptions {
            payload: &payload,
            logo: CenterLogo::AutoBadge("TPB"),
            template: QrTemplate::QrOnly,
            scale: 8,
            show_info: false,
            full_acc: true,
            bank_code: "TPB",
            account: "69238686888",
            account_name: Some("PHAM THANH NAM"),
            amount: 500_000,
            memo: Some("UNG HO PHAM THANH NAM"),
            footer: QrFooterOptions {
                line1: None,
                line2: None,
                is_square: true,
            },
        };
        let png_bytes = render_vietqr_with_options(opts).expect("render png ok");
        assert!(!png_bytes.is_empty());
        let elapsed_ms = t0.elapsed().as_secs_f64() * 1_000.0;
        qr_latencies_ms.push(elapsed_ms);
    }
    let total_qr_secs = start_qr.elapsed().as_secs_f64();
    let qr_stats = compute_stats("2. QR PNG Square (Indexed Color)", qr_latencies_ms, total_qr_secs, "ms");
    qr_stats.print_table_row();

    // 3. SePay / PTN Full Color Standee 540x687 Card (300 iterations)
    let sepay_samples_count = 300;
    let mut sepay_latencies_ms = Vec::with_capacity(sepay_samples_count);
    let start_sepay = Instant::now();

    for _ in 0..sepay_samples_count {
        let t0 = Instant::now();
        let opts = QrRenderOptions {
            payload: &payload,
            logo: CenterLogo::AutoBadge("TPB"),
            template: QrTemplate::SePay,
            scale: 8,
            show_info: true,
            full_acc: true,
            bank_code: "TPB",
            account: "69238686888",
            account_name: Some("PHAM THANH NAM"),
            amount: 500_000,
            memo: Some("DONATE"),
            footer: QrFooterOptions {
                line1: None,
                line2: None,
                is_square: false,
            },
        };
        let png_bytes = render_vietqr_with_options(opts).expect("render sepay ok");
        assert_eq!(png_bytes[12..16], [0x49, 0x48, 0x44, 0x52]); // IHDR chunk
        let elapsed_ms = t0.elapsed().as_secs_f64() * 1_000.0;
        sepay_latencies_ms.push(elapsed_ms);
    }
    let total_sepay_secs = start_sepay.elapsed().as_secs_f64();
    let sepay_stats = compute_stats("3. SePay/PTN Standee (540x687 Full)", sepay_latencies_ms, total_sepay_secs, "ms");
    sepay_stats.print_table_row();

    // 4. HTTP API JSON Endpoint: GET /api/pay (3,000 concurrent requests)
    let http_pay_count = 3_000;
    let concurrency = 20;
    let requests_per_worker = http_pay_count / concurrency;
    let app = Arc::new(app_router());

    let start_http_pay = Instant::now();
    let mut join_set = tokio::task::JoinSet::new();

    for _ in 0..concurrency {
        let app_clone = Arc::clone(&app);
        join_set.spawn(async move {
            let mut worker_latencies = Vec::with_capacity(requests_per_worker);
            for _ in 0..requests_per_worker {
                let req = Request::builder()
                    .uri("/api/pay?bank=TPB&acc=69238686888&amount=500000&memo=BENCHMARK&app=vcb")
                    .body(Body::empty())
                    .unwrap();

                let t0 = Instant::now();
                let resp = (*app_clone).clone().oneshot(req).await.unwrap();
                assert_eq!(resp.status(), StatusCode::OK);
                let _body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
                let elapsed_ms = t0.elapsed().as_secs_f64() * 1_000.0;
                worker_latencies.push(elapsed_ms);
            }
            worker_latencies
        });
    }

    let mut all_http_pay_latencies = Vec::with_capacity(http_pay_count);
    while let Some(res) = join_set.join_next().await {
        all_http_pay_latencies.extend(res.unwrap());
    }
    let total_http_pay_secs = start_http_pay.elapsed().as_secs_f64();
    let http_pay_stats = compute_stats("4. HTTP /api/pay (JSON Deeplink)", all_http_pay_latencies, total_http_pay_secs, "ms");
    http_pay_stats.print_table_row();

    // 5. HTTP Image Endpoint: GET /api/qr.png?square=true (500 concurrent requests)
    let http_img_count = 500;
    let img_concurrency = 10;
    let img_per_worker = http_img_count / img_concurrency;
    let start_http_img = Instant::now();
    let mut join_set_img = tokio::task::JoinSet::new();

    for _ in 0..img_concurrency {
        let app_clone = Arc::clone(&app);
        join_set_img.spawn(async move {
            let mut worker_latencies = Vec::with_capacity(img_per_worker);
            for _ in 0..img_per_worker {
                let req = Request::builder()
                    .uri("/api/qr.png?bank=TPB&acc=69238686888&amount=500000&square=true")
                    .body(Body::empty())
                    .unwrap();

                let t0 = Instant::now();
                let resp = (*app_clone).clone().oneshot(req).await.unwrap();
                assert_eq!(resp.status(), StatusCode::OK);
                let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
                assert!(!body.is_empty());
                let elapsed_ms = t0.elapsed().as_secs_f64() * 1_000.0;
                worker_latencies.push(elapsed_ms);
            }
            worker_latencies
        });
    }

    let mut all_http_img_latencies = Vec::with_capacity(http_img_count);
    while let Some(res) = join_set_img.join_next().await {
        all_http_img_latencies.extend(res.unwrap());
    }
    let total_http_img_secs = start_http_img.elapsed().as_secs_f64();
    let http_img_stats = compute_stats("5. HTTP /api/qr.png (PNG Image)", all_http_img_latencies, total_http_img_secs, "ms");
    http_img_stats.print_table_row();

    print_table_footer();

    // Only assert strict latency SLA thresholds in optimized release mode (--release)
    if !cfg!(debug_assertions) {
        assert!(emvco_stats.p99 < 100.0, "EMVCo p99 should be < 100 µs, got {:.2} µs", emvco_stats.p99);
        assert!(qr_stats.p99 < 50.0, "QR PNG p99 should be < 50 ms in release mode, got {:.2} ms", qr_stats.p99);
        assert!(sepay_stats.p99 < 60.0, "SePay p99 should be < 60 ms in release mode, got {:.2} ms", sepay_stats.p99);
        assert!(http_pay_stats.p99 < 15.0, "HTTP pay p99 should be < 15 ms in release mode, got {:.2} ms", http_pay_stats.p99);
        assert!(http_img_stats.p99 < 50.0, "HTTP QR PNG p99 should be < 50 ms in release mode, got {:.2} ms", http_img_stats.p99);
    }
}

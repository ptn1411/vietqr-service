use image::{ImageBuffer, Rgba};
use vietqr_service::core::canvas::{CenterLogo, QrFooterOptions, render_custom_vietqr};

fn generate_dummy_logo_png() -> Vec<u8> {
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(64, 64);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let dx = x as f32 - 32.0;
        let dy = y as f32 - 32.0;
        if dx * dx + dy * dy <= 28.0 * 28.0 {
            *pixel = Rgba([220, 38, 38, 255]); // Vibrant Red circle
        } else {
            *pixel = Rgba([0, 0, 0, 0]); // Transparent
        }
    }
    let mut cursor = std::io::Cursor::new(Vec::new());
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .expect("should encode dummy logo");
    cursor.into_inner()
}

#[test]
fn test_render_qr_square_no_footer() {
    let payload = "00020101021238570010A000000727012700069704230111692386868880208QRIBFTTA53037045405500005802VN62250821UNG HO PHAM THANH NAM6304E6BC";
    let png = render_custom_vietqr(
        payload,
        CenterLogo::AutoBadge("TPB"),
        QrFooterOptions {
            line1: None,
            line2: None,
            is_square: true,
        },
        8,
    )
    .expect("render square QR");

    assert!(png.len() < 8192, "Square QR PNG must be under 8 KB, got {} bytes", png.len());

    let img = image::load_from_memory(&png).expect("valid PNG");
    assert_eq!(img.width(), img.height(), "Square QR should have equal width and height");
}

#[test]
fn test_render_qr_with_custom_footer() {
    let payload = "00020101021238570010A000000727012700069704230111692386868880208QRIBFTTA53037045405500005802VN62250821UNG HO PHAM THANH NAM6304E6BC";
    let png = render_custom_vietqr(
        payload,
        CenterLogo::AutoBadge("VCB"),
        QrFooterOptions {
            line1: Some("QUET DE THANH TOAN"),
            line2: Some("STK: 69238686888 - TPBANK"),
            is_square: false,
        },
        8,
    )
    .expect("render branded QR with footer");

    assert!(png.len() < 8192, "Branded QR PNG must be under 8 KB, got {} bytes", png.len());

    let img = image::load_from_memory(&png).expect("valid PNG");
    assert!(img.height() > img.width(), "Footer should increase total height");
}

#[test]
fn test_render_qr_with_custom_image_logo() {
    let payload = "00020101021238570010A000000727012700069704230111692386868880208QRIBFTTA53037045405500005802VN62250821UNG HO PHAM THANH NAM6304E6BC";
    let logo_bytes = generate_dummy_logo_png();

    let png = render_custom_vietqr(
        payload,
        CenterLogo::ImageLogo(&logo_bytes),
        QrFooterOptions {
            line1: Some("LOGO TÙY BIẾN"),
            line2: Some("PHAM THANH NAM"),
            is_square: false,
        },
        8,
    )
    .expect("render QR with image logo");

    assert!(png.len() < 12288, "Custom logo QR PNG must be under 12 KB, got {} bytes", png.len());

    let img = image::load_from_memory(&png).expect("valid PNG");
    assert!(img.height() > img.width());
}

#[test]
fn test_render_qr_with_no_center_logo() {
    let payload = "00020101021238570010A000000727012700069704230111692386868880208QRIBFTTA53037045405500005802VN62250821UNG HO PHAM THANH NAM6304E6BC";
    let png = render_custom_vietqr(
        payload,
        CenterLogo::None,
        QrFooterOptions {
            line1: None,
            line2: None,
            is_square: true,
        },
        8,
    )
    .expect("render QR with CenterLogo::None");

    assert!(png.len() < 8192, "Clean QR without logo must be super lightweight, got {} bytes", png.len());
    let img = image::load_from_memory(&png).expect("valid PNG");
    assert_eq!(img.width(), img.height());
}

#[test]
fn test_render_halftone_vietqr_with_dither_methods() {
    use vietqr_service::core::canvas::{QrRenderOptions, QrTemplate};
    use vietqr_service::core::halftone::{
        DitherMethod, HalftoneColorMode, HalftoneParams, render_halftone_vietqr,
    };

    let payload = "00020101021238570010A000000727012700069704230111692386868880208QRIBFTTA53037045405500005802VN62250821UNG HO PHAM THANH NAM6304E6BC";
    let custom_bg = generate_dummy_logo_png();

    for (dither, color_mode) in [
        (DitherMethod::FloydSteinberg, HalftoneColorMode::Custom),
        (DitherMethod::Atkinson, HalftoneColorMode::BankBrand),
        (DitherMethod::Bayer4x4, HalftoneColorMode::PhotoRgb),
        (DitherMethod::Threshold, HalftoneColorMode::Custom),
    ] {
        let opts = QrRenderOptions {
            payload,
            logo: CenterLogo::ImageLogo(&custom_bg),
            template: QrTemplate::Halftone,
            scale: 8,
            show_info: false,
            full_acc: true,
            bank_code: "TPB",
            account: "69238686888",
            account_name: Some("PHAM THANH NAM"),
            amount: 50000,
            memo: Some("UNG HO PHAM THANH NAM"),
            footer: QrFooterOptions::default(),
        };
        let png = render_halftone_vietqr(
            &opts,
            HalftoneParams {
                dither,
                contrast: 1.3,
                brightness: 0.05,
                color_mode,
                fg_color: (76, 29, 149),
                bg_color: (250, 245, 255),
            },
        )
        .expect("render halftone QR");
        let img = image::load_from_memory(&png).expect("valid Halftone PNG");
        assert_eq!(img.width(), img.height());
    }
}

#[test]
fn test_halftone_precomputed_json_preset_roundtrip_and_fast_render() {
    use vietqr_service::core::canvas::{QrRenderOptions, QrTemplate};
    use vietqr_service::core::halftone::{
        HalftoneParams, compile_and_register_preset_json, create_and_register_preset,
        render_halftone_with_preset,
    };

    let payload = "00020101021238570010A000000727012700069704230111692386868880208QRIBFTTA53037045405500005802VN62250821UNG HO PHAM THANH NAM6304E6BC";
    let custom_bg = generate_dummy_logo_png();

    let preset_json =
        create_and_register_preset(Some(&custom_bg), HalftoneParams::default(), true)
            .expect("should export halftone preset JSON");
    assert!(preset_json.preset_id.starts_with("ht_"));
    assert_eq!(preset_json.grid_w, 135);

    let serialized = serde_json::to_string(&preset_json).expect("serialize preset json");
    let parsed: vietqr_service::core::halftone::HalftonePresetJson =
        serde_json::from_str(&serialized).expect("deserialize preset json");
    let compiled =
        compile_and_register_preset_json(&parsed).expect("compile and register preset");

    let opts = QrRenderOptions {
        payload,
        logo: CenterLogo::None,
        template: QrTemplate::Halftone,
        scale: 8,
        show_info: true,
        full_acc: true,
        bank_code: "TPB",
        account: "69238686888",
        account_name: Some("PHAM THANH NAM"),
        amount: 250000,
        memo: Some("FAST PRESET RENDER"),
        footer: QrFooterOptions::default(),
    };

    let png = render_halftone_with_preset(&opts, &compiled, None)
        .expect("fast render from precomputed preset");
    let img = image::load_from_memory(&png).expect("valid PNG from preset");
    assert!(img.height() > img.width());
}

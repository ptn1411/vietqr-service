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

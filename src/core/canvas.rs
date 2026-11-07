use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use anyhow::{Context, Result, bail};
use image::imageops::FilterType;
use image::{GenericImageView, Pixel};
use qrcode::{Color, EcLevel, QrCode};

use crate::core::emvco::{
    bank_display_name, detect_qr_center_label, format_bank_account_spaced, format_vnd_amount,
    normalize_bank_code_for_qr, qr_center_badge_brand_color, sanitize_vietqr_memo,
    sanitize_vietqr_name, strip_vietnamese_accents,
};
use crate::core::png_encoder::{encode_branded_qr_png, encode_rgb_png};

pub const BRANDING_FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/Roboto-Bold.ttf");
pub const PTN_LOGO_BYTES: &[u8] = include_bytes!("../../assets/brand/ptn_logo.png");
pub const PARTNER_BAR_BYTES: &[u8] = include_bytes!("../../assets/brand/partner_bar_tpb.png");
pub const VIETQR_CENTER_BYTES: &[u8] = include_bytes!("../../assets/brand/vietqr_center.png");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QrTemplate {
    #[default]
    Default,
    Compact,
    QrOnly,
    Standee,
    SePay,
}

impl QrTemplate {
    pub fn from_str_opt(s: Option<&str>) -> Self {
        match s.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("compact") | Some("compact2") => QrTemplate::Compact,
            Some("qronly") | Some("qr_only") | Some("raw") => QrTemplate::QrOnly,
            Some("standee") | Some("card") | Some("print") => QrTemplate::Standee,
            Some("sepay") | Some("ptn") | Some("brand") | Some("color") => QrTemplate::SePay,
            _ => QrTemplate::Default,
        }
    }
}

#[derive(Debug, Clone)]
pub enum CenterLogo<'a> {
    AutoBadge(&'a str),
    TextBadge(&'a str),
    ImageLogo(&'a [u8]),
    None,
}

#[derive(Debug, Clone, Default)]
pub struct QrFooterOptions<'a> {
    pub line1: Option<&'a str>,
    pub line2: Option<&'a str>,
    pub is_square: bool,
}

#[derive(Debug, Clone)]
pub struct QrRenderOptions<'a> {
    pub payload: &'a str,
    pub logo: CenterLogo<'a>,
    pub template: QrTemplate,
    pub scale: usize,
    pub show_info: bool,
    pub full_acc: bool,
    pub bank_code: &'a str,
    pub account: &'a str,
    pub account_name: Option<&'a str>,
    pub amount: i64,
    pub memo: Option<&'a str>,
    pub footer: QrFooterOptions<'a>,
}

#[inline]
pub fn point_in_rounded_rect(
    px: i32,
    py: i32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    radius: i32,
) -> bool {
    if px < left || px >= right || py < top || py >= bottom {
        return false;
    }
    if radius <= 0 {
        return true;
    }
    let dx = if px < left + radius {
        (left + radius - 1) - px
    } else if px >= right - radius {
        px - (right - radius)
    } else {
        0
    };
    let dy = if py < top + radius {
        (top + radius - 1) - py
    } else if py >= bottom - radius {
        py - (bottom - radius)
    } else {
        0
    };
    dx * dx + dy * dy <= radius * radius
}

/// Render a 7x7 Finder Pattern with rounded corners on a 1-byte palette buffer.
pub fn render_rounded_finder_pattern(
    pixels: &mut [u8],
    canvas_width: usize,
    origin_x: usize,
    origin_y: usize,
    scale: usize,
) {
    let finder_px = (7 * scale) as i32;
    let ox = origin_x as i32;
    let oy = origin_y as i32;
    let s = scale as i32;

    let r_outer = ((scale * 3) / 4).max(2) as i32;
    let r_mid = (scale / 2).max(1) as i32;
    let r_core = (scale / 2).max(1) as i32;

    for ly in 0..finder_px {
        for lx in 0..finder_px {
            let in_outer = point_in_rounded_rect(lx, ly, 0, 0, finder_px, finder_px, r_outer);
            let in_white_ring =
                point_in_rounded_rect(lx, ly, s, s, finder_px - s, finder_px - s, r_mid);
            let in_core = point_in_rounded_rect(
                lx,
                ly,
                2 * s,
                2 * s,
                finder_px - 2 * s,
                finder_px - 2 * s,
                r_core,
            );

            let is_dark = in_core || (in_outer && !in_white_ring);
            let px = (ox + lx) as usize;
            let py = (oy + ly) as usize;
            pixels[py * canvas_width + px] = if is_dark { 0x00 } else { 0xFF };
        }
    }
}

/// Render 4 corner viewfinder brackets on a 1-byte palette buffer.
pub fn render_corner_brackets(pixels: &mut [u8], qr_width: usize, scale: usize) {
    let margin = scale;
    let thickness = (scale / 2).max(3);
    let arm_len = scale * 5;
    if margin + arm_len >= qr_width / 2 {
        return;
    }

    for dy in 0..arm_len {
        for dx in 0..arm_len {
            if dx < thickness || dy < thickness {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let tl_x = margin + dx;
                let tl_y = margin + dy;
                let tr_x = qr_width - 1 - tl_x;
                let bl_y = qr_width - 1 - tl_y;

                pixels[tl_y * qr_width + tl_x] = 0x00;
                pixels[tl_y * qr_width + tr_x] = 0x00;
                pixels[bl_y * qr_width + tl_x] = 0x00;
                pixels[bl_y * qr_width + tr_x] = 0x00;
            }
        }
    }
}

/// Render anti-aliased text line on a 1-byte palette buffer.
#[allow(clippy::too_many_arguments)]
pub fn render_text_line(
    font: &FontRef,
    pixels: &mut [u8],
    width: usize,
    height: usize,
    center_y: f32,
    scale: PxScale,
    text: &str,
    use_brand_palette: bool,
) {
    if text.is_empty() {
        return;
    }
    let scaled_font = font.as_scaled(scale);
    let mut glyphs = Vec::with_capacity(text.len());
    let mut caret = point(0.0, scaled_font.ascent());

    for ch in text.chars() {
        if ch.is_control() {
            continue;
        }
        let glyph_id = font.glyph_id(ch);
        let glyph = glyph_id.with_scale_and_position(scale, caret);
        caret.x += scaled_font.h_advance(glyph_id);
        glyphs.push(glyph);
    }

    let text_width = caret.x;
    let offset_x = ((width as f32 - text_width) / 2.0).max(0.0);
    let text_height = scaled_font.ascent() - scaled_font.descent();
    let offset_y = center_y - (text_height / 2.0);

    for mut glyph in glyphs {
        glyph.position.x += offset_x;
        glyph.position.y += offset_y;
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let base_x = bounds.min.x.round() as i32;
            let base_y = bounds.min.y.round() as i32;
            outlined.draw(|x, y, coverage| {
                let cov = coverage.clamp(0.0, 1.0);
                if cov <= 0.08 {
                    return;
                }
                let px = base_x + x as i32;
                let py = base_y + y as i32;
                if px >= 0 && (px as usize) < width && py >= 0 && (py as usize) < height {
                    let idx = (py as usize) * width + (px as usize);
                    if use_brand_palette {
                        let step = ((1.0 - cov) * 7.0).round() as u8;
                        let brand_val = (1 + step).clamp(0x01, 0x08);
                        pixels[idx] = pixels[idx].min(brand_val);
                    } else {
                        let gray = (255.0 * (1.0 - cov)).round().max(9.0) as u8;
                        pixels[idx] = pixels[idx].min(gray);
                    }
                }
            });
        }
    }
}

/// Render center logo badge on a 1-byte palette buffer.
pub fn render_center_logo_badge(
    pixels: &mut [u8],
    qr_width: usize,
    total_height: usize,
    qr_matrix_px: usize,
    label: &str,
) {
    let clean_label = label.trim();
    if clean_label.is_empty() {
        return;
    }

    let center = (qr_width / 2) as i32;
    let half_h = ((qr_matrix_px as f32 * 0.082) as i32).clamp(18, 30);
    let half_w = ((qr_matrix_px as f32 * 0.115) as i32).clamp(26, 44);

    let pad = 3i32;
    let stroke = 2i32;
    let r_outer = 8i32;
    let r_border = 6i32;
    let r_inner = 4i32;

    let left = center - half_w;
    let right = center + half_w;
    let top = center - half_h;
    let bottom = center + half_h;

    for py in top..bottom {
        for px in left..right {
            if px < 0 || py < 0 || (px as usize) >= qr_width || (py as usize) >= qr_width {
                continue;
            }
            if point_in_rounded_rect(px, py, left, top, right, bottom, r_outer) {
                let in_border_outer = point_in_rounded_rect(
                    px,
                    py,
                    left + pad,
                    top + pad,
                    right - pad,
                    bottom - pad,
                    r_border,
                );
                let in_border_inner = point_in_rounded_rect(
                    px,
                    py,
                    left + pad + stroke,
                    top + pad + stroke,
                    right - pad - stroke,
                    bottom - pad - stroke,
                    r_inner,
                );
                let val = if in_border_outer && !in_border_inner {
                    0x01 // Brand color border
                } else {
                    0xFF // White cushion
                };
                pixels[(py as usize) * qr_width + (px as usize)] = val;
            }
        }
    }

    if let Ok(font) = FontRef::try_from_slice(BRANDING_FONT_BYTES) {
        let char_count = clean_label.chars().count().max(1) as f32;
        let max_inner_w = ((half_w - pad - stroke - 3) * 2) as f32;
        let font_size = (max_inner_w / (char_count * 0.62)).clamp(11.5, 18.0);
        render_text_line(
            &font,
            pixels,
            qr_width,
            total_height,
            center as f32,
            PxScale::from(font_size),
            clean_label,
            true,
        );
    }
}

// ---------------------------------------------------------------------------
// RGB Buffer Helpers (for Custom Image Logo)
// ---------------------------------------------------------------------------

fn render_rgb_rounded_finder_pattern(
    rgb_pixels: &mut [u8],
    canvas_width: usize,
    origin_x: usize,
    origin_y: usize,
    scale: usize,
) {
    let finder_px = (7 * scale) as i32;
    let ox = origin_x as i32;
    let oy = origin_y as i32;
    let s = scale as i32;

    let r_outer = ((scale * 3) / 4).max(2) as i32;
    let r_mid = (scale / 2).max(1) as i32;
    let r_core = (scale / 2).max(1) as i32;

    for ly in 0..finder_px {
        for lx in 0..finder_px {
            let in_outer = point_in_rounded_rect(lx, ly, 0, 0, finder_px, finder_px, r_outer);
            let in_white_ring =
                point_in_rounded_rect(lx, ly, s, s, finder_px - s, finder_px - s, r_mid);
            let in_core = point_in_rounded_rect(
                lx,
                ly,
                2 * s,
                2 * s,
                finder_px - 2 * s,
                finder_px - 2 * s,
                r_core,
            );

            let is_dark = in_core || (in_outer && !in_white_ring);
            let px = (ox + lx) as usize;
            let py = (oy + ly) as usize;
            let idx = (py * canvas_width + px) * 3;
            let color = if is_dark { 0u8 } else { 255u8 };
            rgb_pixels[idx] = color;
            rgb_pixels[idx + 1] = color;
            rgb_pixels[idx + 2] = color;
        }
    }
}

fn render_rgb_corner_brackets(rgb_pixels: &mut [u8], qr_width: usize, scale: usize) {
    let margin = scale;
    let thickness = (scale / 2).max(3);
    let arm_len = scale * 5;
    if margin + arm_len >= qr_width / 2 {
        return;
    }

    for dy in 0..arm_len {
        for dx in 0..arm_len {
            if dx < thickness || dy < thickness {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let tl_x = margin + dx;
                let tl_y = margin + dy;
                let tr_x = qr_width - 1 - tl_x;
                let bl_y = qr_width - 1 - tl_y;

                for &(px, py) in &[(tl_x, tl_y), (tr_x, tl_y), (tl_x, bl_y), (tr_x, bl_y)] {
                    let idx = (py * qr_width + px) * 3;
                    rgb_pixels[idx] = 0;
                    rgb_pixels[idx + 1] = 0;
                    rgb_pixels[idx + 2] = 0;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_rgb_text_line(
    font: &FontRef,
    rgb_pixels: &mut [u8],
    width: usize,
    height: usize,
    center_y: f32,
    scale: PxScale,
    text: &str,
    color_rgb: (u8, u8, u8),
) {
    if text.is_empty() {
        return;
    }
    let scaled_font = font.as_scaled(scale);
    let mut glyphs = Vec::with_capacity(text.len());
    let mut caret = point(0.0, scaled_font.ascent());

    for ch in text.chars() {
        if ch.is_control() {
            continue;
        }
        let glyph_id = font.glyph_id(ch);
        let glyph = glyph_id.with_scale_and_position(scale, caret);
        caret.x += scaled_font.h_advance(glyph_id);
        glyphs.push(glyph);
    }

    let text_width = caret.x;
    let offset_x = ((width as f32 - text_width) / 2.0).max(0.0);
    let text_height = scaled_font.ascent() - scaled_font.descent();
    let offset_y = center_y - (text_height / 2.0);

    let (tr, tg, tb) = color_rgb;

    for mut glyph in glyphs {
        glyph.position.x += offset_x;
        glyph.position.y += offset_y;
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let base_x = bounds.min.x.round() as i32;
            let base_y = bounds.min.y.round() as i32;
            outlined.draw(|x, y, coverage| {
                let cov = coverage.clamp(0.0, 1.0);
                if cov <= 0.08 {
                    return;
                }
                let px = base_x + x as i32;
                let py = base_y + y as i32;
                if px >= 0 && (px as usize) < width && py >= 0 && (py as usize) < height {
                    let idx = ((py as usize) * width + (px as usize)) * 3;
                    let curr_r = rgb_pixels[idx] as f32;
                    let curr_g = rgb_pixels[idx + 1] as f32;
                    let curr_b = rgb_pixels[idx + 2] as f32;

                    rgb_pixels[idx] = (curr_r * (1.0 - cov) + tr as f32 * cov).round() as u8;
                    rgb_pixels[idx + 1] = (curr_g * (1.0 - cov) + tg as f32 * cov).round() as u8;
                    rgb_pixels[idx + 2] = (curr_b * (1.0 - cov) + tb as f32 * cov).round() as u8;
                }
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Main Render Function
// ---------------------------------------------------------------------------

pub fn render_custom_vietqr(
    payload: &str,
    logo: CenterLogo,
    footer: QrFooterOptions,
    scale: usize,
) -> Result<Vec<u8>> {
    let clean_payload = payload.trim();
    if clean_payload.is_empty() {
        bail!("QR payload cannot be empty");
    }

    let scale = scale.clamp(4, 16);
    let border = 4usize;
    let qr = QrCode::with_error_correction_level(clean_payload.as_bytes(), EcLevel::H)
        .context("Failed to generate QR code with Level H ECC")?;
    let size = qr.width();
    let qr_width = (size + border * 2) * scale;

    let has_footer = !footer.is_square
        && (footer.line1.map(|s| !s.trim().is_empty()).unwrap_or(false)
            || footer.line2.map(|s| !s.trim().is_empty()).unwrap_or(false));

    let (footer_height, total_height) = if has_footer {
        let font_size = ((qr_width as f32) * 0.036).clamp(13.0, 18.0);
        let fh = (font_size * 4.2) as usize;
        (fh, qr_width + fh)
    } else {
        (0, qr_width)
    };

    match logo {
        CenterLogo::ImageLogo(logo_bytes) => {
            // Render to RGB Truecolor buffer
            let mut rgb_pixels = vec![255u8; qr_width * total_height * 3];

            // Center area box to skip modules under logo cushion
            let center = (qr_width / 2) as i32;
            let cushion_half_w = ((size * scale) as f32 * 0.115) as i32;
            let cushion_half_h = ((size * scale) as f32 * 0.115) as i32;
            let c_left = center - cushion_half_w - 2;
            let c_right = center + cushion_half_w + 2;
            let c_top = center - cushion_half_h - 2;
            let c_bottom = center + cushion_half_h + 2;

            // Draw data modules
            for y in 0..size {
                for x in 0..size {
                    let in_finder =
                        (x + 7 >= size || x < 7) && y < 7 || (x < 7 && y + 7 >= size);
                    if in_finder {
                        continue;
                    }
                    if qr[(x, y)] == Color::Dark {
                        let px = (x + border) * scale;
                        let py = (y + border) * scale;
                        for dy in 0..scale {
                            for dx in 0..scale {
                                let cur_x = (px + dx) as i32;
                                let cur_y = (py + dy) as i32;
                                if cur_x >= c_left
                                    && cur_x <= c_right
                                    && cur_y >= c_top
                                    && cur_y <= c_bottom
                                {
                                    continue;
                                }
                                let idx = ((cur_y as usize) * qr_width + (cur_x as usize)) * 3;
                                rgb_pixels[idx] = 0;
                                rgb_pixels[idx + 1] = 0;
                                rgb_pixels[idx + 2] = 0;
                            }
                        }
                    }
                }
            }

            // Draw Finder Patterns & Corner Brackets
            let b_px = border * scale;
            render_rgb_rounded_finder_pattern(&mut rgb_pixels, qr_width, b_px, b_px, scale);
            render_rgb_rounded_finder_pattern(
                &mut rgb_pixels,
                qr_width,
                (border + size - 7) * scale,
                b_px,
                scale,
            );
            render_rgb_rounded_finder_pattern(
                &mut rgb_pixels,
                qr_width,
                b_px,
                (border + size - 7) * scale,
                scale,
            );
            render_rgb_corner_brackets(&mut rgb_pixels, qr_width, scale);

            // Draw White Cushion with subtle rounded border
            let radius = 8i32;
            for py in (center - cushion_half_h)..(center + cushion_half_h) {
                for px in (center - cushion_half_w)..(center + cushion_half_w) {
                    if point_in_rounded_rect(
                        px,
                        py,
                        center - cushion_half_w,
                        center - cushion_half_h,
                        center + cushion_half_w,
                        center + cushion_half_h,
                        radius,
                    ) {
                        let is_border = !point_in_rounded_rect(
                            px,
                            py,
                            center - cushion_half_w + 1,
                            center - cushion_half_h + 1,
                            center + cushion_half_w - 1,
                            center + cushion_half_h - 1,
                            radius - 1,
                        );
                        let idx = ((py as usize) * qr_width + (px as usize)) * 3;
                        if is_border {
                            rgb_pixels[idx] = 226;
                            rgb_pixels[idx + 1] = 232;
                            rgb_pixels[idx + 2] = 240;
                        } else {
                            rgb_pixels[idx] = 255;
                            rgb_pixels[idx + 1] = 255;
                            rgb_pixels[idx + 2] = 255;
                        }
                    }
                }
            }

            // Composite Uploaded Image Logo inside cushion
            if let Ok(dyn_img) = image::load_from_memory(logo_bytes) {
                let max_logo_w = ((cushion_half_w - 4) * 2) as u32;
                let max_logo_h = ((cushion_half_h - 4) * 2) as u32;
                let (orig_w, orig_h) = dyn_img.dimensions();
                if orig_w > 0 && orig_h > 0 {
                    let ratio_w = max_logo_w as f32 / orig_w as f32;
                    let ratio_h = max_logo_h as f32 / orig_h as f32;
                    let ratio = ratio_w.min(ratio_h);
                    let target_w = ((orig_w as f32 * ratio).round() as u32).max(1);
                    let target_h = ((orig_h as f32 * ratio).round() as u32).max(1);

                    let resized = dyn_img.resize_exact(target_w, target_h, FilterType::Lanczos3);
                    let start_x = center - (target_w as i32 / 2);
                    let start_y = center - (target_h as i32 / 2);

                    for (lx, ly, pixel) in resized.pixels() {
                        let px = start_x + lx as i32;
                        let py = start_y + ly as i32;
                        if px >= 0 && (px as usize) < qr_width && py >= 0 && (py as usize) < qr_width
                        {
                            let channels = pixel.channels();
                            let (r, g, b) = (channels[0], channels[1], channels[2]);
                            let alpha = if channels.len() > 3 {
                                channels[3] as f32 / 255.0
                            } else {
                                1.0
                            };
                            if alpha > 0.05 {
                                let idx = ((py as usize) * qr_width + (px as usize)) * 3;
                                let bg_r = rgb_pixels[idx] as f32;
                                let bg_g = rgb_pixels[idx + 1] as f32;
                                let bg_b = rgb_pixels[idx + 2] as f32;

                                rgb_pixels[idx] =
                                    (bg_r * (1.0 - alpha) + r as f32 * alpha).round() as u8;
                                rgb_pixels[idx + 1] =
                                    (bg_g * (1.0 - alpha) + g as f32 * alpha).round() as u8;
                                rgb_pixels[idx + 2] =
                                    (bg_b * (1.0 - alpha) + b as f32 * alpha).round() as u8;
                            }
                        }
                    }
                }
            }

            // Draw Footer Text if requested
            if has_footer {
                if let Ok(font) = FontRef::try_from_slice(BRANDING_FONT_BYTES) {
                    let footer_top = qr_width as f32;
                    let font_size = ((qr_width as f32) * 0.036).clamp(13.0, 18.0);
                    let scale = PxScale::from(font_size);
                    let fh = footer_height as f32;

                    if let Some(line1) = footer.line1 {
                        if !line1.trim().is_empty() {
                            let y1 = footer_top + fh * 0.32;
                            render_rgb_text_line(
                                &font,
                                &mut rgb_pixels,
                                qr_width,
                                total_height,
                                y1,
                                scale,
                                line1.trim(),
                                (30, 41, 59), // Slate 800
                            );
                        }
                    }
                    if let Some(line2) = footer.line2 {
                        if !line2.trim().is_empty() {
                            let y2 = footer_top + fh * 0.72;
                            render_rgb_text_line(
                                &font,
                                &mut rgb_pixels,
                                qr_width,
                                total_height,
                                y2,
                                scale,
                                line2.trim(),
                                (71, 85, 105), // Slate 600
                            );
                        }
                    }
                }
            }

            Ok(encode_rgb_png(&rgb_pixels, qr_width as u32, total_height as u32))
        }
        badge_mode => {
            // Render to 1-byte Indexed-Color Palette buffer
            let mut pixels = vec![0xFFu8; qr_width * total_height];

            // Draw data modules
            for y in 0..size {
                for x in 0..size {
                    let in_finder =
                        (x + 7 >= size || x < 7) && y < 7 || (x < 7 && y + 7 >= size);
                    if in_finder {
                        continue;
                    }
                    if qr[(x, y)] == Color::Dark {
                        let px = (x + border) * scale;
                        let py = (y + border) * scale;
                        for dy in 0..scale {
                            for dx in 0..scale {
                                pixels[(py + dy) * qr_width + (px + dx)] = 0x00;
                            }
                        }
                    }
                }
            }

            // Draw Finder Patterns & Corner Brackets
            let b_px = border * scale;
            render_rounded_finder_pattern(&mut pixels, qr_width, b_px, b_px, scale);
            render_rounded_finder_pattern(
                &mut pixels,
                qr_width,
                (border + size - 7) * scale,
                b_px,
                scale,
            );
            render_rounded_finder_pattern(
                &mut pixels,
                qr_width,
                b_px,
                (border + size - 7) * scale,
                scale,
            );
            render_corner_brackets(&mut pixels, qr_width, scale);

            // Determine badge brand color & label
            let (badge_label, brand_rgb) = match badge_mode {
                CenterLogo::AutoBadge(bank_or_bin) => {
                    let lbl = if bank_or_bin.trim().is_empty() {
                        detect_qr_center_label(clean_payload)
                    } else {
                        normalize_bank_code_for_qr(bank_or_bin)
                    };
                    (Some(lbl), qr_center_badge_brand_color(lbl))
                }
                CenterLogo::TextBadge(txt) => {
                    let lbl = txt.trim();
                    (Some(lbl), (24, 73, 214)) // Royal Blue
                }
                CenterLogo::None => (None, (5, 122, 85)),
                CenterLogo::ImageLogo(_) => unreachable!(),
            };

            if let Some(lbl) = badge_label {
                render_center_logo_badge(&mut pixels, qr_width, total_height, size * scale, lbl);
            }

            // Draw Footer Text if requested
            if has_footer {
                if let Ok(font) = FontRef::try_from_slice(BRANDING_FONT_BYTES) {
                    let footer_top = qr_width as f32;
                    let font_size = ((qr_width as f32) * 0.036).clamp(13.0, 18.0);
                    let scale = PxScale::from(font_size);
                    let fh = footer_height as f32;

                    if let Some(line1) = footer.line1 {
                        if !line1.trim().is_empty() {
                            let y1 = footer_top + fh * 0.32;
                            render_text_line(
                                &font,
                                &mut pixels,
                                qr_width,
                                total_height,
                                y1,
                                scale,
                                line1.trim(),
                                false,
                            );
                        }
                    }
                    if let Some(line2) = footer.line2 {
                        if !line2.trim().is_empty() {
                            let y2 = footer_top + fh * 0.72;
                            render_text_line(
                                &font,
                                &mut pixels,
                                qr_width,
                                total_height,
                                y2,
                                scale,
                                line2.trim(),
                                false,
                            );
                        }
                    }
                }
            }

            Ok(encode_branded_qr_png(
                &pixels,
                qr_width as u32,
                total_height as u32,
                brand_rgb,
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Standee & Multi-template Rendering Helpers
// ---------------------------------------------------------------------------

fn fill_rgb_rect(
    rgb_pixels: &mut [u8],
    canvas_w: usize,
    canvas_h: usize,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    color: (u8, u8, u8),
) {
    let (r, g, b) = color;
    let min_x = left.max(0) as usize;
    let max_x = right.min(canvas_w as i32).max(0) as usize;
    let min_y = top.max(0) as usize;
    let max_y = bottom.min(canvas_h as i32).max(0) as usize;

    for y in min_y..max_y {
        for x in min_x..max_x {
            let idx = (y * canvas_w + x) * 3;
            rgb_pixels[idx] = r;
            rgb_pixels[idx + 1] = g;
            rgb_pixels[idx + 2] = b;
        }
    }
}

fn fill_rgb_rounded_rect(
    rgb_pixels: &mut [u8],
    canvas_w: usize,
    canvas_h: usize,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    radius: i32,
    color: (u8, u8, u8),
) {
    let (r, g, b) = color;
    let min_x = left.max(0) as usize;
    let max_x = right.min(canvas_w as i32).max(0) as usize;
    let min_y = top.max(0) as usize;
    let max_y = bottom.min(canvas_h as i32).max(0) as usize;

    for y in min_y..max_y {
        for x in min_x..max_x {
            if point_in_rounded_rect(x as i32, y as i32, left, top, right, bottom, radius) {
                let idx = (y * canvas_w + x) * 3;
                rgb_pixels[idx] = r;
                rgb_pixels[idx + 1] = g;
                rgb_pixels[idx + 2] = b;
            }
        }
    }
}

fn draw_rgb_rounded_rect_border(
    rgb_pixels: &mut [u8],
    canvas_w: usize,
    canvas_h: usize,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    radius: i32,
    thickness: i32,
    color: (u8, u8, u8),
) {
    let (r, g, b) = color;
    let min_x = left.max(0) as usize;
    let max_x = right.min(canvas_w as i32).max(0) as usize;
    let min_y = top.max(0) as usize;
    let max_y = bottom.min(canvas_h as i32).max(0) as usize;

    for y in min_y..max_y {
        for x in min_x..max_x {
            let in_outer =
                point_in_rounded_rect(x as i32, y as i32, left, top, right, bottom, radius);
            let in_inner = point_in_rounded_rect(
                x as i32,
                y as i32,
                left + thickness,
                top + thickness,
                right - thickness,
                bottom - thickness,
                (radius - thickness).max(0),
            );
            if in_outer && !in_inner {
                let idx = (y * canvas_w + x) * 3;
                rgb_pixels[idx] = r;
                rgb_pixels[idx + 1] = g;
                rgb_pixels[idx + 2] = b;
            }
        }
    }
}

fn render_rgb_corner_brackets_offset(
    rgb_pixels: &mut [u8],
    canvas_w: usize,
    ox: usize,
    oy: usize,
    qr_width: usize,
    scale: usize,
) {
    let margin = scale;
    let thickness = (scale / 2).max(3);
    let arm_len = scale * 5;
    if margin + arm_len >= qr_width / 2 {
        return;
    }

    for dy in 0..arm_len {
        for dx in 0..arm_len {
            if dx < thickness || dy < thickness {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let tl_x = ox + margin + dx;
                let tl_y = oy + margin + dy;
                let tr_x = ox + qr_width - 1 - margin - dx;
                let bl_y = oy + qr_width - 1 - margin - dy;

                for &(px, py) in &[(tl_x, tl_y), (tr_x, tl_y), (tl_x, bl_y), (tr_x, bl_y)] {
                    let idx = (py * canvas_w + px) * 3;
                    rgb_pixels[idx] = 0;
                    rgb_pixels[idx + 1] = 0;
                    rgb_pixels[idx + 2] = 0;
                }
            }
        }
    }
}

fn draw_rgb_center_logo_badge(
    rgb_pixels: &mut [u8],
    canvas_w: usize,
    canvas_h: usize,
    center_x: i32,
    center_y: i32,
    qr_matrix_px: usize,
    label: &str,
    brand_rgb: (u8, u8, u8),
) {
    let clean_label = label.trim();
    if clean_label.is_empty() {
        return;
    }

    let half_h = ((qr_matrix_px as f32 * 0.082) as i32).clamp(18, 30);
    let half_w = ((qr_matrix_px as f32 * 0.115) as i32).clamp(26, 44);

    let pad = 3i32;
    let stroke = 2i32;
    let r_outer = 8i32;
    let r_border = 6i32;
    let r_inner = 4i32;

    let left = center_x - half_w;
    let right = center_x + half_w;
    let top = center_y - half_h;
    let bottom = center_y + half_h;

    let (br, bg, bb) = brand_rgb;

    for py in top..bottom {
        for px in left..right {
            if px < 0 || py < 0 || (px as usize) >= canvas_w || (py as usize) >= canvas_h {
                continue;
            }
            if point_in_rounded_rect(px, py, left, top, right, bottom, r_outer) {
                let in_border_outer = point_in_rounded_rect(
                    px,
                    py,
                    left + pad,
                    top + pad,
                    right - pad,
                    bottom - pad,
                    r_border,
                );
                let in_border_inner = point_in_rounded_rect(
                    px,
                    py,
                    left + pad + stroke,
                    top + pad + stroke,
                    right - pad - stroke,
                    bottom - pad - stroke,
                    r_inner,
                );
                let idx = ((py as usize) * canvas_w + (px as usize)) * 3;
                if in_border_outer && !in_border_inner {
                    rgb_pixels[idx] = br;
                    rgb_pixels[idx + 1] = bg;
                    rgb_pixels[idx + 2] = bb;
                } else {
                    rgb_pixels[idx] = 255;
                    rgb_pixels[idx + 1] = 255;
                    rgb_pixels[idx + 2] = 255;
                }
            }
        }
    }

    if let Ok(font) = FontRef::try_from_slice(BRANDING_FONT_BYTES) {
        let char_count = clean_label.chars().count().max(1) as f32;
        let max_inner_w = ((half_w - pad - stroke - 3) * 2) as f32;
        let font_size = (max_inner_w / (char_count * 0.62)).clamp(11.5, 18.0);
        render_rgb_text_line(
            &font,
            rgb_pixels,
            canvas_w,
            canvas_h,
            center_y as f32,
            PxScale::from(font_size),
            clean_label,
            brand_rgb,
        );
    }
}

fn composite_rgb_image(
    rgb_pixels: &mut [u8],
    canvas_w: usize,
    center_x: i32,
    center_y: i32,
    cushion_half_w: i32,
    cushion_half_h: i32,
    logo_bytes: &[u8],
) {
    if let Ok(dyn_img) = image::load_from_memory(logo_bytes) {
        let max_logo_w = ((cushion_half_w - 4) * 2) as u32;
        let max_logo_h = ((cushion_half_h - 4) * 2) as u32;
        let (orig_w, orig_h) = dyn_img.dimensions();
        if orig_w > 0 && orig_h > 0 {
            let ratio_w = max_logo_w as f32 / orig_w as f32;
            let ratio_h = max_logo_h as f32 / orig_h as f32;
            let ratio = ratio_w.min(ratio_h);
            let target_w = ((orig_w as f32 * ratio).round() as u32).max(1);
            let target_h = ((orig_h as f32 * ratio).round() as u32).max(1);

            let resized = dyn_img.resize_exact(target_w, target_h, FilterType::Lanczos3);
            let start_x = center_x - (target_w as i32 / 2);
            let start_y = center_y - (target_h as i32 / 2);

            for (lx, ly, pixel) in resized.pixels() {
                let px = start_x + lx as i32;
                let py = start_y + ly as i32;
                if px >= 0 && (px as usize) < canvas_w && py >= 0 {
                    let channels = pixel.channels();
                    let (r, g, b) = (channels[0], channels[1], channels[2]);
                    let alpha = if channels.len() > 3 {
                        channels[3] as f32 / 255.0
                    } else {
                        1.0
                    };
                    if alpha > 0.05 {
                        let idx = ((py as usize) * canvas_w + (px as usize)) * 3;
                        let bg_r = rgb_pixels[idx] as f32;
                        let bg_g = rgb_pixels[idx + 1] as f32;
                        let bg_b = rgb_pixels[idx + 2] as f32;

                        rgb_pixels[idx] =
                            (bg_r * (1.0 - alpha) + r as f32 * alpha).round() as u8;
                        rgb_pixels[idx + 1] =
                            (bg_g * (1.0 - alpha) + g as f32 * alpha).round() as u8;
                        rgb_pixels[idx + 2] =
                            (bg_b * (1.0 - alpha) + b as f32 * alpha).round() as u8;
                    }
                }
            }
        }
    }
}

pub fn render_standee_qr(opts: &QrRenderOptions) -> Result<Vec<u8>> {
    let clean_payload = opts.payload.trim();
    if clean_payload.is_empty() {
        bail!("QR payload cannot be empty");
    }

    let qr = QrCode::with_error_correction_level(clean_payload.as_bytes(), EcLevel::H)
        .context("Failed to generate QR code with Level H ECC")?;
    let size = qr.width();

    let scale = opts.scale.clamp(6, 14);
    let qr_px = size * scale;

    let margin_x = (scale * 5).max(36);
    let canvas_w = qr_px + margin_x * 2;
    let header_h = ((canvas_w as f32) * 0.20) as usize;
    let qr_card_pad = (scale * 2).max(16);
    let qr_card_h = qr_px + qr_card_pad * 2;
    let info_h = ((canvas_w as f32) * 0.46) as usize;
    let canvas_h = header_h + qr_card_h + info_h + (scale * 2);

    let mut rgb_pixels = vec![255u8; canvas_w * canvas_h * 3];

    let brand_rgb = qr_center_badge_brand_color(opts.bank_code);

    // 1. Fill Header Bar
    fill_rgb_rect(
        &mut rgb_pixels,
        canvas_w,
        canvas_h,
        0,
        0,
        canvas_w as i32,
        header_h as i32,
        brand_rgb,
    );

    // 2. Header Text
    if let Ok(font) = FontRef::try_from_slice(BRANDING_FONT_BYTES) {
        let t1_size = ((header_h as f32) * 0.28).clamp(16.0, 24.0);
        let t2_size = ((header_h as f32) * 0.17).clamp(11.0, 15.0);

        let y1 = (header_h as f32) * 0.40;
        let y2 = (header_h as f32) * 0.76;

        render_rgb_text_line(
            &font,
            &mut rgb_pixels,
            canvas_w,
            canvas_h,
            y1,
            PxScale::from(t1_size),
            "QUET MA DE THANH TOAN",
            (255, 255, 255),
        );
        render_rgb_text_line(
            &font,
            &mut rgb_pixels,
            canvas_w,
            canvas_h,
            y2,
            PxScale::from(t2_size),
            "NAPAS 24/7  -  CHUYEN KHOAN NHANH",
            (226, 232, 240),
        );
    }

    // 3. QR Card Container background & border
    let qr_card_top = (header_h + scale) as i32;
    let qr_card_bottom = (header_h + scale + qr_card_h) as i32;
    let card_left = (margin_x / 2) as i32;
    let card_right = (canvas_w as i32) - card_left;
    fill_rgb_rounded_rect(
        &mut rgb_pixels,
        canvas_w,
        canvas_h,
        card_left,
        qr_card_top,
        card_right,
        qr_card_bottom,
        14,
        (255, 255, 255),
    );
    draw_rgb_rounded_rect_border(
        &mut rgb_pixels,
        canvas_w,
        canvas_h,
        card_left,
        qr_card_top,
        card_right,
        qr_card_bottom,
        14,
        1,
        (226, 232, 240),
    );

    // 4. Draw QR Modules inside card
    let qr_ox = margin_x;
    let qr_oy = header_h + scale + qr_card_pad;

    // Center logo exclusion zone
    let qr_center_x = (qr_ox + qr_px / 2) as i32;
    let qr_center_y = (qr_oy + qr_px / 2) as i32;
    let cushion_half_w = (qr_px as f32 * 0.115) as i32;
    let cushion_half_h = (qr_px as f32 * 0.115) as i32;
    let c_left = qr_center_x - cushion_half_w - 2;
    let c_right = qr_center_x + cushion_half_w + 2;
    let c_top = qr_center_y - cushion_half_h - 2;
    let c_bottom = qr_center_y + cushion_half_h + 2;

    for y in 0..size {
        for x in 0..size {
            let in_finder = (x + 7 >= size || x < 7) && y < 7 || (x < 7 && y + 7 >= size);
            if in_finder {
                continue;
            }
            if qr[(x, y)] == Color::Dark {
                let px = qr_ox + x * scale;
                let py = qr_oy + y * scale;
                for dy in 0..scale {
                    for dx in 0..scale {
                        let cur_x = (px + dx) as i32;
                        let cur_y = (py + dy) as i32;
                        if cur_x >= c_left
                            && cur_x <= c_right
                            && cur_y >= c_top
                            && cur_y <= c_bottom
                        {
                            continue;
                        }
                        let idx = ((cur_y as usize) * canvas_w + (cur_x as usize)) * 3;
                        rgb_pixels[idx] = 0;
                        rgb_pixels[idx + 1] = 0;
                        rgb_pixels[idx + 2] = 0;
                    }
                }
            }
        }
    }

    // Draw Finder Patterns & Corner Brackets for QR
    render_rgb_rounded_finder_pattern(&mut rgb_pixels, canvas_w, qr_ox, qr_oy, scale);
    render_rgb_rounded_finder_pattern(
        &mut rgb_pixels,
        canvas_w,
        qr_ox + (size - 7) * scale,
        qr_oy,
        scale,
    );
    render_rgb_rounded_finder_pattern(
        &mut rgb_pixels,
        canvas_w,
        qr_ox,
        qr_oy + (size - 7) * scale,
        scale,
    );
    render_rgb_corner_brackets_offset(&mut rgb_pixels, canvas_w, qr_ox, qr_oy, qr_px, scale);

    // Draw Center Logo / Badge
    match opts.logo {
        CenterLogo::ImageLogo(logo_bytes) => {
            fill_rgb_rounded_rect(
                &mut rgb_pixels,
                canvas_w,
                canvas_h,
                qr_center_x - cushion_half_w,
                qr_center_y - cushion_half_h,
                qr_center_x + cushion_half_w,
                qr_center_y + cushion_half_h,
                8,
                (255, 255, 255),
            );
            draw_rgb_rounded_rect_border(
                &mut rgb_pixels,
                canvas_w,
                canvas_h,
                qr_center_x - cushion_half_w,
                qr_center_y - cushion_half_h,
                qr_center_x + cushion_half_w,
                qr_center_y + cushion_half_h,
                8,
                1,
                (226, 232, 240),
            );
            composite_rgb_image(
                &mut rgb_pixels,
                canvas_w,
                qr_center_x,
                qr_center_y,
                cushion_half_w,
                cushion_half_h,
                logo_bytes,
            );
        }
        ref badge_mode => {
            let badge_lbl = match badge_mode {
                CenterLogo::TextBadge(txt) => txt.trim(),
                _ => normalize_bank_code_for_qr(opts.bank_code),
            };
            draw_rgb_center_logo_badge(
                &mut rgb_pixels,
                canvas_w,
                canvas_h,
                qr_center_x,
                qr_center_y,
                qr_px,
                badge_lbl,
                brand_rgb,
            );
        }
    }

    // 5. Information Card at Bottom
    let info_top = (qr_card_bottom + (scale * 2) as i32) as i32;
    let info_bottom = (canvas_h as i32) - (scale * 2) as i32;
    fill_rgb_rounded_rect(
        &mut rgb_pixels,
        canvas_w,
        canvas_h,
        card_left,
        info_top,
        card_right,
        info_bottom,
        14,
        (248, 250, 252),
    );
    draw_rgb_rounded_rect_border(
        &mut rgb_pixels,
        canvas_w,
        canvas_h,
        card_left,
        info_top,
        card_right,
        info_bottom,
        14,
        1,
        (226, 232, 240),
    );

    // Render text lines inside Info Card
    if let Ok(font) = FontRef::try_from_slice(BRANDING_FONT_BYTES) {
        let bank_name = bank_display_name(opts.bank_code);
        let acc_formatted = format_bank_account_spaced(opts.account, opts.full_acc);
        let name_str = opts.account_name.map(sanitize_vietqr_name).unwrap_or_default();
        let amount_str = if opts.amount > 0 {
            format_vnd_amount(opts.amount)
        } else {
            String::new()
        };
        let memo_str = opts.memo.map(|m| sanitize_vietqr_memo(m, 40)).unwrap_or_default();

        let card_h_f = (info_bottom - info_top) as f32;

        let label_size = ((canvas_w as f32) * 0.024).clamp(11.0, 13.0);
        let val_size = ((canvas_w as f32) * 0.034).clamp(13.0, 17.0);
        let acc_size = ((canvas_w as f32) * 0.046).clamp(18.0, 24.0);

        let mut curr_y = info_top as f32 + card_h_f * 0.14;

        // Bank row
        render_rgb_text_line(
            &font,
            &mut rgb_pixels,
            canvas_w,
            canvas_h,
            curr_y,
            PxScale::from(label_size),
            "NGAN HANG THU HUONG",
            (100, 116, 139),
        );
        curr_y += card_h_f * 0.10;
        let bank_line = format!("{}", strip_vietnamese_accents(bank_name).to_ascii_uppercase());
        render_rgb_text_line(
            &font,
            &mut rgb_pixels,
            canvas_w,
            canvas_h,
            curr_y,
            PxScale::from(val_size),
            &bank_line,
            (30, 41, 59),
        );

        // Account number row
        curr_y += card_h_f * 0.16;
        render_rgb_text_line(
            &font,
            &mut rgb_pixels,
            canvas_w,
            canvas_h,
            curr_y,
            PxScale::from(label_size),
            "SO TAI KHOAN",
            (100, 116, 139),
        );
        curr_y += card_h_f * 0.13;
        render_rgb_text_line(
            &font,
            &mut rgb_pixels,
            canvas_w,
            canvas_h,
            curr_y,
            PxScale::from(acc_size),
            &acc_formatted,
            (15, 23, 42),
        );

        // Beneficiary Name row
        if !name_str.is_empty() {
            curr_y += card_h_f * 0.15;
            render_rgb_text_line(
                &font,
                &mut rgb_pixels,
                canvas_w,
                canvas_h,
                curr_y,
                PxScale::from(label_size),
                "CHU TAI KHOAN",
                (100, 116, 139),
            );
            curr_y += card_h_f * 0.10;
            render_rgb_text_line(
                &font,
                &mut rgb_pixels,
                canvas_w,
                canvas_h,
                curr_y,
                PxScale::from(val_size),
                &name_str,
                (30, 41, 59),
            );
        }

        // Amount & Memo row if present
        if !amount_str.is_empty() || !memo_str.is_empty() {
            curr_y += card_h_f * 0.12;
            let mut extra_info = String::new();
            if !amount_str.is_empty() {
                extra_info.push_str(&amount_str);
            }
            if !memo_str.is_empty() {
                if !extra_info.is_empty() {
                    extra_info.push_str("  |  ");
                }
                extra_info.push_str(&memo_str);
            }
            render_rgb_text_line(
                &font,
                &mut rgb_pixels,
                canvas_w,
                canvas_h,
                curr_y,
                PxScale::from(val_size - 1.0),
                &extra_info,
                (4, 120, 87),
            );
        }

        // Sub footer line
        let footer_y = (canvas_h as f32) - (scale as f32 * 0.8);
        render_rgb_text_line(
            &font,
            &mut rgb_pixels,
            canvas_w,
            canvas_h,
            footer_y,
            PxScale::from(10.5),
            "Ho tro chuyen nhanh 24/7 tu tat ca cac ung dung Ngan hang & Vi dien tu",
            (148, 163, 184),
        );
    }

    Ok(encode_rgb_png(&rgb_pixels, canvas_w as u32, canvas_h as u32))
}

pub fn render_vietqr_with_options(opts: QrRenderOptions) -> Result<Vec<u8>> {
    match opts.template {
        QrTemplate::SePay => render_sepay_ptn_qr(&opts),
        QrTemplate::Standee => render_standee_qr(&opts),
        QrTemplate::QrOnly => {
            let footer = QrFooterOptions {
                line1: None,
                line2: None,
                is_square: true,
            };
            render_custom_vietqr(opts.payload, opts.logo, footer, opts.scale)
        }
        QrTemplate::Compact => {
            if !opts.show_info {
                let footer = QrFooterOptions {
                    line1: None,
                    line2: None,
                    is_square: true,
                };
                render_custom_vietqr(opts.payload, opts.logo, footer, opts.scale)
            } else {
                let bank_lbl = normalize_bank_code_for_qr(opts.bank_code);
                let acc_lbl = format_bank_account_spaced(opts.account, opts.full_acc);
                let c_line1 = format!("{} • {}", bank_lbl, acc_lbl);
                let c_line2 = if opts.amount > 0 {
                    Some(format_vnd_amount(opts.amount))
                } else {
                    opts.memo.map(|m| sanitize_vietqr_memo(m, 30))
                };
                let footer = QrFooterOptions {
                    line1: opts.footer.line1.or(Some(&c_line1)),
                    line2: opts.footer.line2.or(c_line2.as_deref()),
                    is_square: false,
                };
                render_custom_vietqr(opts.payload, opts.logo, footer, opts.scale)
            }
        }
        QrTemplate::Default => {
            if !opts.show_info {
                let footer = QrFooterOptions {
                    line1: None,
                    line2: None,
                    is_square: true,
                };
                render_custom_vietqr(opts.payload, opts.logo, footer, opts.scale)
            } else if opts.footer.line1.is_none() && opts.footer.line2.is_none() {
                let bank_lbl = bank_display_name(opts.bank_code);
                let acc_lbl = format_bank_account_spaced(opts.account, opts.full_acc);
                let d_line1 = format!("STK: {} - {}", acc_lbl, bank_lbl);

                let mut d_line2 = String::new();
                if let Some(nm) = opts.account_name {
                    let sname = sanitize_vietqr_name(nm);
                    if !sname.is_empty() {
                        d_line2.push_str(&sname);
                    }
                }
                if opts.amount > 0 {
                    let amt_str = format_vnd_amount(opts.amount);
                    if !d_line2.is_empty() {
                        d_line2.push_str(" • ");
                    }
                    d_line2.push_str(&amt_str);
                } else if let Some(m) = opts.memo {
                    let sm = sanitize_vietqr_memo(m, 25);
                    if !sm.is_empty() {
                        if !d_line2.is_empty() {
                            d_line2.push_str(" • ");
                        }
                        d_line2.push_str(&sm);
                    }
                }
                let footer = QrFooterOptions {
                    line1: Some(&d_line1),
                    line2: if d_line2.is_empty() {
                        None
                    } else {
                        Some(&d_line2)
                    },
                    is_square: false,
                };
                render_custom_vietqr(opts.payload, opts.logo, footer, opts.scale)
            } else {
                render_custom_vietqr(opts.payload, opts.logo, opts.footer, opts.scale)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SePay / PTN Full Color Template Rendering
// ---------------------------------------------------------------------------

fn render_rgb_square_finder_pattern(
    rgb_pixels: &mut [u8],
    canvas_w: usize,
    ox: usize,
    oy: usize,
    scale: usize,
) {
    let s = scale as i32;
    let finder_px = 7 * s;
    for ly in 0..finder_px {
        for lx in 0..finder_px {
            let in_outer = lx >= 0 && lx < finder_px && ly >= 0 && ly < finder_px;
            let in_white = lx >= s && lx < finder_px - s && ly >= s && ly < finder_px - s;
            let in_core =
                lx >= 2 * s && lx < finder_px - 2 * s && ly >= 2 * s && ly < finder_px - 2 * s;
            let is_dark = in_core || (in_outer && !in_white);

            let px = ox + lx as usize;
            let py = oy + ly as usize;
            let idx = (py * canvas_w + px) * 3;
            let col = if is_dark { 0 } else { 255 };
            rgb_pixels[idx] = col;
            rgb_pixels[idx + 1] = col;
            rgb_pixels[idx + 2] = col;
        }
    }
}

fn blit_image_centered(
    rgb_pixels: &mut [u8],
    canvas_w: usize,
    canvas_h: usize,
    center_x: i32,
    center_y: i32,
    img_bytes: &[u8],
) {
    if let Ok(dyn_img) = image::load_from_memory(img_bytes) {
        let (w, h) = dyn_img.dimensions();
        let start_x = center_x - (w as i32 / 2);
        let start_y = center_y - (h as i32 / 2);
        for (lx, ly, pixel) in dyn_img.pixels() {
            let px = start_x + lx as i32;
            let py = start_y + ly as i32;
            if px >= 0 && (px as usize) < canvas_w && py >= 0 && (py as usize) < canvas_h {
                let channels = pixel.channels();
                let alpha = if channels.len() > 3 {
                    channels[3] as f32 / 255.0
                } else {
                    1.0
                };
                if alpha > 0.02 {
                    let idx = ((py as usize) * canvas_w + (px as usize)) * 3;
                    let (r, g, b) =
                        (channels[0] as f32, channels[1] as f32, channels[2] as f32);
                    let bg_r = rgb_pixels[idx] as f32;
                    let bg_g = rgb_pixels[idx + 1] as f32;
                    let bg_b = rgb_pixels[idx + 2] as f32;

                    rgb_pixels[idx] = (bg_r * (1.0 - alpha) + r * alpha).round() as u8;
                    rgb_pixels[idx + 1] = (bg_g * (1.0 - alpha) + g * alpha).round() as u8;
                    rgb_pixels[idx + 2] = (bg_b * (1.0 - alpha) + b * alpha).round() as u8;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_rgb_text_pair(
    font: &FontRef,
    rgb_pixels: &mut [u8],
    width: usize,
    height: usize,
    center_y: f32,
    scale: PxScale,
    prefix: &str,
    prefix_color: (u8, u8, u8),
    val: &str,
    val_color: (u8, u8, u8),
) {
    let scaled_font = font.as_scaled(scale);
    let mut prefix_width = 0.0f32;
    for ch in prefix.chars() {
        if !ch.is_control() {
            prefix_width += scaled_font.h_advance(font.glyph_id(ch));
        }
    }
    let mut val_width = 0.0f32;
    for ch in val.chars() {
        if !ch.is_control() {
            val_width += scaled_font.h_advance(font.glyph_id(ch));
        }
    }
    let total_width = prefix_width + val_width;
    let start_x = ((width as f32 - total_width) / 2.0).max(0.0);
    let text_height = scaled_font.ascent() - scaled_font.descent();
    let offset_y = center_y - (text_height / 2.0);

    let mut draw_segment = |mut cur_x: f32, text: &str, color: (u8, u8, u8)| -> f32 {
        let (tr, tg, tb) = color;
        for ch in text.chars() {
            if ch.is_control() {
                continue;
            }
            let glyph_id = font.glyph_id(ch);
            let glyph = glyph_id.with_scale_and_position(
                scale,
                point(cur_x, scaled_font.ascent() + offset_y),
            );
            cur_x += scaled_font.h_advance(glyph_id);
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                let base_x = bounds.min.x.round() as i32;
                let base_y = bounds.min.y.round() as i32;
                outlined.draw(|x, y, coverage| {
                    let cov = coverage.clamp(0.0, 1.0);
                    if cov <= 0.08 {
                        return;
                    }
                    let px = base_x + x as i32;
                    let py = base_y + y as i32;
                    if px >= 0 && (px as usize) < width && py >= 0 && (py as usize) < height {
                        let idx = ((py as usize) * width + (px as usize)) * 3;
                        let curr_r = rgb_pixels[idx] as f32;
                        let curr_g = rgb_pixels[idx + 1] as f32;
                        let curr_b = rgb_pixels[idx + 2] as f32;

                        rgb_pixels[idx] = (curr_r * (1.0 - cov) + tr as f32 * cov).round() as u8;
                        rgb_pixels[idx + 1] =
                            (curr_g * (1.0 - cov) + tg as f32 * cov).round() as u8;
                        rgb_pixels[idx + 2] =
                            (curr_b * (1.0 - cov) + tb as f32 * cov).round() as u8;
                    }
                });
            }
        }
        cur_x
    };

    let next_x = draw_segment(start_x, prefix, prefix_color);
    draw_segment(next_x, val, val_color);
}

pub fn render_sepay_ptn_qr(opts: &QrRenderOptions) -> Result<Vec<u8>> {
    let clean_payload = opts.payload.trim();
    if clean_payload.is_empty() {
        bail!("QR payload cannot be empty");
    }

    let qr = QrCode::with_error_correction_level(clean_payload.as_bytes(), EcLevel::H)
        .context("Failed to generate QR code with Level H ECC")?;
    let size = qr.width();

    let canvas_w = 540usize;
    let canvas_h = 687usize;

    let mut rgb_pixels = vec![255u8; canvas_w * canvas_h * 3];

    // 1. Draw Top Header: PTN Logo
    blit_image_centered(
        &mut rgb_pixels,
        canvas_w,
        canvas_h,
        (canvas_w / 2) as i32,
        45,
        PTN_LOGO_BYTES,
    );

    // 2. Outer QR frame with dark navy border
    let frame_left = 85i32;
    let frame_right = (canvas_w - 85) as i32;
    let frame_top = 82i32;
    let frame_bottom = 445i32;
    let navy_color = (15, 35, 75);
    draw_rgb_rounded_rect_border(
        &mut rgb_pixels,
        canvas_w,
        canvas_h,
        frame_left,
        frame_top,
        frame_right,
        frame_bottom,
        4,
        2,
        navy_color,
    );

    // 3. Draw QR matrix inside frame
    let frame_w = (frame_right - frame_left) as usize;
    let frame_h = (frame_bottom - frame_top) as usize;

    let max_qr_px = frame_w - 32;
    let scale = (max_qr_px / size).max(4);
    let qr_px = size * scale;
    let qr_ox = frame_left as usize + (frame_w - qr_px) / 2;
    let qr_oy = frame_top as usize + (frame_h - qr_px) / 2;

    let qr_center_x = (qr_ox + qr_px / 2) as i32;
    let qr_center_y = (qr_oy + qr_px / 2) as i32;
    let cushion_half_w = 34i32;
    let cushion_half_h = 34i32;
    let c_left = qr_center_x - cushion_half_w;
    let c_right = qr_center_x + cushion_half_w;
    let c_top = qr_center_y - cushion_half_h;
    let c_bottom = qr_center_y + cushion_half_h;

    for y in 0..size {
        for x in 0..size {
            let in_finder = (x + 7 >= size || x < 7) && y < 7 || (x < 7 && y + 7 >= size);
            if in_finder {
                continue;
            }
            if qr[(x, y)] == Color::Dark {
                let px = qr_ox + x * scale;
                let py = qr_oy + y * scale;
                for dy in 0..scale {
                    for dx in 0..scale {
                        let cur_x = (px + dx) as i32;
                        let cur_y = (py + dy) as i32;
                        if cur_x >= c_left
                            && cur_x <= c_right
                            && cur_y >= c_top
                            && cur_y <= c_bottom
                        {
                            continue;
                        }
                        let idx = ((cur_y as usize) * canvas_w + (cur_x as usize)) * 3;
                        rgb_pixels[idx] = 0;
                        rgb_pixels[idx + 1] = 0;
                        rgb_pixels[idx + 2] = 0;
                    }
                }
            }
        }
    }

    // Draw Finder Patterns (square with inner core, standard VietQR look)
    render_rgb_square_finder_pattern(&mut rgb_pixels, canvas_w, qr_ox, qr_oy, scale);
    render_rgb_square_finder_pattern(
        &mut rgb_pixels,
        canvas_w,
        qr_ox + (size - 7) * scale,
        qr_oy,
        scale,
    );
    render_rgb_square_finder_pattern(
        &mut rgb_pixels,
        canvas_w,
        qr_ox,
        qr_oy + (size - 7) * scale,
        scale,
    );

    // 4. Center Logo inside QR
    match opts.logo {
        CenterLogo::ImageLogo(logo_bytes) => {
            composite_rgb_image(
                &mut rgb_pixels,
                canvas_w,
                qr_center_x,
                qr_center_y,
                cushion_half_w,
                cushion_half_h,
                logo_bytes,
            );
        }
        _ => {
            blit_image_centered(
                &mut rgb_pixels,
                canvas_w,
                canvas_h,
                qr_center_x,
                qr_center_y,
                VIETQR_CENTER_BYTES,
            );
        }
    }

    // 5. Draw Partner Trust Bar (napas 24/7 | TPBank | VIETQR)
    blit_image_centered(
        &mut rgb_pixels,
        canvas_w,
        canvas_h,
        (canvas_w / 2) as i32,
        472,
        PARTNER_BAR_BYTES,
    );

    // 6. Beneficiary Information block
    if let Ok(font) = FontRef::try_from_slice(BRANDING_FONT_BYTES) {
        let name_str = opts
            .account_name
            .map(sanitize_vietqr_name)
            .unwrap_or_else(|| {
                if opts.account == "69238686888" {
                    "PHAM THANH NAM".to_string()
                } else {
                    "CHU TAI KHOAN".to_string()
                }
            });
        let acc_formatted = format_bank_account_spaced(opts.account, opts.full_acc);
        let amount_str = if opts.amount > 0 {
            format_vnd_amount(opts.amount)
        } else {
            String::new()
        };
        let bank_name = bank_display_name(opts.bank_code);

        // Line 1: Tên chủ TK: PHAM THANH NAM
        render_rgb_text_pair(
            &font,
            &mut rgb_pixels,
            canvas_w,
            canvas_h,
            528.0,
            PxScale::from(15.0),
            "Ten chu TK: ",
            (100, 116, 139), // Slate 500
            &name_str,
            (15, 23, 42), // Slate 900
        );

        // Line 2: Số TK: 69238686888
        render_rgb_text_pair(
            &font,
            &mut rgb_pixels,
            canvas_w,
            canvas_h,
            556.0,
            PxScale::from(16.5),
            "So TK: ",
            (100, 116, 139),
            &acc_formatted,
            (24, 73, 214), // Royal Blue
        );

        // Line 3: Số tiền: 500.000 VND (or Memo if amount is 0)
        let mut next_y = 584.0;
        if !amount_str.is_empty() {
            render_rgb_text_pair(
                &font,
                &mut rgb_pixels,
                canvas_w,
                canvas_h,
                next_y,
                PxScale::from(15.5),
                "So tien: ",
                (100, 116, 139),
                &amount_str,
                (24, 73, 214), // Royal Blue
            );
            next_y += 26.0;
        }

        if let Some(memo) = opts.memo {
            let clean_memo = sanitize_vietqr_memo(memo, 35);
            if !clean_memo.is_empty() {
                render_rgb_text_pair(
                    &font,
                    &mut rgb_pixels,
                    canvas_w,
                    canvas_h,
                    next_y,
                    PxScale::from(14.0),
                    "Noi dung: ",
                    (100, 116, 139),
                    &clean_memo,
                    (51, 65, 85),
                );
                next_y += 24.0;
            }
        }

        // Line 4: Bank Full Name
        let clean_bank = strip_vietnamese_accents(bank_name);
        render_rgb_text_line(
            &font,
            &mut rgb_pixels,
            canvas_w,
            canvas_h,
            next_y,
            PxScale::from(13.5),
            &clean_bank,
            (100, 116, 139),
        );
    }

    Ok(encode_rgb_png(&rgb_pixels, canvas_w as u32, canvas_h as u32))
}

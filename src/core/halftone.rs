use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use anyhow::{Context, Result, bail};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, Pixel, Rgba, RgbaImage};
use qrcode::{Color, EcLevel, QrCode};

use crate::core::canvas::{BRANDING_FONT_BYTES, CenterLogo, PTN_LOGO_BYTES, QrRenderOptions};
use crate::core::emvco::{
    bank_display_name, format_bank_account_spaced, format_vnd_amount, qr_center_badge_brand_color,
    sanitize_vietqr_memo, sanitize_vietqr_name, strip_vietnamese_accents,
};
use crate::core::png_encoder::encode_rgb_png;

/// Sub-pixel subdivision per QR module (3x3 sub-pixels, center 1x1 preserves QR data bit).
const SUB: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DitherMethod {
    #[default]
    FloydSteinberg,
    Atkinson,
    Bayer4x4,
    Threshold,
}

impl DitherMethod {
    pub fn from_str_opt(s: Option<&str>) -> Self {
        match s.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("atkinson") | Some("atk") => DitherMethod::Atkinson,
            Some("bayer") | Some("bayer4") | Some("ordered") => DitherMethod::Bayer4x4,
            Some("threshold") | Some("thresh") | Some("binary") => DitherMethod::Threshold,
            _ => DitherMethod::FloydSteinberg,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HalftoneColorMode {
    #[default]
    Custom,
    BankBrand,
    PhotoRgb,
}

impl HalftoneColorMode {
    pub fn from_str_opt(s: Option<&str>) -> Self {
        match s.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("bank") | Some("brand") | Some("auto") => HalftoneColorMode::BankBrand,
            Some("photo") | Some("image") | Some("rgb") | Some("multicolor") => {
                HalftoneColorMode::PhotoRgb
            }
            _ => HalftoneColorMode::Custom,
        }
    }
}

/// Parses a `#RRGGBB`, `RRGGBB`, or `#RGB` hex color string into `(r, g, b)`.
pub fn parse_hex_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let clean = s.trim().trim_start_matches('#');
    if clean.len() == 6 {
        let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
        let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
        let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
        Some((r, g, b))
    } else if clean.len() == 3 {
        let r = u8::from_str_radix(&clean[0..1], 16).ok()? * 17;
        let g = u8::from_str_radix(&clean[1..2], 16).ok()? * 17;
        let b = u8::from_str_radix(&clean[2..3], 16).ok()? * 17;
        Some((r, g, b))
    } else {
        None
    }
}

/// Clamps a foreground RGB color so its perceptual luminance stays dark enough (`Y <= 125`)
/// for 100% reliable banking camera QR decoding while preserving hue and saturation.
#[inline]
pub fn clamp_dark_rgb(rgb: (u8, u8, u8)) -> (u8, u8, u8) {
    let (r, g, b) = (rgb.0 as f32, rgb.1 as f32, rgb.2 as f32);
    let luma = 0.299 * r + 0.587 * g + 0.114 * b;
    let max_luma = 120.0f32;
    if luma > max_luma && luma > 1.0 {
        let factor = max_luma / luma;
        (
            (r * factor).round() as u8,
            (g * factor).round() as u8,
            (b * factor).round() as u8,
        )
    } else {
        rgb
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HalftoneParams {
    pub dither: DitherMethod,
    pub contrast: f32,
    pub brightness: f32,
    pub color_mode: HalftoneColorMode,
    pub fg_color: (u8, u8, u8),
    pub bg_color: (u8, u8, u8),
}

impl Default for HalftoneParams {
    fn default() -> Self {
        Self {
            dither: DitherMethod::FloydSteinberg,
            contrast: 1.25,
            brightness: 0.0,
            color_mode: HalftoneColorMode::Custom,
            fg_color: (15, 23, 42),
            bg_color: (255, 255, 255),
        }
    }
}

/// Returns alignment pattern center coordinates for QR version `v` (1..=40).
fn alignment_pattern_centers(version: usize) -> Vec<usize> {
    if version < 2 {
        return Vec::new();
    }
    let count = version / 7 + 2;
    let size = version * 4 + 17;
    let first = 6usize;
    let last = size - 7;
    if count == 2 {
        return vec![first, last];
    }
    let span = last - first;
    let step = ((span as f32 / (count as f32 - 1.0)).ceil() as usize + 1) & !1;
    let mut coords = vec![first];
    for i in 1..(count - 1) {
        coords.push(last - (count - 1 - i) * step);
    }
    coords.push(last);
    coords
}

/// Checks whether module `(x, y)` belongs to a Finder Pattern (with separator)
/// or an Alignment Pattern that must remain solid for instant camera lock.
#[inline]
pub fn is_protected_qr_module(x: usize, y: usize, size: usize) -> bool {
    // 1. Three 8x8 Finder Pattern + Separator zones
    let in_top_left = x < 8 && y < 8;
    let in_top_right = x + 8 >= size && y < 8;
    let in_bottom_left = x < 8 && y + 8 >= size;
    if in_top_left || in_top_right || in_bottom_left {
        return true;
    }

    // 2. Dark module
    if x == 8 && y + 8 == size {
        return true;
    }

    // 3. 5x5 Alignment Patterns (skip if overlapping finder corners)
    let version = (size.saturating_sub(17)) / 4;
    if version >= 2 {
        let centers = alignment_pattern_centers(version);
        for &cy in &centers {
            for &cx in &centers {
                let overlaps_finder = (cx < 8 && cy < 8)
                    || (cx + 8 >= size && cy < 8)
                    || (cx < 8 && cy + 8 >= size);
                if overlaps_finder {
                    continue;
                }
                if x >= cx.saturating_sub(2)
                    && x <= cx + 2
                    && y >= cy.saturating_sub(2)
                    && y <= cy + 2
                {
                    return true;
                }
            }
        }
    }

    false
}

/// Builds a default multi-tone artwork image when the user has not uploaded a custom photo yet.
fn build_default_halftone_artwork(target_px: u32) -> DynamicImage {
    let mut img = RgbaImage::new(target_px, target_px);
    let center = target_px as f32 / 2.0;
    let max_r = center * 1.15;

    for y in 0..target_px {
        for x in 0..target_px {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt() / max_r;
            let ring = ((dist * std::f32::consts::PI * 6.0).cos() * 0.18) + (1.0 - dist * 0.65);
            let val = ring.clamp(0.12, 0.92);
            let r = (val * 225.0).round() as u8;
            let g = ((val * 245.0).min(255.0)).round() as u8;
            let b = ((val * 255.0).min(255.0)).round() as u8;
            img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }

    if let Ok(logo) = image::load_from_memory(PTN_LOGO_BYTES) {
        let logo_w = (target_px as f32 * 0.72).round() as u32;
        let ratio = logo.height() as f32 / logo.width().max(1) as f32;
        let logo_h = (logo_w as f32 * ratio).round().max(1.0) as u32;
        let resized = logo.resize_exact(logo_w, logo_h, FilterType::Triangle);
        let ox = (target_px.saturating_sub(logo_w)) / 2;
        let oy = (target_px.saturating_sub(logo_h)) / 2;
        for (lx, ly, p) in resized.pixels() {
            let ch = p.channels();
            let alpha = ch[3] as f32 / 255.0;
            if alpha > 0.05 {
                let dst = img.get_pixel_mut(ox + lx, oy + ly);
                let r = (dst.0[0] as f32 * (1.0 - alpha) + ch[0] as f32 * alpha).round() as u8;
                let g = (dst.0[1] as f32 * (1.0 - alpha) + ch[1] as f32 * alpha).round() as u8;
                let b = (dst.0[2] as f32 * (1.0 - alpha) + ch[2] as f32 * alpha).round() as u8;
                *dst = Rgba([r, g, b, 255]);
            }
        }
    }

    DynamicImage::ImageRgba8(img)
}

/// Extracts high-contrast dark RGB colors per sub-pixel from `img` for `HalftoneColorMode::PhotoRgb`.
fn extract_photo_rgb_grid(img: &DynamicImage, grid_w: usize) -> Vec<(u8, u8, u8)> {
    let (src_w, src_h) = img.dimensions();
    let side = src_w.min(src_h).max(1);
    let crop_x = (src_w.saturating_sub(side)) / 2;
    let crop_y = (src_h.saturating_sub(side)) / 2;
    let cropped = img.crop_imm(crop_x, crop_y, side, side);
    let resized = cropped.resize_exact(grid_w as u32, grid_w as u32, FilterType::Triangle);

    let mut colors = vec![(15u8, 23u8, 42u8); grid_w * grid_w];
    for (x, y, pixel) in resized.pixels() {
        let ch = pixel.channels();
        let alpha = if ch.len() > 3 {
            ch[3] as f32 / 255.0
        } else {
            1.0
        };
        let r = (ch[0] as f32 * alpha + 15.0 * (1.0 - alpha)).round() as u8;
        let g = (ch[1] as f32 * alpha + 23.0 * (1.0 - alpha)).round() as u8;
        let b = (ch[2] as f32 * alpha + 42.0 * (1.0 - alpha)).round() as u8;
        colors[(y as usize) * grid_w + (x as usize)] = clamp_dark_rgb((r, g, b));
    }
    colors
}

/// Center-crops, resizes, adjusts contrast/brightness, and dithers an image to a binary `w x w` grid.
/// Returns `Vec<bool>` where `true` means dark sub-pixel and `false` means light sub-pixel.
pub fn dither_image_to_grid(
    img: &DynamicImage,
    grid_w: usize,
    params: HalftoneParams,
) -> Vec<bool> {
    let (src_w, src_h) = img.dimensions();
    let side = src_w.min(src_h).max(1);
    let crop_x = (src_w.saturating_sub(side)) / 2;
    let crop_y = (src_h.saturating_sub(side)) / 2;

    let cropped = img.crop_imm(crop_x, crop_y, side, side);
    let resized = cropped.resize_exact(grid_w as u32, grid_w as u32, FilterType::Triangle);

    let contrast = params.contrast.clamp(0.2, 3.0);
    let brightness = params.brightness.clamp(-0.8, 0.8);

    let mut luma = vec![1.0f32; grid_w * grid_w];
    for (x, y, pixel) in resized.pixels() {
        let ch = pixel.channels();
        let alpha = if ch.len() > 3 {
            ch[3] as f32 / 255.0
        } else {
            1.0
        };
        // Composite over white background so transparent pixels stay light
        let r = ch[0] as f32 * alpha + 255.0 * (1.0 - alpha);
        let g = ch[1] as f32 * alpha + 255.0 * (1.0 - alpha);
        let b = ch[2] as f32 * alpha + 255.0 * (1.0 - alpha);

        let raw = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
        let adjusted = ((raw - 0.5) * contrast + 0.5 + brightness).clamp(0.0, 1.0);
        luma[(y as usize) * grid_w + (x as usize)] = adjusted;
    }

    let mut dark_grid = vec![false; grid_w * grid_w];

    match params.dither {
        DitherMethod::Threshold => {
            for (i, &v) in luma.iter().enumerate() {
                dark_grid[i] = v < 0.5;
            }
        }
        DitherMethod::Bayer4x4 => {
            const BAYER: [[u8; 4]; 4] = [
                [0, 8, 2, 10],
                [12, 4, 14, 6],
                [3, 11, 1, 9],
                [15, 7, 13, 5],
            ];
            for y in 0..grid_w {
                for x in 0..grid_w {
                    let threshold = (BAYER[y & 3][x & 3] as f32 + 0.5) / 16.0;
                    dark_grid[y * grid_w + x] = luma[y * grid_w + x] < threshold;
                }
            }
        }
        DitherMethod::Atkinson => {
            let w = grid_w as isize;
            for y in 0..w {
                for x in 0..w {
                    let idx = (y * w + x) as usize;
                    let old = luma[idx];
                    let new = if old < 0.5 { 0.0 } else { 1.0 };
                    dark_grid[idx] = new < 0.5;
                    let err = (old - new) / 8.0;
                    for (dx, dy) in [(1, 0), (2, 0), (-1, 1), (0, 1), (1, 1), (0, 2)] {
                        let nx = x + dx;
                        let ny = y + dy;
                        if nx >= 0 && nx < w && ny >= 0 && ny < w {
                            let nidx = (ny * w + nx) as usize;
                            luma[nidx] = (luma[nidx] + err).clamp(0.0, 1.0);
                        }
                    }
                }
            }
        }
        DitherMethod::FloydSteinberg => {
            let w = grid_w as isize;
            for y in 0..w {
                let serpentine = y % 2 == 1;
                let dir: isize = if serpentine { -1 } else { 1 };
                for step in 0..w {
                    let x = if serpentine { w - 1 - step } else { step };
                    let idx = (y * w + x) as usize;
                    let old = luma[idx];
                    let new = if old < 0.5 { 0.0 } else { 1.0 };
                    dark_grid[idx] = new < 0.5;
                    let err = old - new;
                    for (dx, dy, weight) in [
                        (dir, 0, 7.0 / 16.0),
                        (-dir, 1, 3.0 / 16.0),
                        (0, 1, 5.0 / 16.0),
                        (dir, 1, 1.0 / 16.0),
                    ] {
                        let nx = x + dx;
                        let ny = y + dy;
                        if nx >= 0 && nx < w && ny >= 0 && ny < w {
                            let nidx = (ny * w + nx) as usize;
                            luma[nidx] = (luma[nidx] + err * weight).clamp(0.0, 1.0);
                        }
                    }
                }
            }
        }
    }

    dark_grid
}

#[allow(clippy::too_many_arguments)]
fn draw_centered_text(
    font: &FontRef,
    rgb_pixels: &mut [u8],
    width: usize,
    height: usize,
    center_y: f32,
    scale: PxScale,
    text: &str,
    color: (u8, u8, u8),
) {
    let scaled_font = font.as_scaled(scale);
    let mut text_width = 0.0f32;
    for ch in text.chars() {
        if !ch.is_control() {
            text_width += scaled_font.h_advance(font.glyph_id(ch));
        }
    }
    let start_x = ((width as f32 - text_width) / 2.0).max(0.0);
    let text_height = scaled_font.ascent() - scaled_font.descent();
    let offset_y = center_y - (text_height / 2.0);
    let (tr, tg, tb) = color;

    let mut cur_x = start_x;
    for ch in text.chars() {
        if ch.is_control() {
            continue;
        }
        let glyph_id = font.glyph_id(ch);
        let glyph =
            glyph_id.with_scale_and_position(scale, point(cur_x, scaled_font.ascent() + offset_y));
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
                    let r = rgb_pixels[idx] as f32;
                    let g = rgb_pixels[idx + 1] as f32;
                    let b = rgb_pixels[idx + 2] as f32;
                    rgb_pixels[idx] = (r * (1.0 - cov) + tr as f32 * cov).round() as u8;
                    rgb_pixels[idx + 1] = (g * (1.0 - cov) + tg as f32 * cov).round() as u8;
                    rgb_pixels[idx + 2] = (b * (1.0 - cov) + tb as f32 * cov).round() as u8;
                }
            });
        }
    }
}

/// Renders a standalone Halftone VietQR PNG image from `QrRenderOptions` and `HalftoneParams`.
pub fn render_halftone_vietqr(opts: &QrRenderOptions, ht: HalftoneParams) -> Result<Vec<u8>> {
    let clean_payload = opts.payload.trim();
    if clean_payload.is_empty() {
        bail!("QR payload cannot be empty");
    }

    let qr = QrCode::with_error_correction_level(clean_payload.as_bytes(), EcLevel::H)
        .context("Failed to generate Halftone QR code with Level H ECC")?;
    let n = qr.width();
    let sub_w = n * SUB;

    // Load user-uploaded background image if provided via CenterLogo::ImageLogo, else fallback artwork
    let dyn_img = match opts.logo {
        CenterLogo::ImageLogo(bytes) if !bytes.is_empty() => image::load_from_memory(bytes)
            .unwrap_or_else(|_| build_default_halftone_artwork(sub_w as u32)),
        _ => build_default_halftone_artwork(sub_w as u32),
    };

    let dither_grid = dither_image_to_grid(&dyn_img, sub_w, ht);
    let photo_rgb_grid = if ht.color_mode == HalftoneColorMode::PhotoRgb {
        Some(extract_photo_rgb_grid(&dyn_img, sub_w))
    } else {
        None
    };

    let base_fg = match ht.color_mode {
        HalftoneColorMode::BankBrand => {
            clamp_dark_rgb(qr_center_badge_brand_color(opts.bank_code))
        }
        HalftoneColorMode::Custom | HalftoneColorMode::PhotoRgb => clamp_dark_rgb(ht.fg_color),
    };
    let (bg_r, bg_g, bg_b) = ht.bg_color;

    // Pixel size of each sub-pixel (each QR module is `SUB * sub_px` pixels wide)
    let sub_px = (opts.scale / 2).clamp(3, 6);
    let mod_px = SUB * sub_px;
    let quiet_mods = 3usize;
    let qr_area_px = (n + quiet_mods * 2) * mod_px;

    let footer_h = if opts.show_info { 132usize } else { 0usize };
    let canvas_w = qr_area_px;
    let canvas_h = qr_area_px + footer_h;

    let mut rgb_pixels = vec![255u8; canvas_w * canvas_h * 3];
    if (bg_r, bg_g, bg_b) != (255, 255, 255) {
        for chunk in rgb_pixels.chunks_exact_mut(3) {
            chunk[0] = bg_r;
            chunk[1] = bg_g;
            chunk[2] = bg_b;
        }
    }
    let qr_ox = quiet_mods * mod_px;
    let qr_oy = quiet_mods * mod_px;

    // Render every module's 3x3 sub-pixels
    for my in 0..n {
        for mx in 0..n {
            let qr_dark = qr[(mx, my)] == Color::Dark;
            let protected = is_protected_qr_module(mx, my, n);

            for sy in 0..SUB {
                for sx in 0..SUB {
                    let sub_idx = (my * SUB + sy) * sub_w + (mx * SUB + sx);
                    let is_dark = if protected || (sx == 1 && sy == 1) {
                        qr_dark
                    } else {
                        dither_grid[sub_idx]
                    };

                    if is_dark {
                        let (dr, dg, db) = if protected {
                            base_fg
                        } else if let Some(ref rgb_grid) = photo_rgb_grid {
                            rgb_grid[sub_idx]
                        } else {
                            base_fg
                        };

                        let start_px = qr_ox + mx * mod_px + sx * sub_px;
                        let start_py = qr_oy + my * mod_px + sy * sub_px;
                        for dy in 0..sub_px {
                            let row_offset = (start_py + dy) * canvas_w;
                            for dx in 0..sub_px {
                                let idx = (row_offset + start_px + dx) * 3;
                                rgb_pixels[idx] = dr;
                                rgb_pixels[idx + 1] = dg;
                                rgb_pixels[idx + 2] = db;
                            }
                        }
                    }
                }
            }
        }
    }

    // Optional Beneficiary Info Footer
    if opts.show_info {
        let sep_y = qr_area_px.saturating_sub(6);
        let pad_x = quiet_mods * mod_px;
        for x in pad_x..(canvas_w.saturating_sub(pad_x)) {
            let idx = (sep_y * canvas_w + x) * 3;
            rgb_pixels[idx] = 226;
            rgb_pixels[idx + 1] = 232;
            rgb_pixels[idx + 2] = 240;
        }

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
            let bank_short = opts.bank_code.trim().to_ascii_uppercase();
            let bank_full = strip_vietnamese_accents(bank_display_name(opts.bank_code));

            let line1 = format!("{bank_short} • {acc_formatted}");
            let mut line2 = name_str;
            if opts.amount > 0 {
                line2.push_str(" • ");
                line2.push_str(&format_vnd_amount(opts.amount));
            }
            let line3 = if let Some(memo) = opts.memo {
                let m = sanitize_vietqr_memo(memo, 34);
                if !m.is_empty() {
                    format!("ND: {m}")
                } else {
                    bank_full
                }
            } else {
                bank_full
            };

            let base_y = qr_area_px as f32 + 24.0;
            draw_centered_text(
                &font,
                &mut rgb_pixels,
                canvas_w,
                canvas_h,
                base_y,
                PxScale::from(18.0),
                &line1,
                base_fg,
            );
            draw_centered_text(
                &font,
                &mut rgb_pixels,
                canvas_w,
                canvas_h,
                base_y + 32.0,
                PxScale::from(16.0),
                &line2,
                (4, 120, 87),
            );
            draw_centered_text(
                &font,
                &mut rgb_pixels,
                canvas_w,
                canvas_h,
                base_y + 62.0,
                PxScale::from(13.5),
                &line3,
                (100, 116, 139),
            );
        }
    }

    Ok(encode_rgb_png(&rgb_pixels, canvas_w as u32, canvas_h as u32))
}

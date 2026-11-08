use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, OnceLock, RwLock};

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use anyhow::{Context, Result, bail};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, Pixel, Rgba, RgbaImage};
use qrcode::{Color, EcLevel, QrCode};
use serde::{Deserialize, Serialize};

use crate::core::canvas::{BRANDING_FONT_BYTES, CenterLogo, PTN_LOGO_BYTES, QrRenderOptions};
use crate::core::emvco::{
    bank_display_name, format_bank_account_spaced, format_vnd_amount, qr_center_badge_brand_color,
    sanitize_vietqr_memo, sanitize_vietqr_name, strip_vietnamese_accents,
};
use crate::core::png_encoder::encode_rgb_png;

/// Sub-pixel subdivision per QR module (3x3 sub-pixels, center 1x1 preserves QR data bit).
const SUB: usize = 3;

/// Canonical sub-pixel grid width for exported Halftone JSON presets (45 modules * 3 = 135).
pub const CANONICAL_PRESET_GRID_W: usize = 135;

/// Maximum number of precomputed presets kept in the in-memory cache.
const MAX_CACHED_PRESETS: usize = 256;

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

    pub fn as_str(self) -> &'static str {
        match self {
            DitherMethod::FloydSteinberg => "floyd",
            DitherMethod::Atkinson => "atkinson",
            DitherMethod::Bayer4x4 => "bayer",
            DitherMethod::Threshold => "threshold",
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

    pub fn as_str(self) -> &'static str {
        match self {
            HalftoneColorMode::Custom => "custom",
            HalftoneColorMode::BankBrand => "bank",
            HalftoneColorMode::PhotoRgb => "photo",
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

#[inline]
pub fn format_hex_rgb(rgb: (u8, u8, u8)) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb.0, rgb.1, rgb.2)
}

/// Clamps a foreground RGB color so its perceptual luminance stays dark enough (`Y <= 120`)
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

// ---------------------------------------------------------------------------
// Precomputed Halftone JSON Preset & In-Memory Registry
// ---------------------------------------------------------------------------

/// Serializable JSON preset containing the pre-dithered sub-pixel matrix (~2-4 KB)
/// so clients can render Halftone QRs in < 0.6ms without re-uploading or decoding raw images.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalftonePresetJson {
    pub preset_id: String,
    pub version: u8,
    pub grid_w: usize,
    pub dither: String,
    pub contrast: f32,
    pub brightness: f32,
    pub colormode: String,
    pub fg: String,
    pub bg: String,
    /// Hex-encoded bitpack (optionally zlib-compressed with `z:` prefix) of `grid_w * grid_w` dark bits.
    pub bits_pack: String,
    /// Optional zlib-compressed hex-encoded RGB buffer (`grid_w * grid_w * 3` bytes) for `photo` mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rgb_pack: Option<String>,
}

/// Decoded, ready-to-render Halftone matrix kept in RAM for zero-allocation O(1) lookup.
#[derive(Debug, Clone)]
pub struct CompiledHalftonePreset {
    pub preset_id: String,
    pub grid_w: usize,
    pub params: HalftoneParams,
    pub dark_grid: Vec<bool>,
    pub rgb_grid: Option<Vec<(u8, u8, u8)>>,
}

static PRESET_STORE: OnceLock<RwLock<HashMap<String, Arc<CompiledHalftonePreset>>>> =
    OnceLock::new();

fn preset_store() -> &'static RwLock<HashMap<String, Arc<CompiledHalftonePreset>>> {
    PRESET_STORE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn store_compiled_preset(preset: CompiledHalftonePreset) -> Arc<CompiledHalftonePreset> {
    let id = preset.preset_id.clone();
    let arc = Arc::new(preset);
    if let Ok(mut map) = preset_store().write() {
        if map.len() >= MAX_CACHED_PRESETS && !map.contains_key(&id) {
            if let Some(first_key) = map.keys().next().cloned() {
                map.remove(&first_key);
            }
        }
        map.insert(id, Arc::clone(&arc));
    }
    arc
}

pub fn get_compiled_preset(preset_id: &str) -> Option<Arc<CompiledHalftonePreset>> {
    let clean = preset_id.trim();
    preset_store()
        .read()
        .ok()
        .and_then(|map| map.get(clean).cloned())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex(s: &str) -> Result<Vec<u8>> {
    let clean = s.trim();
    if clean.len() % 2 != 0 {
        bail!("Invalid hex length");
    }
    let mut out = Vec::with_capacity(clean.len() / 2);
    let bytes = clean.as_bytes();
    for chunk in bytes.chunks_exact(2) {
        let hi = (chunk[0] as char)
            .to_digit(16)
            .context("Invalid hex char")?;
        let lo = (chunk[1] as char)
            .to_digit(16)
            .context("Invalid hex char")?;
        out.push(((hi << 4) | lo) as u8);
    }
    Ok(out)
}

fn compress_to_hex(raw: &[u8]) -> Result<String> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(raw)?;
    let compressed = encoder.finish()?;
    Ok(format!("z:{}", encode_hex(&compressed)))
}

fn decompress_from_hex(s: &str) -> Result<Vec<u8>> {
    if let Some(rest) = s.strip_prefix("z:") {
        let compressed = decode_hex(rest)?;
        let mut decoder = ZlibDecoder::new(&compressed[..]);
        let mut out = Vec::new();
        decoder.read_to_end(&mut out)?;
        Ok(out)
    } else {
        decode_hex(s)
    }
}

fn pack_bits(bits: &[bool]) -> Vec<u8> {
    let mut packed = vec![0u8; (bits.len() + 7) / 8];
    for (i, &b) in bits.iter().enumerate() {
        if b {
            packed[i >> 3] |= 1 << (i & 7);
        }
    }
    packed
}

fn unpack_bits(packed: &[u8], total_bits: usize) -> Vec<bool> {
    let mut bits = vec![false; total_bits];
    for i in 0..total_bits {
        let byte_idx = i >> 3;
        if byte_idx < packed.len() {
            bits[i] = (packed[byte_idx] & (1 << (i & 7))) != 0;
        }
    }
    bits
}

fn compute_fnv1a_preset_id(packed_bits: &[u8], params: &HalftoneParams) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in packed_bits {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for b in [
        params.fg_color.0,
        params.fg_color.1,
        params.fg_color.2,
        params.bg_color.0,
        params.bg_color.1,
        params.bg_color.2,
        params.color_mode as u8,
        params.dither as u8,
    ] {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("ht_{:012x}", hash & 0xffffffffffff)
}

/// Builds a `HalftonePresetJson` from raw image bytes (or default artwork) and registers it in RAM.
pub fn create_and_register_preset(
    img_bytes: Option<&[u8]>,
    params: HalftoneParams,
    include_rgb: bool,
) -> Result<HalftonePresetJson> {
    let grid_w = CANONICAL_PRESET_GRID_W;
    let rgba = match img_bytes {
        Some(bytes) if !bytes.is_empty() => {
            let dyn_img = image::load_from_memory(bytes)
                .context("Không thể giải mã file ảnh để tạo Preset Halftone")?;
            prepare_square_rgba(&dyn_img, grid_w)
        }
        _ => get_or_build_default_rgba(grid_w),
    };

    let dark_grid = dither_rgba_to_grid(&rgba, grid_w, params);
    let packed_bits = pack_bits(&dark_grid);
    let bits_pack = compress_to_hex(&packed_bits)?;

    let need_rgb = include_rgb || params.color_mode == HalftoneColorMode::PhotoRgb;
    let (rgb_grid, rgb_pack) = if need_rgb {
        let grid = extract_photo_rgb_from_rgba(&rgba, grid_w);
        let mut raw_rgb = Vec::with_capacity(grid.len() * 3);
        for &(r, g, b) in &grid {
            raw_rgb.push(r);
            raw_rgb.push(g);
            raw_rgb.push(b);
        }
        let encoded = compress_to_hex(&raw_rgb)?;
        (Some(grid), Some(encoded))
    } else {
        (None, None)
    };

    let preset_id = compute_fnv1a_preset_id(&packed_bits, &params);
    let json_preset = HalftonePresetJson {
        preset_id: preset_id.clone(),
        version: 1,
        grid_w,
        dither: params.dither.as_str().to_string(),
        contrast: params.contrast,
        brightness: params.brightness,
        colormode: params.color_mode.as_str().to_string(),
        fg: format_hex_rgb(params.fg_color),
        bg: format_hex_rgb(params.bg_color),
        bits_pack,
        rgb_pack,
    };

    store_compiled_preset(CompiledHalftonePreset {
        preset_id,
        grid_w,
        params,
        dark_grid,
        rgb_grid,
    });

    Ok(json_preset)
}

/// Compiles a `HalftonePresetJson` into `CompiledHalftonePreset` and registers it in RAM.
pub fn compile_and_register_preset_json(
    json: &HalftonePresetJson,
) -> Result<Arc<CompiledHalftonePreset>> {
    if json.grid_w < 21 || json.grid_w > 600 {
        bail!("Invalid preset grid_w: {}", json.grid_w);
    }
    let total = json.grid_w * json.grid_w;
    let packed_bits = decompress_from_hex(&json.bits_pack)?;
    let dark_grid = unpack_bits(&packed_bits, total);

    let rgb_grid = if let Some(ref rgb_str) = json.rgb_pack {
        let raw_rgb = decompress_from_hex(rgb_str)?;
        if raw_rgb.len() >= total * 3 {
            let mut vec_rgb = Vec::with_capacity(total);
            for chunk in raw_rgb.chunks_exact(3).take(total) {
                vec_rgb.push(clamp_dark_rgb((chunk[0], chunk[1], chunk[2])));
            }
            Some(vec_rgb)
        } else {
            None
        }
    } else {
        None
    };

    let params = HalftoneParams {
        dither: DitherMethod::from_str_opt(Some(&json.dither)),
        contrast: json.contrast.clamp(0.2, 3.0),
        brightness: json.brightness.clamp(-0.8, 0.8),
        color_mode: HalftoneColorMode::from_str_opt(Some(&json.colormode)),
        fg_color: parse_hex_rgb(&json.fg).unwrap_or((15, 23, 42)),
        bg_color: parse_hex_rgb(&json.bg).unwrap_or((255, 255, 255)),
    };

    let preset_id = if json.preset_id.trim().is_empty() {
        compute_fnv1a_preset_id(&packed_bits, &params)
    } else {
        json.preset_id.trim().to_string()
    };

    Ok(store_compiled_preset(CompiledHalftonePreset {
        preset_id,
        grid_w: json.grid_w,
        params,
        dark_grid,
        rgb_grid,
    }))
}

// ---------------------------------------------------------------------------
// Optimized Stack-Based Alignment & Function Pattern Protection
// ---------------------------------------------------------------------------

/// Stack-allocated alignment pattern center coordinates `(coords, len)` for QR version `v` (1..=40).
/// Zero heap allocations!
#[inline]
fn alignment_pattern_centers_stack(version: usize) -> ([usize; 7], usize) {
    let mut coords = [0usize; 7];
    if version < 2 {
        return (coords, 0);
    }
    let count = (version / 7 + 2).min(7);
    let size = version * 4 + 17;
    let first = 6usize;
    let last = size - 7;
    coords[0] = first;
    if count == 2 {
        coords[1] = last;
        return (coords, 2);
    }
    let span = last - first;
    let step = ((span as f32 / (count as f32 - 1.0)).ceil() as usize + 1) & !1;
    for i in 1..(count - 1) {
        coords[i] = last - (count - 1 - i) * step;
    }
    coords[count - 1] = last;
    (coords, count)
}

#[inline]
fn is_protected_with_centers(
    x: usize,
    y: usize,
    size: usize,
    centers: &[usize],
) -> bool {
    // 1. Three 8x8 Finder Pattern + Separator zones
    if (x < 8 && y < 8) || (x + 8 >= size && y < 8) || (x < 8 && y + 8 >= size) {
        return true;
    }

    // 2. Dark module
    if x == 8 && y + 8 == size {
        return true;
    }

    // 3. 5x5 Alignment Patterns (skip if overlapping finder corners)
    for &cy in centers {
        for &cx in centers {
            if (cx < 8 && cy < 8) || (cx + 8 >= size && cy < 8) || (cx < 8 && cy + 8 >= size) {
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

    false
}

/// Public helper for checking if module `(x, y)` is protected.
#[inline]
pub fn is_protected_qr_module(x: usize, y: usize, size: usize) -> bool {
    let version = (size.saturating_sub(17)) / 4;
    let (centers, count) = alignment_pattern_centers_stack(version);
    is_protected_with_centers(x, y, size, &centers[..count])
}

// ---------------------------------------------------------------------------
// Single-Pass Image Preparation & Cached Default Artwork
// ---------------------------------------------------------------------------

static DEFAULT_CANONICAL_RGBA: OnceLock<RgbaImage> = OnceLock::new();

fn build_default_rgba(target_px: u32) -> RgbaImage {
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

    img
}

fn get_or_build_default_rgba(grid_w: usize) -> RgbaImage {
    let base = DEFAULT_CANONICAL_RGBA
        .get_or_init(|| build_default_rgba(CANONICAL_PRESET_GRID_W as u32));
    if grid_w == CANONICAL_PRESET_GRID_W {
        base.clone()
    } else {
        image::imageops::resize(base, grid_w as u32, grid_w as u32, FilterType::Triangle)
    }
}

/// Center-crops `img` to a square and resizes to `grid_w x grid_w` in a single pass.
fn prepare_square_rgba(img: &DynamicImage, grid_w: usize) -> RgbaImage {
    let (src_w, src_h) = img.dimensions();
    let side = src_w.min(src_h).max(1);
    let crop_x = (src_w.saturating_sub(side)) / 2;
    let crop_y = (src_h.saturating_sub(side)) / 2;
    let cropped = img.crop_imm(crop_x, crop_y, side, side);
    cropped
        .resize_exact(grid_w as u32, grid_w as u32, FilterType::Triangle)
        .to_rgba8()
}

fn extract_photo_rgb_from_rgba(rgba: &RgbaImage, grid_w: usize) -> Vec<(u8, u8, u8)> {
    let mut colors = vec![(15u8, 23u8, 42u8); grid_w * grid_w];
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let ch = pixel.0;
        let alpha = ch[3] as f32 / 255.0;
        let r = (ch[0] as f32 * alpha + 15.0 * (1.0 - alpha)).round() as u8;
        let g = (ch[1] as f32 * alpha + 23.0 * (1.0 - alpha)).round() as u8;
        let b = (ch[2] as f32 * alpha + 42.0 * (1.0 - alpha)).round() as u8;
        colors[(y as usize) * grid_w + (x as usize)] = clamp_dark_rgb((r, g, b));
    }
    colors
}

fn dither_rgba_to_grid(rgba: &RgbaImage, grid_w: usize, params: HalftoneParams) -> Vec<bool> {
    let contrast = params.contrast.clamp(0.2, 3.0);
    let brightness = params.brightness.clamp(-0.8, 0.8);

    let mut luma = vec![1.0f32; grid_w * grid_w];
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let ch = pixel.0;
        let alpha = ch[3] as f32 / 255.0;
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

/// Center-crops, resizes, adjusts contrast/brightness, and dithers a `DynamicImage` to a binary `grid_w x grid_w` grid.
pub fn dither_image_to_grid(
    img: &DynamicImage,
    grid_w: usize,
    params: HalftoneParams,
) -> Vec<bool> {
    let rgba = prepare_square_rgba(img, grid_w);
    dither_rgba_to_grid(&rgba, grid_w, params)
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

/// Fast-path renderer using a precomputed `CompiledHalftonePreset` (skips image decode, resize, and dither!).
pub fn render_halftone_with_preset(
    opts: &QrRenderOptions,
    preset: &CompiledHalftonePreset,
    override_params: Option<HalftoneParams>,
) -> Result<Vec<u8>> {
    let clean_payload = opts.payload.trim();
    if clean_payload.is_empty() {
        bail!("QR payload cannot be empty");
    }

    let qr = QrCode::with_error_correction_level(clean_payload.as_bytes(), EcLevel::H)
        .context("Failed to generate Halftone QR code with Level H ECC")?;
    let n = qr.width();
    let sub_w = n * SUB;

    let ht = override_params.unwrap_or(preset.params);
    render_halftone_matrix(
        opts,
        &qr,
        n,
        sub_w,
        ht,
        &preset.dark_grid,
        preset.rgb_grid.as_deref(),
        preset.grid_w,
    )
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

    // Single-pass prepare_square_rgba or cached default artwork
    let rgba = match opts.logo {
        CenterLogo::ImageLogo(bytes) if !bytes.is_empty() => image::load_from_memory(bytes)
            .map(|img| prepare_square_rgba(&img, sub_w))
            .unwrap_or_else(|_| get_or_build_default_rgba(sub_w)),
        _ => get_or_build_default_rgba(sub_w),
    };

    let dither_grid = dither_rgba_to_grid(&rgba, sub_w, ht);
    let photo_rgb_grid = if ht.color_mode == HalftoneColorMode::PhotoRgb {
        Some(extract_photo_rgb_from_rgba(&rgba, sub_w))
    } else {
        None
    };

    render_halftone_matrix(
        opts,
        &qr,
        n,
        sub_w,
        ht,
        &dither_grid,
        photo_rgb_grid.as_deref(),
        sub_w,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_halftone_matrix(
    opts: &QrRenderOptions,
    qr: &QrCode,
    n: usize,
    sub_w: usize,
    ht: HalftoneParams,
    dither_grid: &[bool],
    photo_rgb_grid: Option<&[(u8, u8, u8)]>,
    src_grid_w: usize,
) -> Result<Vec<u8>> {
    let base_fg = match ht.color_mode {
        HalftoneColorMode::BankBrand => {
            clamp_dark_rgb(qr_center_badge_brand_color(opts.bank_code))
        }
        HalftoneColorMode::Custom | HalftoneColorMode::PhotoRgb => clamp_dark_rgb(ht.fg_color),
    };
    let (bg_r, bg_g, bg_b) = ht.bg_color;

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

    // Precompute alignment pattern centers ONCE on the stack (zero heap allocations in the loop!)
    let version = (n.saturating_sub(17)) / 4;
    let (centers_arr, centers_len) = alignment_pattern_centers_stack(version);
    let centers = &centers_arr[..centers_len];
    let exact_grid = src_grid_w == sub_w;

    for my in 0..n {
        for mx in 0..n {
            let qr_dark = qr[(mx, my)] == Color::Dark;
            let protected = is_protected_with_centers(mx, my, n, centers);

            for sy in 0..SUB {
                let gy = my * SUB + sy;
                let src_y = if exact_grid {
                    gy
                } else {
                    (gy * src_grid_w / sub_w).min(src_grid_w - 1)
                };
                for sx in 0..SUB {
                    let gx = mx * SUB + sx;
                    let src_x = if exact_grid {
                        gx
                    } else {
                        (gx * src_grid_w / sub_w).min(src_grid_w - 1)
                    };
                    let src_idx = src_y * src_grid_w + src_x;

                    let is_dark = if protected || (sx == 1 && sy == 1) {
                        qr_dark
                    } else {
                        dither_grid.get(src_idx).copied().unwrap_or(false)
                    };

                    if is_dark {
                        let (dr, dg, db) = if protected {
                            base_fg
                        } else if let Some(rgb_grid) = photo_rgb_grid {
                            rgb_grid.get(src_idx).copied().unwrap_or(base_fg)
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

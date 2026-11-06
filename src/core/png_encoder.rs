use flate2::Compression;
use flate2::write::ZlibEncoder;
use std::io::Write;

/// Encode pixel buffer into an ultra-lightweight Indexed-Color Palette PNG (`color_type = 3`)
/// with 16-entry `PLTE` chunk (48 bytes), PNG `Up` filter (`filter_type = 2`), and Zlib Level 9.
///
/// Pixel conventions:
/// - `0xFF`: Pure White (Palette Index 0)
/// - `0x00`: Pure Black (Palette Index 1)
/// - `0x01..=0x08`: 8 Bank Brand Color levels blended with white (Palette Index 8..=15)
/// - `0x09..=0xFE`: Grayscale shades for anti-aliased footer text (Palette Index 2..=7)
pub fn encode_branded_qr_png(
    pixels: &[u8],
    width: u32,
    height: u32,
    brand_rgb: (u8, u8, u8),
) -> Vec<u8> {
    let mut out = Vec::with_capacity(4096);

    // PNG signature
    out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // IHDR (bit_depth = 8, color_type = 3 Indexed-color palette)
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth: 8
    ihdr.push(3); // color type: 3 (Indexed-color palette)
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_png_chunk(&mut out, b"IHDR", &ihdr);

    // PLTE — 16 RGB entries (48 bytes total)
    let mut plte = Vec::with_capacity(16 * 3);
    // Index 0: Pure White
    plte.extend_from_slice(&[255, 255, 255]);
    // Index 1: Pure Black
    plte.extend_from_slice(&[0, 0, 0]);
    // Index 2..=7: 6 Grayscale steps for anti-aliased footer text
    for step in 1..=6u8 {
        let g = ((step as u16 * 255) / 7) as u8;
        plte.extend_from_slice(&[g, g, g]);
    }
    // Index 8..=15: 8 Bank Brand Color steps (step 0 = 100% brand color, step 7 = soft tint)
    let (br, bg, bb) = brand_rgb;
    for step in 0..8u16 {
        let alpha = 8 - step;
        let r = ((br as u16 * alpha + 255 * step) / 8) as u8;
        let g = ((bg as u16 * alpha + 255 * step) / 8) as u8;
        let b = ((bb as u16 * alpha + 255 * step) / 8) as u8;
        plte.extend_from_slice(&[r, g, b]);
    }
    write_png_chunk(&mut out, b"PLTE", &plte);

    // IDAT — PNG Up-filtered scanlines + best zlib compression
    let raw_data = build_up_filtered_palette_scanlines(pixels, width as usize, height as usize);
    let compressed = deflate_compress(&raw_data);
    write_png_chunk(&mut out, b"IDAT", &compressed);

    // IEND
    write_png_chunk(&mut out, b"IEND", &[]);

    out
}

/// Encode 24-bit RGB pixel buffer (3 bytes per pixel: [R, G, B, ...]) into Truecolor PNG (`color_type = 2`)
/// with PNG Up filter and Zlib Level 9 compression.
pub fn encode_rgb_png(rgb_pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(width as usize * height as usize + 1024);

    // PNG signature
    out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    // IHDR (bit_depth = 8, color_type = 2 Truecolor RGB)
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth: 8
    ihdr.push(2); // color type: 2 (Truecolor RGB)
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_png_chunk(&mut out, b"IHDR", &ihdr);

    // IDAT — Filter scanlines (Up filter type 2) + best zlib compression
    let row_bytes = width as usize * 3;
    let mut raw_data = Vec::with_capacity(height as usize * (1 + row_bytes));
    for y in 0..height as usize {
        let curr_row = &rgb_pixels[y * row_bytes..(y + 1) * row_bytes];
        if y == 0 {
            raw_data.push(0); // Filter: None
            raw_data.extend_from_slice(curr_row);
        } else {
            raw_data.push(2); // Filter: Up
            let prev_row = &rgb_pixels[(y - 1) * row_bytes..y * row_bytes];
            for i in 0..row_bytes {
                raw_data.push(curr_row[i].wrapping_sub(prev_row[i]));
            }
        }
    }
    let compressed = deflate_compress(&raw_data);
    write_png_chunk(&mut out, b"IDAT", &compressed);

    // IEND
    write_png_chunk(&mut out, b"IEND", &[]);

    out
}

#[inline]
fn map_pixel_to_palette_index(p: u8) -> u8 {
    match p {
        0xFF => 0,
        0x00 => 1,
        0x01..=0x08 => 8 + (p - 1),
        gray => {
            let bucket = ((gray as u16 * 6) / 256).min(5) as u8;
            2 + bucket
        }
    }
}

/// Build PNG scanlines using Filter Type 2 (`Up`) so repeated vertical QR module rows become `0x00` runs.
fn build_up_filtered_palette_scanlines(pixels: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(height * (1 + width));
    let mut prev_row = vec![0u8; width];
    let mut curr_row = vec![0u8; width];

    for y in 0..height {
        let row_slice = &pixels[y * width..(y + 1) * width];
        for x in 0..width {
            curr_row[x] = map_pixel_to_palette_index(row_slice[x]);
        }
        if y == 0 {
            data.push(0); // Filter: None on first row
            data.extend_from_slice(&curr_row);
        } else {
            data.push(2); // Filter: Up (curr[x] - prev[x])
            for x in 0..width {
                data.push(curr_row[x].wrapping_sub(prev_row[x]));
            }
        }
        prev_row.copy_from_slice(&curr_row);
    }
    data
}

fn deflate_compress(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    if encoder.write_all(data).is_ok() {
        if let Ok(compressed) = encoder.finish() {
            return compressed;
        }
    }
    deflate_store(data)
}

fn deflate_store(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&[0x78, 0x01]);
    let chunks = data.chunks(65535);
    let total = chunks.len();
    for (i, chunk) in data.chunks(65535).enumerate() {
        let is_last = i + 1 == total;
        out.push(if is_last { 0x01 } else { 0x00 });
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    let checksum = adler32(data);
    out.extend_from_slice(&checksum.to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn write_png_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    let len = data.len() as u32;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    let mut crc_data = Vec::with_capacity(4 + data.len());
    crc_data.extend_from_slice(chunk_type);
    crc_data.extend_from_slice(data);
    let crc = crc32(&crc_data);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

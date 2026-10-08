//! Turn an upload into new WebP bytes.
//!
//! The format comes from the file header. The declared type is only a check
//! that the caller and the bytes agree. Original bytes are not returned.

use std::io::Cursor;

use image::codecs::webp::WebPEncoder;
use image::{
    DynamicImage, ExtendedColorType, GenericImageView, ImageDecoder, ImageFormat, ImageReader,
    Limits,
};

use super::{
    DeclaredFormat, FULL_LONGEST_EDGE, MAX_PIXELS, MAX_UPLOAD_BYTES, MediaError, THUMB_LONGEST_EDGE,
};

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

#[derive(Debug)]
pub(crate) struct NormalizedImage {
    pub width: u32,
    pub height: u32,
    pub full: Vec<u8>,
    pub thumb: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ImageKind {
    Jpeg,
    Png,
    WebP,
}

/// Read at most [`MAX_UPLOAD_BYTES`].
///
/// A declared length above the cap returns [`MediaError::TooManyBytes`] before
/// the reader is polled. A stream that grows past the cap returns the same
/// error before the image is decoded.
pub(crate) async fn read_capped<R>(
    reader: &mut R,
    length: super::UploadLength,
) -> Result<Vec<u8>, MediaError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    if let super::UploadLength::Declared(size) = length {
        if size > MAX_UPLOAD_BYTES {
            return Err(MediaError::TooManyBytes);
        }
    }
    let mut body = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let read = tokio::io::AsyncReadExt::read(reader, &mut chunk)
            .await
            .map_err(|_| MediaError::Store)?;
        if read == 0 {
            break;
        }
        let Some(next) = body.len().checked_add(read) else {
            return Err(MediaError::TooManyBytes);
        };
        if next > usize::try_from(MAX_UPLOAD_BYTES).unwrap_or(usize::MAX) {
            return Err(MediaError::TooManyBytes);
        }
        body.extend_from_slice(&chunk[..read]);
    }
    if body.is_empty() {
        return Err(MediaError::Unreadable);
    }
    Ok(body)
}

pub(crate) fn normalize_image(
    bytes: &[u8],
    declared: DeclaredFormat,
) -> Result<NormalizedImage, MediaError> {
    let kind = agreed_kind(bytes, declared)?;
    reject_animation(kind, bytes)?;
    reject_header_pixels(kind, bytes)?;
    let oriented = decode_oriented(kind, bytes)?;
    let (width, height) = oriented.dimensions();
    if width == 0 || height == 0 || exceeds_pixel_budget(width, height) {
        return Err(MediaError::TooManyPixels);
    }
    let full = fit_longest_edge(oriented, FULL_LONGEST_EDGE);
    let thumb = fit_longest_edge(full.clone(), THUMB_LONGEST_EDGE);
    let (width, height) = full.dimensions();
    Ok(NormalizedImage {
        width,
        height,
        full: encode_webp(&full)?,
        thumb: encode_webp(&thumb)?,
    })
}

pub(crate) fn exceeds_pixel_budget(width: u32, height: u32) -> bool {
    u64::from(width).saturating_mul(u64::from(height)) > MAX_PIXELS
}

fn agreed_kind(bytes: &[u8], declared: DeclaredFormat) -> Result<ImageKind, MediaError> {
    let sniffed = sniff(bytes).ok_or(MediaError::Unreadable)?;
    match declared {
        DeclaredFormat::Omitted => Ok(sniffed),
        DeclaredFormat::Unsupported => Err(MediaError::TypeMismatch),
        DeclaredFormat::Jpeg => expect_kind(sniffed, ImageKind::Jpeg),
        DeclaredFormat::Png => expect_kind(sniffed, ImageKind::Png),
        DeclaredFormat::WebP => expect_kind(sniffed, ImageKind::WebP),
    }
}

fn expect_kind(sniffed: ImageKind, declared: ImageKind) -> Result<ImageKind, MediaError> {
    if sniffed == declared {
        Ok(sniffed)
    } else {
        Err(MediaError::TypeMismatch)
    }
}

fn sniff(bytes: &[u8]) -> Option<ImageKind> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(ImageKind::Jpeg);
    }
    if bytes.starts_with(PNG_MAGIC) {
        return Some(ImageKind::Png);
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some(ImageKind::WebP);
    }
    None
}

fn reject_animation(kind: ImageKind, bytes: &[u8]) -> Result<(), MediaError> {
    let animated = match kind {
        ImageKind::Jpeg => false,
        ImageKind::Png => png_is_animated(bytes),
        ImageKind::WebP => webp_is_animated(bytes),
    };
    if animated {
        Err(MediaError::Animated)
    } else {
        Ok(())
    }
}

fn reject_header_pixels(kind: ImageKind, bytes: &[u8]) -> Result<(), MediaError> {
    let Some((width, height)) = header_size(kind, bytes) else {
        return Ok(());
    };
    if width == 0 || height == 0 {
        return Err(MediaError::Unreadable);
    }
    if exceeds_pixel_budget(width, height) {
        return Err(MediaError::TooManyPixels);
    }
    Ok(())
}

fn header_size(kind: ImageKind, bytes: &[u8]) -> Option<(u32, u32)> {
    match kind {
        ImageKind::Jpeg => jpeg_size(bytes),
        ImageKind::Png => png_size(bytes),
        ImageKind::WebP => None,
    }
}

fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || !bytes.starts_with(PNG_MAGIC) || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = be_u32(&bytes[16..20])?;
    let height = be_u32(&bytes[20..24])?;
    Some((width, height))
}

fn jpeg_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut index = 2;
    let mut markers = 0;
    while index + 4 < bytes.len() && markers < 64 {
        if bytes[index] != 0xFF {
            return None;
        }
        while index < bytes.len() && bytes[index] == 0xFF {
            index += 1;
        }
        let marker = *bytes.get(index)?;
        index += 1;
        if marker == 0xD9 || marker == 0xDA || (0xD0..=0xD7).contains(&marker) {
            return None;
        }
        let len = usize::from(u16::from_be_bytes([
            *bytes.get(index)?,
            *bytes.get(index + 1)?,
        ]));
        if len < 2 || index + len > bytes.len() {
            return None;
        }
        if matches!(marker, 0xC0 | 0xC1 | 0xC2) {
            let height = u32::from(u16::from_be_bytes([
                *bytes.get(index + 3)?,
                *bytes.get(index + 4)?,
            ]));
            let width = u32::from(u16::from_be_bytes([
                *bytes.get(index + 5)?,
                *bytes.get(index + 6)?,
            ]));
            return Some((width, height));
        }
        index += len;
        markers += 1;
    }
    None
}

fn png_is_animated(bytes: &[u8]) -> bool {
    if !bytes.starts_with(PNG_MAGIC) {
        return false;
    }
    let mut index = 8;
    let mut chunks = 0;
    while index + 8 <= bytes.len() && chunks < 64 {
        let Some(len) = be_u32(&bytes[index..index + 4]) else {
            return false;
        };
        let kind = &bytes[index + 4..index + 8];
        if kind == b"acTL" {
            return true;
        }
        if kind == b"IDAT" || kind == b"IEND" {
            return false;
        }
        let next = index.saturating_add(12).saturating_add(len as usize);
        if next <= index || next > bytes.len() {
            return false;
        }
        index = next;
        chunks += 1;
    }
    false
}

fn webp_is_animated(bytes: &[u8]) -> bool {
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return false;
    }
    let mut index = 12;
    let mut chunks = 0;
    while index + 8 <= bytes.len() && chunks < 64 {
        let kind = &bytes[index..index + 4];
        let Some(size) = le_u32(&bytes[index + 4..index + 8]) else {
            return false;
        };
        if kind == b"ANIM" {
            return true;
        }
        if kind == b"VP8X" {
            return bytes.get(index + 8).is_some_and(|flag| flag & 0x02 != 0);
        }
        let next = index.saturating_add(8).saturating_add(size as usize);
        let next = next + ((size as usize) & 1);
        if next <= index {
            return false;
        }
        index = next;
        chunks += 1;
    }
    false
}

fn be_u32(bytes: &[u8]) -> Option<u32> {
    let chunk = bytes.get(..4)?.try_into().ok()?;
    Some(u32::from_be_bytes(chunk))
}

fn le_u32(bytes: &[u8]) -> Option<u32> {
    let chunk = bytes.get(..4)?.try_into().ok()?;
    Some(u32::from_le_bytes(chunk))
}

fn decode_oriented(kind: ImageKind, bytes: &[u8]) -> Result<DynamicImage, MediaError> {
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(image_format(kind));
    reader.limits(decode_limits());
    let mut decoder = reader.into_decoder().map_err(image_error)?;
    let (width, height) = decoder.dimensions();
    if width == 0 || height == 0 || exceeds_pixel_budget(width, height) {
        return Err(MediaError::TooManyPixels);
    }
    if decoder.total_bytes() > MAX_PIXELS.saturating_mul(4) {
        return Err(MediaError::TooManyPixels);
    }
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder).map_err(image_error)?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn image_format(kind: ImageKind) -> ImageFormat {
    match kind {
        ImageKind::Jpeg => ImageFormat::Jpeg,
        ImageKind::Png => ImageFormat::Png,
        ImageKind::WebP => ImageFormat::WebP,
    }
}

fn decode_limits() -> Limits {
    let mut limits = Limits::default();
    let edge = u32::try_from(MAX_PIXELS).unwrap_or(u32::MAX);
    limits.max_image_width = Some(edge);
    limits.max_image_height = Some(edge);
    limits.max_alloc = Some(MAX_PIXELS.saturating_mul(4));
    limits
}

fn image_error(error: image::ImageError) -> MediaError {
    match error {
        image::ImageError::Limits(_) => MediaError::TooManyPixels,
        _ => MediaError::Unreadable,
    }
}

fn fit_longest_edge(image: DynamicImage, edge: u32) -> DynamicImage {
    let (width, height) = image.dimensions();
    let longest = width.max(height);
    if longest <= edge || longest == 0 {
        return image;
    }
    image.resize(
        scaled_edge(width, longest, edge),
        scaled_edge(height, longest, edge),
        image::imageops::FilterType::Triangle,
    )
}

fn scaled_edge(side: u32, longest: u32, edge: u32) -> u32 {
    let scaled = u64::from(side) * u64::from(edge) / u64::from(longest);
    u32::try_from(scaled).unwrap_or(1).max(1)
}

fn encode_webp(image: &DynamicImage) -> Result<Vec<u8>, MediaError> {
    let rgba = image.to_rgba8();
    let mut buffer = Vec::new();
    WebPEncoder::new_lossless(&mut buffer)
        .encode(
            rgba.as_raw(),
            rgba.width(),
            rgba.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|_| MediaError::Unreadable)?;
    if buffer.is_empty() {
        return Err(MediaError::Unreadable);
    }
    Ok(buffer)
}

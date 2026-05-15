use image::RgbImage;
use libheif_rs::{
    Channel, ColorSpace, CompressionFormat, EncoderQuality, HeifContext, Image, LibHeif, RgbChroma,
};
use shared::error::AppError;

pub fn write_spatial_heic(
    left: &RgbImage,
    right: &RgbImage,
    _baseline: f32,
    _fov: f32,
) -> Result<Vec<u8>, AppError> {
    let lib_heif = LibHeif::new();
    let mut context = HeifContext::new()
        .map_err(|e| AppError::Internal(format!("create heif context: {e}")))?;

    let mut encoder = lib_heif
        .encoder_for_format(CompressionFormat::Hevc)
        .map_err(|e| AppError::Internal(format!("get hevc encoder: {e}")))?;

    encoder
        .set_quality(EncoderQuality::Lossy(92))
        .map_err(|e| AppError::Internal(format!("set encoder quality: {e}")))?;

    // Encode left eye first — libheif assigns item_id=1 (becomes primary)
    let left_heif = rgb_to_heif(left)?;
    context
        .encode_image(&left_heif, &mut encoder, None)
        .map_err(|e| AppError::Internal(format!("encode left image: {e}")))?;

    let right_heif = rgb_to_heif(right)?;
    context
        .encode_image(&right_heif, &mut encoder, None)
        .map_err(|e| AppError::Internal(format!("encode right image: {e}")))?;

    let heif_bytes = context
        .write_to_bytes()
        .map_err(|e| AppError::Internal(format!("write heif to bytes: {e}")))?;

    // Post-process: add mvis brand + cdsc item reference for Apple spatial
    patch_for_spatial(heif_bytes)
}

/// Patches a libheif-produced HEIF file to be recognised as an Apple spatial photo:
///   - Adds `mvis` to ftyp compatible brands
///   - Inserts an iref/cdsc box: item 2 (right eye) content-describes item 1 (left eye)
///   - Updates iloc base_offsets to account for the bytes inserted before mdat
fn patch_for_spatial(data: Vec<u8>) -> Result<Vec<u8>, AppError> {
    // Track total bytes inserted before mdat so iloc can be corrected.
    let mvis_shift = 4usize;
    let iref_shift = build_iref_cdsc().len();
    let total_shift = mvis_shift + iref_shift;

    let data = add_mvis_brand(data)?;
    let data = add_cdsc_iref(data)?;
    let mut data = data;
    fix_iloc_offsets(&mut data, total_shift)?;
    Ok(data)
}

fn add_mvis_brand(mut data: Vec<u8>) -> Result<Vec<u8>, AppError> {
    if data.len() < 16 {
        return Err(AppError::Internal("heif too short".into()));
    }
    if &data[4..8] != b"ftyp" {
        return Err(AppError::Internal("expected ftyp as first box".into()));
    }

    let old_size = u32_be(&data, 0) as usize;

    let new_size = old_size + 4;
    data[0..4].copy_from_slice(&(new_size as u32).to_be_bytes());

    let mut out = Vec::with_capacity(data.len() + 4);
    out.extend_from_slice(&data[..old_size]);
    out.extend_from_slice(b"mvis");
    out.extend_from_slice(&data[old_size..]);
    Ok(out)
}

/// Builds the bytes for:
///   iref (FullBox, version=0)
///     cdsc from=2 to=[1]
fn build_iref_cdsc() -> Vec<u8> {
    // inner cdsc reference box (plain Box, not FullBox)
    // size(4) + 'cdsc'(4) + from_id(2) + ref_count(2) + to_id(2) = 14 bytes
    let mut cdsc = Vec::with_capacity(14);
    cdsc.extend_from_slice(&14u32.to_be_bytes()); // size
    cdsc.extend_from_slice(b"cdsc");
    cdsc.extend_from_slice(&2u16.to_be_bytes()); // from_item_id = right eye
    cdsc.extend_from_slice(&1u16.to_be_bytes()); // reference_count
    cdsc.extend_from_slice(&1u16.to_be_bytes()); // to_item_id = left eye

    // iref FullBox: size(4) + 'iref'(4) + version(1) + flags(3) + cdsc
    let iref_size = 4 + 4 + 1 + 3 + cdsc.len();
    let mut iref = Vec::with_capacity(iref_size);
    iref.extend_from_slice(&(iref_size as u32).to_be_bytes());
    iref.extend_from_slice(b"iref");
    iref.extend_from_slice(&[0u8, 0, 0, 0]); // version=0, flags=0
    iref.extend_from_slice(&cdsc);
    iref
}

/// Finds the meta box and appends an iref/cdsc child, updating meta's size.
fn add_cdsc_iref(data: Vec<u8>) -> Result<Vec<u8>, AppError> {
    let iref = build_iref_cdsc();

    let meta_start = find_box(&data, 0, b"meta")
        .ok_or_else(|| AppError::Internal("meta box not found in heif".into()))?;

    let meta_old_size = u32_be(&data, meta_start) as usize;
    let meta_new_size = meta_old_size + iref.len();

    let mut out = Vec::with_capacity(data.len() + iref.len());
    out.extend_from_slice(&data[..meta_start]);
    out.extend_from_slice(&(meta_new_size as u32).to_be_bytes());
    out.extend_from_slice(&data[meta_start + 4..meta_start + meta_old_size]);
    out.extend_from_slice(&iref);
    out.extend_from_slice(&data[meta_start + meta_old_size..]);
    Ok(out)
}

fn u32_be(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap())
}

fn u16_be(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes(data[offset..offset + 2].try_into().unwrap())
}

fn find_box(data: &[u8], start: usize, box_type: &[u8; 4]) -> Option<usize> {
    let mut offset = start;
    while offset + 8 <= data.len() {
        let size = u32_be(data, offset) as usize;
        if size < 8 {
            break;
        }
        if &data[offset + 4..offset + 8] == box_type {
            return Some(offset);
        }
        offset += size;
    }
    None
}

fn find_box_in(data: &[u8], start: usize, end: usize, box_type: &[u8; 4]) -> Option<usize> {
    let mut offset = start;
    while offset + 8 <= end.min(data.len()) {
        let size = u32_be(data, offset) as usize;
        if size < 8 {
            break;
        }
        if &data[offset + 4..offset + 8] == box_type {
            return Some(offset);
        }
        offset += size;
    }
    None
}

fn add_be_offset(data: &mut [u8], pos: usize, size: usize, shift: usize) -> Result<(), AppError> {
    match size {
        4 => {
            let old = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap());
            let new_val = old
                .checked_add(shift as u32)
                .ok_or_else(|| AppError::Internal("iloc offset overflow".into()))?;
            data[pos..pos + 4].copy_from_slice(&new_val.to_be_bytes());
        }
        8 => {
            let old = u64::from_be_bytes(data[pos..pos + 8].try_into().unwrap());
            let new_val = old
                .checked_add(shift as u64)
                .ok_or_else(|| AppError::Internal("iloc offset overflow".into()))?;
            data[pos..pos + 8].copy_from_slice(&new_val.to_be_bytes());
        }
        _ => {}
    }
    Ok(())
}

/// After inserting `shift` bytes before mdat, the iloc item base_offsets (which are
/// absolute file offsets) are stale by exactly `shift`. This walks the iloc box and
/// increments every base_offset (or, if base_offset_size==0, every extent_offset).
fn fix_iloc_offsets(data: &mut Vec<u8>, shift: usize) -> Result<(), AppError> {
    // iloc lives inside meta (a FullBox), so skip the 12-byte FullBox header when
    // searching meta's children.
    let meta_start = find_box(data, 0, b"meta")
        .ok_or_else(|| AppError::Internal("meta box not found".into()))?;
    let meta_size = u32_be(data, meta_start) as usize;

    let iloc_start = find_box_in(data, meta_start + 12, meta_start + meta_size, b"iloc")
        .ok_or_else(|| AppError::Internal("iloc box not found".into()))?;

    // iloc FullBox: size(4) + 'iloc'(4) + version(1) + flags(3) = 12 bytes header
    let version = data[iloc_start + 8];

    let sizes_byte = data[iloc_start + 12];
    let offset_size = (sizes_byte >> 4) as usize;   // extent_offset field width
    let length_size = (sizes_byte & 0x0f) as usize; // extent_length field width

    let bos_byte = data[iloc_start + 13];
    let base_offset_size = (bos_byte >> 4) as usize;
    let index_size = if version >= 1 { (bos_byte & 0x0f) as usize } else { 0 };

    let item_count_size = if version < 2 { 2 } else { 4 };
    let item_count = if version < 2 {
        u16_be(data, iloc_start + 14) as usize
    } else {
        u32_be(data, iloc_start + 14) as usize
    };

    let item_id_size = if version < 2 { 2 } else { 4 };
    let mut pos = iloc_start + 14 + item_count_size; // points to first item

    for _ in 0..item_count {
        pos += item_id_size;
        if version >= 1 {
            pos += 2; // construction_method (v1/v2 only)
        }
        pos += 2; // data_reference_index

        // base_offset: absolute file offset used as base for all extents of this item
        if base_offset_size > 0 {
            add_be_offset(data, pos, base_offset_size, shift)?;
        }
        pos += base_offset_size;

        let extent_count = u16_be(data, pos) as usize;
        pos += 2;

        for _ in 0..extent_count {
            if index_size > 0 {
                pos += index_size; // extent_index (v1/v2)
            }
            // When base_offset_size==0, extent_offset is an absolute file offset.
            if base_offset_size == 0 && offset_size > 0 {
                add_be_offset(data, pos, offset_size, shift)?;
            }
            pos += offset_size;
            pos += length_size;
        }
    }

    Ok(())
}

fn rgb_to_heif(img: &RgbImage) -> Result<Image, AppError> {
    let (width, height) = img.dimensions();
    let mut heif_img = Image::new(width, height, ColorSpace::Rgb(RgbChroma::Rgb))
        .map_err(|e| AppError::Internal(format!("create heif image: {e}")))?;

    heif_img
        .create_plane(Channel::Interleaved, width, height, 24)
        .map_err(|e| AppError::Internal(format!("create heif plane: {e}")))?;

    let planes = heif_img.planes_mut();
    let plane = planes
        .interleaved
        .ok_or_else(|| AppError::Internal("failed to get heif plane".into()))?;

    let stride = plane.stride;
    let data = plane.data;

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            let off = y as usize * stride + x as usize * 3;
            data[off] = pixel[0];
            data[off + 1] = pixel[1];
            data[off + 2] = pixel[2];
        }
    }

    Ok(heif_img)
}

use libheif_rs::{
    Channel, ColorSpace, CompressionFormat, EncoderQuality, HeifContext, Image, LibHeif, RgbChroma,
};
use shared::error::AppError;
use image::RgbImage;

pub fn write_spatial_heic(
    left: &RgbImage,
    right: &RgbImage,
    _baseline: f32,
    _fov: f32,
) -> Result<Vec<u8>, AppError> {
    let lib_heif = LibHeif::new();
    let mut context = HeifContext::new()
        .map_err(|e| AppError::Internal(format!("create heif context: {e}")))?;

    let mut encoder = lib_heif.encoder_for_format(CompressionFormat::Hevc)
        .map_err(|e| AppError::Internal(format!("get hevc encoder: {e}")))?;
    
    encoder.set_quality(EncoderQuality::LossLess)
        .map_err(|e| AppError::Internal(format!("set encoder quality: {e}")))?;

    let left_heif = rgb_to_heif(left)?;
    context.encode_image(&left_heif, &mut encoder, None)
        .map_err(|e| AppError::Internal(format!("encode left image: {e}")))?;

    let right_heif = rgb_to_heif(right)?;
    context.encode_image(&right_heif, &mut encoder, None)
        .map_err(|e| AppError::Internal(format!("encode right image: {e}")))?;

    let result = context.write_to_bytes()
        .map_err(|e| AppError::Internal(format!("write heif to bytes: {e}")))?;

    Ok(result)
}

fn rgb_to_heif(img: &RgbImage) -> Result<Image, AppError> {
    let (width, height) = img.dimensions();
    let mut heif_img = Image::new(width, height, ColorSpace::Rgb(RgbChroma::Rgb))
        .map_err(|e| AppError::Internal(format!("create heif image: {e}")))?;

    heif_img.create_plane(Channel::Interleaved, width, height, 24)
        .map_err(|e| AppError::Internal(format!("create heif plane: {e}")))?;

    let planes = heif_img.planes_mut();
    let plane = planes.interleaved
        .ok_or_else(|| AppError::Internal("failed to get heif plane".to_string()))?;
    
    let stride = plane.stride;
    let data = plane.data;

    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            let offset = (y as usize * stride) + (x as usize * 3);
            data[offset] = pixel[0];
            data[offset + 1] = pixel[1];
            data[offset + 2] = pixel[2];
        }
    }

    Ok(heif_img)
}

use image::{DynamicImage, ImageBuffer, Luma, Rgb, RgbImage};
use ndarray::Array4;
use ort::{inputs, session::Session, value::Tensor};
use shared::error::AppError;
use std::sync::Mutex;

use crate::depth::DepthMap;

fn pixel_mean(img: &RgbImage) -> f32 {
    let sum: f32 = img.pixels().map(|p| p[0] as f32).sum();
    sum / img.pixels().count() as f32
}

/// Simple two-pass scanline hole fill.  Holes (mask == 255) are filled with
/// the nearest non-hole neighbour on the same row.  Works well for the
/// thin disocclusion strips produced by DIBR.
fn fill_holes_scanline(image: &mut RgbImage, mask: &ImageBuffer<Luma<u8>, Vec<u8>>) {
    let (width, height) = image.dimensions();
    for y in 0..height {
        let mut last = image::Rgb([0u8, 0, 0]);
        for x in 0..width {
            if mask.get_pixel(x, y)[0] > 128 {
                image.put_pixel(x, y, last);
            } else {
                last = *image.get_pixel(x, y);
            }
        }
        let mut last = image::Rgb([0u8, 0, 0]);
        for x in (0..width).rev() {
            if mask.get_pixel(x, y)[0] > 128 {
                image.put_pixel(x, y, last);
            } else {
                last = *image.get_pixel(x, y);
            }
        }
    }
}

pub struct StereoGenerator {
    lama_session: Mutex<Session>,
}

impl StereoGenerator {
    pub fn load(lama_model_path: &str) -> Result<Self, AppError> {
        let lama_session = Session::builder()
            .map_err(|e| AppError::Internal(format!("ort builder: {e}")))?
            .commit_from_file(lama_model_path)
            .map_err(|e| AppError::Internal(format!("load lama model '{lama_model_path}': {e}")))?;

        Ok(Self {
            lama_session: Mutex::new(lama_session),
        })
    }

    /// Generates a stereo pair (left, right) from a mono image and depth map.
    /// Uses DIBR for initial warping, then LaMa (with scanline fallback) for holes.
    pub fn generate_stereo(
        &self,
        image: &DynamicImage,
        depth: &DepthMap,
        max_disparity: f32,
    ) -> Result<(RgbImage, RgbImage), AppError> {
        let left = image.to_rgb8();

        let (mut right_warped, mask) = self.warp_dibr(&left, depth, max_disparity);

        let hole_count = mask.pixels().filter(|p| p[0] > 128).count();
        let warped_mean = pixel_mean(&right_warped);
        tracing::info!(warped_mean, hole_count, width = left.width(), height = left.height(), "DIBR stats");

        if hole_count == 0 {
            tracing::info!("no holes — skipping inpainting");
            return Ok((left, right_warped));
        }

        match self.inpaint(&mut right_warped, &mask) {
            Ok(()) => {
                let after_mean = pixel_mean(&right_warped);
                tracing::info!(warped_mean, after_mean, "after LaMa inpaint");
                let bad = after_mean < 2.0 || after_mean > 250.0 || (after_mean - warped_mean).abs() > 100.0;
                if bad {
                    tracing::warn!(warped_mean, after_mean, "LaMa output looks degenerate — using scanline fill instead");
                    let (mut fallback, _) = self.warp_dibr(&left, depth, max_disparity);
                    fill_holes_scanline(&mut fallback, &mask);
                    right_warped = fallback;
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "LaMa inpaint failed — using scanline fill");
                fill_holes_scanline(&mut right_warped, &mask);
            }
        }

        let final_mean = pixel_mean(&right_warped);
        tracing::info!(final_mean, "final right eye mean");
        Ok((left, right_warped))
    }

    fn warp_dibr(
        &self,
        left: &RgbImage,
        depth: &DepthMap,
        max_disparity: f32,
    ) -> (RgbImage, ImageBuffer<Luma<u8>, Vec<u8>>) {
        let (width, height) = left.dimensions();
        let mut right = RgbImage::new(width, height);
        let mut mask = ImageBuffer::<Luma<u8>, Vec<u8>>::new(width, height);

        for p in mask.pixels_mut() {
            *p = Luma([255]);
        }

        let mut z_buffer = vec![f32::MIN; (width * height) as usize];

        for y in 0..height {
            for x in 0..width {
                let d = depth.data[(y * width + x) as usize];
                let disparity = d * max_disparity;
                let nx = x as f32 - disparity;

                if nx >= 0.0 && nx < width as f32 {
                    let nx_int = nx.round() as u32;
                    let idx = (y * width + nx_int) as usize;
                    if d > z_buffer[idx] {
                        right.put_pixel(nx_int, y, *left.get_pixel(x, y));
                        mask.put_pixel(nx_int, y, Luma([0]));
                        z_buffer[idx] = d;
                    }
                }
            }
        }

        (right, mask)
    }

    fn inpaint(
        &self,
        image: &mut RgbImage,
        mask: &ImageBuffer<Luma<u8>, Vec<u8>>,
    ) -> Result<(), AppError> {
        let pre_mean = pixel_mean(image);
        tracing::debug!(pre_mean, "inpaint: image mean before LaMa");
        let (orig_w, orig_h) = image.dimensions();
        let target_sz = 512;

        let resized_img = image::DynamicImage::ImageRgb8(image.clone()).resize_exact(
            target_sz,
            target_sz,
            image::imageops::FilterType::Triangle,
        );
        let resized_mask = image::DynamicImage::ImageLuma8(mask.clone()).resize_exact(
            target_sz,
            target_sz,
            image::imageops::FilterType::Triangle,
        );

        let img_rgb = resized_img.to_rgb8();
        let mask_luma = resized_mask.to_luma8();

        let mut img_tensor = Array4::<f32>::zeros([1, 3, target_sz as usize, target_sz as usize]);
        let mut mask_tensor = Array4::<f32>::zeros([1, 1, target_sz as usize, target_sz as usize]);

        for y in 0..target_sz {
            for x in 0..target_sz {
                let p = img_rgb.get_pixel(x, y);
                img_tensor[[0, 0, y as usize, x as usize]] = f32::from(p[0]) / 255.0;
                img_tensor[[0, 1, y as usize, x as usize]] = f32::from(p[1]) / 255.0;
                img_tensor[[0, 2, y as usize, x as usize]] = f32::from(p[2]) / 255.0;

                let m = mask_luma.get_pixel(x, y);
                mask_tensor[[0, 0, y as usize, x as usize]] = if m[0] > 128 { 1.0 } else { 0.0 };
            }
        }

        let ort_img = Tensor::<f32>::from_array(img_tensor)
            .map_err(|e| AppError::Internal(format!("build ort image tensor: {e}")))?;
        let ort_mask = Tensor::<f32>::from_array(mask_tensor)
            .map_err(|e| AppError::Internal(format!("build ort mask tensor: {e}")))?;

        let mut session = self
            .lama_session
            .lock()
            .map_err(|_| AppError::Internal("lama session lock failed".to_string()))?;
        let outputs = session
            .run(inputs!["image" => ort_img, "mask" => ort_mask])
            .map_err(|e| AppError::Internal(format!("lama inference: {e}")))?;

        let raw = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| AppError::Internal(format!("extract lama tensor: {e}")))?;

        let (shape, data) = raw;
        tracing::info!(
            ndim = shape.len(),
            d0 = shape.get(0).copied().unwrap_or(0),
            d1 = shape.get(1).copied().unwrap_or(0),
            d2 = shape.get(2).copied().unwrap_or(0),
            d3 = shape.get(3).copied().unwrap_or(0),
            data_len = data.len(),
            "LaMa raw output shape"
        );

        if shape.len() < 4 {
            return Err(AppError::Internal(format!(
                "LaMa output has {} dims, expected 4",
                shape.len()
            )));
        }

        // LaMa output is [1, 3, H, W]
        let h = shape[2] as usize;
        let w = shape[3] as usize;
        let plane_size = h * w;

        let (data_min, data_max) = data
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        let data_mean = data.iter().sum::<f32>() / data.len().max(1) as f32;
        tracing::info!(data_min, data_max, data_mean, "LaMa output value stats");

        // If values are outside [0,1] (e.g. model outputs [0,255] or arbitrary scale),
        // normalise by the observed range so the result is always [0,1].
        let out_of_range = data_max > 1.5 || data_min < -0.5;
        if out_of_range {
            tracing::warn!(data_min, data_max, "LaMa output outside [0,1] — normalising");
        }
        let norm_range = (data_max - data_min).max(1e-6);
        let scale = move |v: f32| -> u8 {
            if out_of_range {
                ((v - data_min) / norm_range * 255.0).clamp(0.0, 255.0) as u8
            } else {
                (v.clamp(0.0, 1.0) * 255.0) as u8
            }
        };

        // Composite: only replace hole pixels with LaMa output; keep known pixels from
        // the warped image unchanged (avoids LaMa colour drift on non-occluded areas).
        let inpainted_resized = RgbImage::from_fn(target_sz, target_sz, |x, y| {
            let is_hole = mask_luma.get_pixel(x, y)[0] > 128;
            if !is_hole {
                return *img_rgb.get_pixel(x, y);
            }
            let offset = y as usize * w + x as usize;
            Rgb([
                scale(data[offset]),
                scale(data[plane_size + offset]),
                scale(data[2 * plane_size + offset]),
            ])
        });

        let final_img = image::DynamicImage::ImageRgb8(inpainted_resized)
            .resize_exact(orig_w, orig_h, image::imageops::FilterType::Triangle)
            .to_rgb8();

        *image = final_img;
        Ok(())
    }
}

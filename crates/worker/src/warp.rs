use image::{DynamicImage, GenericImageView, ImageBuffer, Luma, Rgb, RgbImage};
use ndarray::Array4;
use ort::{inputs, session::Session, value::Tensor};
use shared::error::AppError;

use crate::depth::DepthMap;

use std::sync::Mutex;

pub struct StereoGenerator {
    lama_session: Mutex<Session>,
}

impl StereoGenerator {
    pub fn load(lama_model_path: &str) -> Result<Self, AppError> {
        let lama_session = Session::builder()
            .map_err(|e| AppError::Internal(format!("ort builder: {e}")))?
            .commit_from_file(lama_model_path)
            .map_err(|e| AppError::Internal(format!("load lama model '{lama_model_path}': {e}")))?;

        Ok(Self { lama_session: Mutex::new(lama_session) })
    }

    /// Generates a stereo pair (left, right) from a mono image and depth map.
    /// Uses DIBR for initial warping and LaMa for inpainting holes.
    pub fn generate_stereo(
        &self,
        image: &DynamicImage,
        depth: &DepthMap,
        max_disparity: f32,
    ) -> Result<(RgbImage, RgbImage), AppError> {
        let left = image.to_rgb8();
        
        // 1. Warp right image using DIBR
        let (mut right_warped, mask) = self.warp_dibr(&left, depth, max_disparity);

        // 2. Inpaint holes in right image using LaMa
        self.inpaint(&mut right_warped, &mask)?;

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
        
        // Initialise mask with 255 (hole)
        for p in mask.pixels_mut() {
            *p = Luma([255]);
        }

        // DIBR: shift pixels to the right for the right eye
        // disparity = depth * max_disparity
        // We iterate and place pixels, keeping track of depth to handle occlusions (z-buffer)
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

    fn inpaint(&self, image: &mut RgbImage, mask: &ImageBuffer<Luma<u8>, Vec<u8>>) -> Result<(), AppError> {
        // LaMa expects input to be multiple of 8 or 16, typically 512x512 or similar.
        // For simplicity, we'll resize to 512x512, inpaint, then resize back.
        // A better approach would be tiling or using the original resolution if the model supports it.
        
        let (orig_w, orig_h) = image.dimensions();
        let target_sz = 512;

        let resized_img = image::DynamicImage::ImageRgb8(image.clone())
            .resize_exact(target_sz, target_sz, image::imageops::FilterType::Triangle);
        let resized_mask = image::DynamicImage::ImageLuma8(mask.clone())
            .resize_exact(target_sz, target_sz, image::imageops::FilterType::Triangle);

        let img_rgb = resized_img.to_rgb8();
        let mask_luma = resized_mask.to_luma8();

        // Build tensors
        let mut img_tensor = Array4::<f32>::zeros([1, 3, target_sz as usize, target_sz as usize]);
        let mut mask_tensor = Array4::<f32>::zeros([1, 1, target_sz as usize, target_sz as usize]);

        for y in 0..target_sz {
            for x in 0..target_sz {
                let p = img_rgb.get_pixel(x, y);
                img_tensor[[0, 0, y as usize, x as usize]] = p[0] as f32 / 255.0;
                img_tensor[[0, 1, y as usize, x as usize]] = p[1] as f32 / 255.0;
                img_tensor[[0, 2, y as usize, x as usize]] = p[2] as f32 / 255.0;

                let m = mask_luma.get_pixel(x, y);
                mask_tensor[[0, 0, y as usize, x as usize]] = if m[0] > 128 { 1.0 } else { 0.0 };
            }
        }

        let ort_img = Tensor::<f32>::from_array(img_tensor)
            .map_err(|e| AppError::Internal(format!("build ort image tensor: {e}")))?;
        let ort_mask = Tensor::<f32>::from_array(mask_tensor)
            .map_err(|e| AppError::Internal(format!("build ort mask tensor: {e}")))?;

        let mut session = self.lama_session.lock().map_err(|_| AppError::Internal("lama session lock failed".to_string()))?;
        let outputs = session
            .run(inputs!["image" => ort_img, "mask" => ort_mask])
            .map_err(|e| AppError::Internal(format!("lama inference: {e}")))?;

        let raw = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| AppError::Internal(format!("extract lama tensor: {e}")))?;

        let (shape, data) = raw;
        // LaMa output is [1, 3, H, W]
        let h = shape[2] as usize;
        let w = shape[3] as usize;

        let inpainted_resized = RgbImage::from_fn(target_sz, target_sz, |x, y| {
            let r_idx = 0 * h * w + y as usize * w + x as usize;
            let g_idx = 1 * h * w + y as usize * w + x as usize;
            let b_idx = 2 * h * w + y as usize * w + x as usize;
            Rgb([
                (data[r_idx].clamp(0.0, 1.0) * 255.0) as u8,
                (data[g_idx].clamp(0.0, 1.0) * 255.0) as u8,
                (data[b_idx].clamp(0.0, 1.0) * 255.0) as u8,
            ])
        });

        let final_img = image::DynamicImage::ImageRgb8(inpainted_resized)
            .resize_exact(orig_w, orig_h, image::imageops::FilterType::Triangle)
            .to_rgb8();

        *image = final_img;
        Ok(())
    }
}

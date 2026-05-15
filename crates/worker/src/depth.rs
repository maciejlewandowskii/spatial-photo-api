use image::{imageops::FilterType, DynamicImage, GrayImage};
use ndarray::Array4;
use ort::{inputs, session::Session, value::Tensor};
use shared::error::AppError;

pub const DEPTHPRO_INPUT_SIZE: usize = 1536;
pub const DEPTH_ANYTHING_INPUT_SIZE: usize = 518;

const IMAGENET_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const IMAGENET_STD: [f32; 3] = [0.229, 0.224, 0.225];

use std::sync::Mutex;

pub struct DepthEstimator {
    session: Mutex<Session>,
    input_size: usize,
}

impl DepthEstimator {
    pub fn load(model_path: &str) -> Result<Self, AppError> {
        let input_size = if model_path.contains("depthpro") || model_path.contains("depth_pro") {
            DEPTHPRO_INPUT_SIZE
        } else {
            DEPTH_ANYTHING_INPUT_SIZE
        };

        let session = Session::builder()
            .map_err(|e| AppError::Internal(format!("ort builder: {e}")))?
            .commit_from_file(model_path)
            .map_err(|e| AppError::Internal(format!("load depth model '{model_path}': {e}")))?;

        Ok(Self { session: Mutex::new(session), input_size })
    }

    /// Returns a depth map normalised to [0.0, 1.0] at the source image's resolution.
    pub fn estimate(&self, image: &DynamicImage) -> Result<DepthMap, AppError> {
        let (orig_w, orig_h) = (image.width(), image.height());
        let sz = self.input_size;

        let resized = image.resize_exact(sz as u32, sz as u32, FilterType::Lanczos3);
        let rgb = resized.to_rgb8();

        let mut tensor = Array4::<f32>::zeros([1, 3, sz, sz]);
        for (x, y, pixel) in rgb.enumerate_pixels() {
            for c in 0..3 {
                let v = (pixel[c] as f32 / 255.0 - IMAGENET_MEAN[c]) / IMAGENET_STD[c];
                tensor[[0, c, y as usize, x as usize]] = v;
            }
        }

        let ort_tensor = Tensor::<f32>::from_array(tensor)
            .map_err(|e| AppError::Internal(format!("build ort tensor: {e}")))?;

        let mut session = self.session.lock().map_err(|_| AppError::Internal("depth session lock failed".to_string()))?;
        let outputs = session
            .run(inputs!["pixel_values" => ort_tensor])
            .map_err(|e| AppError::Internal(format!("depth inference: {e}")))?;

        let raw = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| AppError::Internal(format!("extract depth tensor: {e}")))?;

        let (_shape, data) = raw;
        let flat: Vec<f32> = data.to_vec();

        let (min, max) = flat.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| {
            (lo.min(v), hi.max(v))
        });
        let range = (max - min).max(1e-6);
        let normalised: Vec<f32> = flat.iter().map(|&v| (v - min) / range).collect();

        let model_gray = GrayImage::from_fn(sz as u32, sz as u32, |x, y| {
            let v = normalised[y as usize * sz + x as usize];
            image::Luma([(v * 255.0) as u8])
        });
        let scaled = DynamicImage::ImageLuma8(model_gray)
            .resize_exact(orig_w, orig_h, FilterType::Triangle);
        let final_gray = scaled.to_luma8();

        let depth_f32: Vec<f32> =
            final_gray.pixels().map(|p| f32::from(p[0]) / 255.0).collect();

        Ok(DepthMap {
            data: depth_f32,
            gray: final_gray,
        })
    }
}

pub struct DepthMap {
    /// Depth values in [0.0, 1.0]; higher = closer to camera.
    pub data: Vec<f32>,
    pub gray: GrayImage,
}

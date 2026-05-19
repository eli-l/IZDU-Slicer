use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use anyhow::{Error, Result};

pub enum AspectRatio {
    Preserve,
    Ignore,
}

impl AspectRatio {
    pub fn from_str(s: &str) -> Option<AspectRatio> {
        match s.to_lowercase().as_str() {
            "preserve" => Some(AspectRatio::Preserve),
            "ignore" => Some(AspectRatio::Ignore),
            _ => None,
        }
    }
}

pub struct ResizeParams {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub aspect_ratio: AspectRatio,
}

impl ResizeParams {
    pub fn new(width: Option<u32>, height: Option<u32>, aspect_ratio: AspectRatio) -> Self {
        ResizeParams { width, height, aspect_ratio }
    }
}

pub fn resize_image(img: &DynamicImage, params: &ResizeParams) -> Result<DynamicImage> {
    let (orig_w, orig_h) = (img.width(), img.height());

    let (target_w, target_h) = match (&params.width, &params.height, &params.aspect_ratio) {
        (None, None, _) => {
            return Ok(img.clone());
        }
        (Some(w), None, AspectRatio::Preserve) => {
            let ratio = *w as f64 / orig_w as f64;
            let h = (orig_h as f64 * ratio).round() as u32;
            (*w, h)
        }
        (None, Some(h), AspectRatio::Preserve) => {
            let ratio = *h as f64 / orig_h as f64;
            let w = (orig_w as f64 * ratio).round() as u32;
            (w, *h)
        }
        (Some(w), Some(h), AspectRatio::Preserve) => {
            let ratio_w = *w as f64 / orig_w as f64;
            let ratio_h = *h as f64 / orig_h as f64;
            let ratio = ratio_w.min(ratio_h);
            let new_w = (orig_w as f64 * ratio).round() as u32;
            let new_h = (orig_h as f64 * ratio).round() as u32;
            (new_w, new_h)
        }
        (Some(w), Some(h), AspectRatio::Ignore) => {
            (*w, *h)
        }
        (Some(w), None, AspectRatio::Ignore) => {
            (*w, orig_h)
        }
        (None, Some(h), AspectRatio::Ignore) => {
            (orig_w, *h)
        }
    };

    Ok(img.resize_exact(
        target_w,
        target_h,
        image::imageops::FilterType::Lanczos3,
    ))
}

pub fn resize_image_to_bytes(img: &DynamicImage, params: &ResizeParams) -> Result<Vec<u8>> {
    let resized = resize_image(img, params)?;
    let mut buf = std::io::Cursor::new(Vec::new());
    resized.write_to(&mut buf, image::ImageFormat::Png)?;
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_image(w: u32, h: u32) -> DynamicImage {
        ImageBuffer::from_fn(w, h, |x, y| {
            let r = (x % 256) as u8;
            let g = (y % 256) as u8;
            let b = ((x + y) % 256) as u8;
            Rgba([r, g, b, 255])
        }).into()
    }

    #[test]
    fn test_resize_width_only_preserves_aspect() {
        let img = make_test_image(200, 100);
        let params = ResizeParams::new(Some(50), None, AspectRatio::Preserve);
        let resized = resize_image(&img, &params).unwrap();
        assert_eq!(resized.width(), 50);
        assert_eq!(resized.height(), 25);
    }

    #[test]
    fn test_resize_height_only_preserves_aspect() {
        let img = make_test_image(200, 100);
        let params = ResizeParams::new(None, Some(25), AspectRatio::Preserve);
        let resized = resize_image(&img, &params).unwrap();
        assert_eq!(resized.width(), 50);
        assert_eq!(resized.height(), 25);
    }

    #[test]
    fn test_resize_both_preserve_fits_within_bounds() {
        let img = make_test_image(200, 100);
        let params = ResizeParams::new(Some(100), Some(60), AspectRatio::Preserve);
        let resized = resize_image(&img, &params).unwrap();
        assert!(resized.width() <= 100);
        assert!(resized.height() <= 60);
        let orig_ratio = 200.0 / 100.0;
        let new_ratio = resized.width() as f64 / resized.height() as f64;
        assert!((orig_ratio - new_ratio).abs() < 0.01);
    }

    #[test]
    fn test_resize_both_ignore_exact_dimensions() {
        let img = make_test_image(200, 100);
        let params = ResizeParams::new(Some(80), Some(40), AspectRatio::Ignore);
        let resized = resize_image(&img, &params).unwrap();
        assert_eq!(resized.width(), 80);
        assert_eq!(resized.height(), 40);
    }

    #[test]
    fn test_resize_no_dimensions_returns_original() {
        let img = make_test_image(200, 100);
        let params = ResizeParams::new(None, None, AspectRatio::Preserve);
        let resized = resize_image(&img, &params).unwrap();
        assert_eq!(resized.width(), 200);
        assert_eq!(resized.height(), 100);
    }

    #[test]
    fn test_aspect_ratio_from_str() {
        assert!(matches!(AspectRatio::from_str("preserve"), Some(AspectRatio::Preserve)));
        assert!(matches!(AspectRatio::from_str("ignore"), Some(AspectRatio::Ignore)));
        assert!(matches!(AspectRatio::from_str("PRESERVE"), Some(AspectRatio::Preserve)));
        assert!(matches!(AspectRatio::from_str("invalid"), None));
    }
}
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};

#[derive(Debug)]
pub struct Dimension {
    pub height: u32,
    pub width: u32,
    pub smallest: u32,
}

pub fn get_single_image_dimensions(img: &DynamicImage) -> Dimension {
    let h = img.height() / 2;
    let w = img.width() / 2;
    Dimension {
        height: h,
        width: w,
        smallest: std::cmp::min(h, w),
    }
}

pub fn initialize_output(w: u32, h: u32) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
    [
        ImageBuffer::new(w, h),
        ImageBuffer::new(w, h),
        ImageBuffer::new(w, h),
        ImageBuffer::new(w, h),
    ]
}

// Use Subview to split image, more clean code, seems to be a bit faster
pub fn slice_images_view(
    img: DynamicImage,
    new_img_size: &Dimension,
) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
    let mut output = initialize_output(new_img_size.width, new_img_size.height);
    output.iter_mut().enumerate().for_each(|(pic, new_img)| {
        let x = (pic as u32 % 2) * new_img_size.width;
        let y = (pic as u32 / 2) * new_img_size.height;
        let image = img.view(x, y, new_img_size.width, new_img_size.height);
        *new_img = image.to_image();
    });
    output
}

// Split image by copying pixels one by one - initial approach.
// Might be usable in future to alter some pixels while copying (watermarking?)
// Leaving it here as-is for now.
#[allow(dead_code)]
pub fn slice_images_copy_px(
    img: DynamicImage,
    new_img_size: Dimension,
) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
    let mut images = initialize_output(new_img_size.width, new_img_size.height);

    for pic in 0..4 {
        let new_img = &mut images[pic as usize];
        for i in 0..new_img_size.height {
            for j in 0..new_img_size.width {
                let x = i + ((pic % 2) * new_img_size.width);
                let y = j + ((pic / 2) * new_img_size.height);
                let px = img.get_pixel(x, y);
                new_img.put_pixel(i, j, px);
            }
        }
        let name = format!("{}.png", pic);
        println!("Saved image {}", &name)
    }

    images
}

pub fn resize(
    images: [ImageBuffer<Rgba<u8>, Vec<u8>>; 4],
    size: u32,
) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
    let mut resized = initialize_output(size, size);
    resized.iter_mut().enumerate().for_each(|(i, img)| {
        let resized_img = DynamicImage::ImageRgba8(images[i].clone()).resize(
            size,
            size,
            image::imageops::FilterType::Nearest,
        );
        *img = resized_img.to_rgba8();
    });
    resized
}

/// Resize a single DynamicImage to fit within the given bounds.
/// If width and height are both provided and aspect_ratio is "ignore",
/// resizes to exact dimensions. Otherwise scales to fit within bounds.
pub fn resize_single(
    img: DynamicImage,
    width: Option<u32>,
    height: Option<u32>,
    aspect_ratio: &str,
) -> DynamicImage {
    match (width, height, aspect_ratio) {
        (None, None, _) => img,
        (None, Some(_h), "ignore") => img, // caller must validate: ignore requires both dims
        (Some(_w), None, "ignore") => img, // caller must validate: ignore requires both dims
        (Some(w), Some(h), "ignore") => {
            img.resize_exact(w, h, image::imageops::FilterType::Triangle)
        }
        (Some(w), Some(h), _) => img.resize(w, h, image::imageops::FilterType::Triangle),
        (Some(w), None, _) => {
            let ratio = w as f64 / img.width() as f64;
            let new_h = (img.height() as f64 * ratio) as u32;
            img.resize(w, new_h, image::imageops::FilterType::Triangle)
        }
        (None, Some(h), _) => {
            let ratio = h as f64 / img.height() as f64;
            let new_w = (img.width() as f64 * ratio) as u32;
            img.resize(new_w, h, image::imageops::FilterType::Triangle)
        }
    }
}

/// Crop a region from a DynamicImage using 4 corner points (A, B, C, D).
/// Coordinates are in image pixel space, origin (0,0) at top-left.
/// A = top-left, B = top-right, C = bottom-left, D = bottom-right.
///
/// Coordinate contract (half-open intervals, per-pixel image space):
/// - 0 ≤ x < image_width, 0 ≤ y < image_height
/// - bx > ax (top-left X must be less than top-right X)
/// - cy > ay (top-left Y must be less than bottom-left Y)
/// - Output dimensions: width = bx - ax, height = cy - ay
///
pub fn crop_image(
    img: DynamicImage,
    a: (u32, u32),
    b: (u32, u32),
    c: (u32, u32),
    d: (u32, u32),
) -> Result<DynamicImage, String> {
    validate_crop_points(&img, a, b, c, d)?;

    let x = std::cmp::min(a.0, b.0);
    let y = std::cmp::min(a.1, c.1);
    let right = std::cmp::max(b.0, d.0);
    let bottom = std::cmp::max(c.1, d.1);
    let width = right - x;
    let height = bottom - y;

    if width == 0 || height == 0 {
        return Err("crop area must have positive width and height".to_string());
    }

    Ok(img.crop_imm(x, y, width, height))
}

fn validate_crop_points(
    img: &DynamicImage,
    a: (u32, u32),
    b: (u32, u32),
    c: (u32, u32),
    d: (u32, u32),
) -> Result<(), String> {
    let image_width = img.width();
    let image_height = img.height();

    // Bounds: 0 ≤ x < image_width, 0 ≤ y < image_height
    for (name, (x, y)) in [("A", a), ("B", b), ("C", c), ("D", d)] {
        if x >= image_width || y >= image_height {
            return Err(format!(
                "point {} ({}, {}) is out of bounds for image {}x{} (valid range: 0 ≤ x < {}, 0 ≤ y < {})",
                name, x, y, image_width, image_height, image_width, image_height
            ));
        }
    }

    // Ordering: ax < bx (top-left X before top-right X) and ay < cy (top-left Y before bottom-left Y)
    if a.0 >= b.0 || a.1 >= c.1 {
        return Err(
            "crop points must have A.x < B.x and A.y < C.y (top-left must be above-left of top-right)"
                .to_string(),
        );
    }

    // Axis alignment: A.x == C.x, A.y == B.y, B.x == D.x, C.y == D.y
    if a.0 != c.0 || a.1 != b.1 || b.0 != d.0 || c.1 != d.1 {
        return Err(
            "crop points must form an axis-aligned rectangle: A.x == C.x, A.y == B.y, B.x == D.x, C.y == D.y"
                .to_string(),
        );
    }

    Ok(())
}

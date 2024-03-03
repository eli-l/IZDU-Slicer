use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};

pub fn add_watermark(
    mut img: ImageBuffer<Rgba<u8>, Vec<u8>>,
    watermark: DynamicImage,
    alpha: f32,
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let (w, h) = (watermark.width(), watermark.height());
    let (iw, ih) = (img.width(), img.height());
    let x = (iw - w) / 2;
    let y = (ih - h) / 2;

    for i in 0..w {
        for j in 0..h {
            let wm = watermark.get_pixel(i, j);
            let image = *img.get_pixel(x + i, y + j);
            let alpha2 = f32::max(alpha, 0.5);
            let alpha1 = 1.0 - alpha2;
            let alpha = alpha1 + alpha2 * (1.0 - alpha1);
            let px = image::Rgba([
                ((alpha1 * image[0] as f32) + (alpha2 * wm[0] as f32 * (1.0 - alpha1))) as u8,
                ((alpha1 * image[1] as f32) + (alpha2 * wm[1] as f32 * (1.0 - alpha1))) as u8,
                ((alpha1 * image[2] as f32) + (alpha2 * wm[2] as f32 * (1.0 - alpha1))) as u8,
                (alpha * 255.0) as u8,
            ]);
            img.put_pixel(x + i, y + j, px);
        }
    }
    img
}

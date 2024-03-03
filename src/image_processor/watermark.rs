use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use rusttype::{Font, Scale};

pub fn create_watermark(text: &str, size: (u32, u32)) -> DynamicImage {
    let font_data = include_bytes!("../../resources/OpenSans-Regular.ttf");
    let font = Font::try_from_bytes(font_data as &[u8]).unwrap();

    let scale = Scale::uniform(20.0);
    let (width, height) = size;
    let wm_image = DynamicImage::ImageRgba8(render_text_to_image(&font, scale, text)).resize(
        width,
        height,
        image::imageops::FilterType::Nearest,
    );
    wm_image
}

fn render_text_to_image(font: &Font, scale: Scale, text: &str) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let v_metrics = font.v_metrics(scale);
    let glyphs: Vec<_> = font
        .layout(text, scale, rusttype::point(0.0, v_metrics.ascent))
        .collect();

    let glyphs_height = v_metrics.ascent - v_metrics.descent;
    let glyphs_width = glyphs
        .iter()
        .map(|g| g.position().x as f32 + g.unpositioned().h_metrics().advance_width)
        .fold(0.0 as f32, |a, b| a.max(b))
        .ceil() as u32;

    let mut image = ImageBuffer::new(glyphs_width, glyphs_height as u32);

    for glyph in glyphs {
        if let Some(bounding_box) = glyph.pixel_bounding_box() {
            glyph.draw(|x, y, v| {
                let x = x as i32 + bounding_box.min.x;
                let y = y as i32 + bounding_box.min.y;
                let v = (v * 255.0) as u8;
                image.put_pixel(x as u32, y as u32, Rgba([v, v, v, v]));
            });
        }
    }
    image
}

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

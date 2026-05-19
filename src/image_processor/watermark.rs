use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};

pub type Watermark = DynamicImage;

pub fn create_watermark(text: &str, size: (u32, u32)) -> Watermark {
    let font_data = include_bytes!("../../resources/OpenSans-Regular.ttf");
    let font = FontRef::try_from_slice(font_data).unwrap();

    let scale = PxScale::from(40.0);
    let (width, height) = size;
    let wm_image =
        DynamicImage::ImageRgba8(render_text_to_image(&font, scale, text))
            .resize_exact(width, height, image::imageops::FilterType::Nearest);
    wm_image
}

fn render_text_to_image(
    font: &FontRef,
    scale: PxScale,
    text: &str,
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let scaled_font = font.as_scaled(scale);

    let ascent = scaled_font.ascent();
    let descent = scaled_font.descent();
    let line_height = ascent - descent;

    // Collect glyphs for all characters
    let mut glyphs: Vec<(ab_glyph::Glyph, f32)> = Vec::new();
    let mut cursor_x: f32 = 0.0;

    for c in text.chars() {
        let glyph = scaled_font.scaled_glyph(c);
        let h_advance = scaled_font.h_advance(glyph.id);
        glyphs.push((glyph, cursor_x));
        cursor_x += h_advance;
    }

    if glyphs.is_empty() {
        return ImageBuffer::new(1, 1);
    }

    // Calculate bounds
    let mut max_x: f32 = 0.0;
    let mut max_y: f32 = 0.0;

    for (ref glyph, x_pos) in &glyphs {
        if let Some(outlined) = font.outline_glyph((*glyph).clone()) {
            let bounds = outlined.px_bounds();
            max_x = max_x.max(x_pos + bounds.max.x);
            max_y = max_y.max(bounds.max.y);
        }
    }

    let img_width = (max_x.ceil() as u32).max(1);
    let img_height = (line_height.ceil() as u32).max(1);
    let mut image = ImageBuffer::new(img_width, img_height);

    // Draw each glyph
    for (glyph, x_pos) in glyphs {
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let draw_x = (x_pos + bounds.min.x) as f32;
            let draw_y = (ascent + bounds.min.y) as f32;

            outlined.draw(|px, py, coverage| {
                let x = (draw_x + px as f32) as u32;
                let y = (draw_y + py as f32) as u32;
                if x < img_width && y < img_height {
                    let v = (coverage * 255.0) as u8;
                    image.put_pixel(x, y, Rgba([v, v, v, v]));
                }
            });
        }
    }

    image
}

pub fn add_watermark(
    mut img: ImageBuffer<Rgba<u8>, Vec<u8>>,
    watermark: &Watermark,
    alpha: f32,
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let (w, h) = (watermark.width(), watermark.height());
    let (iw, ih) = (img.width(), img.height());
    let x = (iw.saturating_sub(w)) / 2;
    let y = (ih.saturating_sub(h)) / 2;
    let effective_w = w.min(iw.saturating_sub(x));
    let effective_h = h.min(ih.saturating_sub(y));

    for i in 0..effective_w {
        for j in 0..effective_h {
            let wm_pixel = watermark.get_pixel(i, j);
            let wm_alpha = (wm_pixel[3] as f32 / 255.0) * (1.0 - alpha);
            let image = *img.get_pixel(x.saturating_add(i), y.saturating_add(j));
            let inv_a = 1.0 - wm_alpha;
            let px = Rgba([
                (wm_alpha * wm_pixel[0] as f32 + inv_a * image[0] as f32) as u8,
                (wm_alpha * wm_pixel[1] as f32 + inv_a * image[1] as f32) as u8,
                (wm_alpha * wm_pixel[2] as f32 + inv_a * image[2] as f32) as u8,
                image[3],
            ]);
            img.put_pixel(x.saturating_add(i), y.saturating_add(j), px);
        }
    }
    img
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use image::{GenericImageView, ImageBuffer, Rgba};
    use base64::Engine;

    // 4x4 blue PNG decoded from base64
    fn small_blue_img() -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        use base64::engine::general_purpose::STANDARD;
        let data = STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAIAAAAmkwkpAAAAEElEQVR4nGNgYPiPhIjiAACOsw/xs6MvMwAAAABJRU5ErkJggg==")
            .unwrap();
        image::load_from_memory(&data).unwrap().to_rgba8()
    }

    // 1x1 red PNG bytes -> ImageBuffer
    fn tiny_red_img() -> ImageBuffer<Rgba<u8>, Vec<u8>> {
        let bytes: [u8; 66] = [
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0a,
            0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
            0x08, 0x02, 0x4b, 0x0f, 0x51, 0x7d, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44,
            0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, 0x03, 0x01,
            0x01, 0x00, 0xc9, 0xfe, 0x92, 0xef, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
            0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        image::load_from_memory(&bytes).unwrap().to_rgba8()
    }

    // ------------------------------------------------------------------
    // create_watermark tests
    // ------------------------------------------------------------------

    #[test]
    fn create_watermark_returns_valid_image() {
        let wm = create_watermark("IZDU", (100, 40));
        assert!(wm.width() > 0, "watermark width must be > 0");
        assert!(wm.height() > 0, "watermark height must be > 0");
        assert_eq!(wm.width(), 100);
        assert_eq!(wm.height(), 40);
    }

    #[test]
    fn create_watermark_contains_rendered_pixels() {
        let wm = create_watermark("X", (50, 20));
        let rgba = wm.to_rgba8();
        // At least one pixel should be non-zero (text was rendered)
        let has_nonzero = rgba.pixels().any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0);
        assert!(has_nonzero, "watermark should contain rendered text pixels");
    }

    #[test]
    fn create_watermark_different_texts_produce_different_images() {
        let wm_a = create_watermark("AAA", (80, 30));
        let wm_b = create_watermark("BBBBB", (80, 30));
        assert_ne!(
            wm_a.to_rgba8().as_raw(),
            wm_b.to_rgba8().as_raw(),
            "different text should produce different watermark images"
        );
    }

    // ------------------------------------------------------------------
    // add_watermark tests
    // ------------------------------------------------------------------

    #[test]
    fn add_watermark_alpha_zero_is_opaque() {
        let img = small_blue_img(); // solid blue [0, 0, 255, 255]
        let wm = create_watermark("X", (img.width(), img.height()));

        let result = add_watermark(img.clone(), &wm, 0.0); // alpha=0 → opaque

        // With opaque watermark, center pixel should differ from original blue
        let center = result.get_pixel(img.width() / 2, img.height() / 2);
        assert_ne!(
            *center,
            Rgba([0, 0, 255, 255]),
            "alpha=0 watermark should replace the base image pixel"
        );
    }

    #[test]
    fn add_watermark_alpha_one_is_invisible() {
        let img = small_blue_img(); // solid blue [0, 0, 255, 255]
        let wm = create_watermark("X", (img.width(), img.height()));

        let result = add_watermark(img.clone(), &wm, 1.0); // alpha=1 → invisible

        // With fully transparent watermark, image should be unchanged
        let center = result.get_pixel(img.width() / 2, img.height() / 2);
        assert_eq!(
            *center,
            Rgba([0, 0, 255, 255]),
            "alpha=1 watermark should leave the base image unchanged"
        );
    }

    #[test]
    fn add_watermark_alpha_50_percent_blends() {
        let img = tiny_red_img(); // 1x1 red [255, 0, 0, 255]
        let wm = create_watermark("A", (1, 1));

        let result = add_watermark(img.clone(), &wm, 0.5); // 50% transparency

        let px = result.get_pixel(0, 0);
        assert_ne!(
            *px,
            Rgba([255, 0, 0, 255]),
            "50% alpha should not be pure original red"
        );
        assert_ne!(px[3], 0, "alpha channel should be non-zero");
    }

    #[test]
    fn add_watermark_preserves_image_dimensions() {
        let img = small_blue_img(); // 4x4
        let wm = create_watermark("X", (2, 2));

        let result = add_watermark(img, &wm, 0.3);

        assert_eq!(result.width(), 4, "width should be preserved");
        assert_eq!(result.height(), 4, "height should be preserved");
    }

    #[test]
    fn add_watermark_watermark_centered() {
        let img = small_blue_img(); // 4x4
        let wm = create_watermark("TEST", (2, 2));

        let result = add_watermark(img, &wm, 0.0);

        // Watermark centered: (4-2)/2 = 1, so center pixel (1,1) should differ
        let cx = (4 - 2) / 2;
        let cy = (4 - 2) / 2;
        let center = result.get_pixel(cx, cy);
        assert_ne!(
            *center,
            Rgba([0, 0, 255, 255]),
            "centered watermark should affect center pixels"
        );
    }

    // ------------------------------------------------------------------
    // Font loading smoke test
    // ------------------------------------------------------------------

    #[test]
    fn font_bytes_loadable() {
        let font_data = include_bytes!("../../resources/OpenSans-Regular.ttf");
        let font = FontRef::try_from_slice(font_data);
        assert!(
            font.is_ok(),
            "OpenSans-Regular.ttf should be loadable via include_bytes"
        );
    }
}
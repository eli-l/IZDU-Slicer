use anyhow::{Error, Result};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use reqwest;
use rusttype::{Font, Scale};

#[derive(Debug)]
struct Dimension {
    height: u32,
    width: u32,
    smallest: u32,
}

pub async fn process(url: &str, scale_px: u32) -> Result<[ImageBuffer<Rgba<u8>, Vec<u8>>; 4]> {
    let u = url.to_string();
    let img = download_image(u).await?;
    let single_img_size = get_single_image_dimensions(&img);
    let sliced = slice_images_view(img, &single_img_size);
    if scale_px > 0 && scale_px < single_img_size.smallest {
        return Ok(resize(sliced, scale_px));
    }
    Ok(sliced)
}

fn resize(
    images: [ImageBuffer<Rgba<u8>, Vec<u8>>; 4],
    size: u32,
) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
    let mut resized = initialize_output(size, size);
    resized.iter_mut().enumerate().for_each(|(i, img)| {
        let resized = DynamicImage::ImageRgba8(images[i].clone()).resize(
            size,
            size,
            image::imageops::FilterType::Nearest,
        );
        *img = resized.to_rgba8();
    });
    resized
}

// Use Subview to split image, more clean code, seems to be a bit faster
fn slice_images_view(
    img: DynamicImage,
    new_img_size: &Dimension,
) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
    let mut output = initialize_output(new_img_size.width, new_img_size.height);
    output.iter_mut().enumerate().for_each(|(pic, new_img)| {
        let x = (pic as u32 % 2) * new_img_size.width;
        let y = (pic as u32 / 2) * new_img_size.height;
        let view = img.view(x, y, new_img_size.width, new_img_size.height);

        let font_data = include_bytes!("../resources/OpenSans-Regular.ttf");
        let font = Font::try_from_bytes(font_data as &[u8]).unwrap();
        let text = "github.com/eli-l/IZDU-Slicer";
        let scale = Scale::uniform(24.0);
        let wm_ready = DynamicImage::ImageRgba8(render_text_to_image(&font, scale, text)).resize(
            600,
            300,
            image::imageops::FilterType::Nearest,
        );

        *new_img = add_watermark(view.to_image(), wm_ready, 0.8)
    });
    output
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

fn add_watermark(
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

            // let alpha1 = image[3] as f32 / 255.0;
            // let alpha2 = wm[3] as f32 / 255.0;
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
    // image::imageops::overlay(&mut img, &watermark, x as i64, y as i64);
    img
}

// Split image by copying pixels one by one - initial approach.
// Might be usable in future to alter some pixels while copying (watermarking?)
// Leaving it here as-is for now.
#[allow(dead_code)]
fn slice_images_copy_px(
    img: DynamicImage,
    new_img_size: Dimension,
) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
    let mut images = initialize_output(new_img_size.width, new_img_size.height);

    for pic in 0..4 {
        // let mut new_img = ImageBuffer::new(img_size.width, img_size.height);
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

fn initialize_output(w: u32, h: u32) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
    [
        ImageBuffer::new(w, h),
        ImageBuffer::new(w, h),
        ImageBuffer::new(w, h),
        ImageBuffer::new(w, h),
    ]
}

async fn download_image(url: String) -> Result<DynamicImage> {
    let response = reqwest::get(&url).await?;
    let img_bytes = response.bytes().await?;
    println!("Got image {}", &url);
    let size = img_bytes.len() * std::mem::size_of::<u8>();
    println!(
        "Initial image size: {:.2} MB",
        size as f64 / 1024.0 / 1024.0
    );
    image::load_from_memory(&img_bytes).map_err(|err| Error::new(err))
}

fn get_single_image_dimensions(img: &DynamicImage) -> Dimension {
    let h = img.height() / 2;
    let w = img.width() / 2;
    Dimension {
        height: h,
        width: w,
        smallest: std::cmp::min(h, w),
    }
}

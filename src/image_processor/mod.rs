pub mod image_slicer;
pub mod watermark;

use anyhow::{Error, Result};
use image::{DynamicImage, ImageBuffer, Rgba};

pub async fn slice(url: &str, scale_px: u32) -> Result<[ImageBuffer<Rgba<u8>, Vec<u8>>; 4]> {
    let u = url.to_string();
    let img = download_image(u).await?;
    let single_img_size = image_slicer::get_single_image_dimensions(&img);
    let mut sliced = image_slicer::slice_images_view(img, &single_img_size);

    // TODO: Make watermark optional via query parameter
    let text = "github.com/eli-l/IZDU-Slicer";
    sliced.iter_mut().for_each(|slice_img| {
        let wm_image = watermark::create_watermark(text, (slice_img.width(), slice_img.height()));
        *slice_img = watermark::add_watermark(slice_img.clone(), wm_image, 0.5);
    });

    if scale_px > 0 && scale_px < single_img_size.smallest {
        return Ok(image_slicer::resize(sliced, scale_px));
    }
    Ok(sliced)
}

pub async fn download_image(url: String) -> Result<DynamicImage> {
    let response = reqwest::get(&url).await?;

    if !response.status().is_success() {
        return Err(Error::msg(format!(
            "Failed to download image: {}. Status: {}",
            &url, response.status()
        )));
    }

    let img_bytes = response.bytes().await?;
    println!("Got image {}", &url);
    let size = img_bytes.len() * std::mem::size_of::<u8>();
    println!(
        "Initial image size: {:.2} MB",
        size as f64 / 1024.0 / 1024.0
    );
    image::load_from_memory(&img_bytes).map_err(|err| Error::new(err))
}
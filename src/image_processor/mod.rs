pub mod image_slicer;
pub mod watermark;

use actix_web::{web, HttpRequest};
use anyhow::{Error, Result};
use image::{DynamicImage, ImageBuffer, Rgba};
use crate::ImagePayload;
pub use crate::image_processor::watermark::Watermark;

pub enum ImageSource {
    Url(String),
    Binary(Vec<u8>),
    Base64(String),
}

fn get_content_type(req: &HttpRequest) -> &str {
    req.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
}

pub async fn get_source(
    req: HttpRequest,
    body: web::Bytes,
) -> Result<ImageSource> {
    let content_type = get_content_type(&req);

    let source = if content_type.starts_with("application/json") {
        match serde_json::from_slice::<ImagePayload>(&body) {
            Ok(payload) => {
                if let Some(url) = payload.image_url {
                    ImageSource::Url(url)
                } else if let Some(base64) = payload.image_base64 {
                    ImageSource::Base64(base64)
                } else {
                    return Err(Error::msg(
                        "No image source in JSON: provide image_url or image_base64",
                    ));
                }
            }
            Err(e) => {
                return Err(Error::msg(format!("Unrecognized JSON: {}", e)));
            }
        }
    } else if content_type.starts_with("image/")
        || content_type == "application/octet-stream"
        || !body.is_empty()
    {
        ImageSource::Binary(body.to_vec())
    } else {
        return Err(Error::msg(
            "Unsupported content type: provide JSON with image_url/image_base64 or binary image data",
        ));
    };
    Ok(source)
}

#[allow(dead_code)]
pub async fn slice(source: ImageSource, scale_px: u32) -> Result<[ImageBuffer<Rgba<u8>, Vec<u8>>; 4]> {
    let img = load_image(source).await?;
    let single_img_size = image_slicer::get_single_image_dimensions(&img);
    let sliced = image_slicer::slice_images_view(img, &single_img_size);

    if scale_px > 0 && scale_px < single_img_size.smallest {
        return Ok(image_slicer::resize(sliced, scale_px));
    }
    Ok(sliced)
}

#[allow(dead_code)]
pub async fn slice_with_watermark_text(
    source: ImageSource,
    scale_px: u32,
    watermark_text: &str,
    transparency: u16,
) -> Result<[ImageBuffer<Rgba<u8>, Vec<u8>>; 4]> {
    let img = load_image(source).await?;
    let single_img_size = image_slicer::get_single_image_dimensions(&img);
    let mut sliced = image_slicer::slice_images_view(img, &single_img_size);

    let wm_image = watermark::create_watermark(watermark_text, (single_img_size.width, single_img_size.height));
    sliced.iter_mut().for_each(|slice_img| {
        *slice_img = watermark::add_watermark(slice_img.clone(), &wm_image, transparency as f32 / 100.0);
    });

    if scale_px > 0 && scale_px < single_img_size.smallest {
        return Ok(image_slicer::resize(sliced, scale_px));
    }
    Ok(sliced)
}

#[allow(dead_code)]
pub async fn slice_with_watermark(
    source: ImageSource,
    scale_px: u32,
    watermark: Watermark,
    transparency: u16,
) -> Result<[ImageBuffer<Rgba<u8>, Vec<u8>>; 4]> {
    let img = load_image(source).await?;
    let single_img_size = image_slicer::get_single_image_dimensions(&img);
    let mut sliced = image_slicer::slice_images_view(img, &single_img_size);

    sliced.iter_mut().for_each(|slice_img| {
        *slice_img = watermark::add_watermark(slice_img.clone(), &watermark, transparency as f32 / 100.0);
    });

    if scale_px > 0 && scale_px < single_img_size.smallest {
        return Ok(image_slicer::resize(sliced, scale_px));
    }
    Ok(sliced)
}

pub async fn load_image(source: ImageSource) -> Result<DynamicImage> {
    match source {
        ImageSource::Url(url) => download_image(url).await,
        ImageSource::Binary(bytes) => load_from_bytes(bytes),
        ImageSource::Base64(base64_str) => load_from_base64(base64_str),
    }
}

async fn download_image(url: String) -> Result<DynamicImage> {
    let response = reqwest::get(&url).await?;

    if !response.status().is_success() {
        return Err(Error::msg(format!(
            "Failed to download image: {}. Status: {}",
            &url, response.status()
        )));
    }

    let img_bytes = response.bytes().await?;
    println!("Got image from URL: {}", &url);
    let size = img_bytes.len() * std::mem::size_of::<u8>();
    println!(
        "Initial image size: {:.2} MB",
        size as f64 / 1024.0 / 1024.0
    );
    image::load_from_memory(&img_bytes).map_err(|err| Error::new(err))
}

fn load_from_bytes(bytes: Vec<u8>) -> Result<DynamicImage> {
    println!("Loading image from binary data");
    let size = bytes.len() * std::mem::size_of::<u8>();
    println!(
        "Image size: {:.2} MB",
        size as f64 / 1024.0 / 1024.0
    );
    image::load_from_memory(&bytes).map_err(|err| Error::new(err))
}

fn load_from_base64(base64_str: String) -> Result<DynamicImage> {
    println!("Loading image from base64");
    use base64::{engine::general_purpose, Engine as _};

    let bytes = general_purpose::STANDARD
        .decode(base64_str.trim())
        .map_err(|e| Error::msg(format!("Failed to decode base64: {}", e)))?;

    let size = bytes.len() * std::mem::size_of::<u8>();
    println!(
        "Decoded image size: {:.2} MB",
        size as f64 / 1024.0 / 1024.0
    );
    image::load_from_memory(&bytes).map_err(|err| Error::new(err))
}
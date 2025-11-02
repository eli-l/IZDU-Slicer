use anyhow::{Error, Result};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use reqwest;

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
        *new_img = view.to_image();
    });
    output
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

fn get_single_image_dimensions(img: &DynamicImage) -> Dimension {
    let h = img.height() / 2;
    let w = img.width() / 2;
    Dimension {
        height: h,
        width: w,
        smallest: std::cmp::min(h, w),
    }
}

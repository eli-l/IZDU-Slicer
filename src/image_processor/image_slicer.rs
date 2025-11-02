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

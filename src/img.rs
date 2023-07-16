// The explicit module declaration is not needed. Each file acts as a module of it's own.
// So, if the file name is `img` and then you have this extra nested module `image_processor`, then accessing the functions
// from the root will be with the path `img::image_processor::[function_name]`.
pub mod image_processor {
    use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
    use reqwest;
    use std::error::Error;
    use std::fmt;

    // You can make use of the crates: `thiserror` or `anyhow` to make error handling easier.
    // Then you won't have to explicitly implement Error trait and the Display trait.
    // --
    // Personal opinion: because you are building a simple backed, then `anyhow` will be a better fit.
    // thiserror -- when building a library and you want to handle different errors and propagate them to the user.
    // anyhow -- when building an application and you just want to early return the error to the user.
    #[derive(Debug)]
    struct ImageFetchingError {
        message: String,
    }

    impl Error for ImageFetchingError {}

    impl fmt::Display for ImageFetchingError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "Error getting image: {}", self.message)
        }
    }

    #[derive(Debug)]
    struct Dimension {
        height: u32,
        width: u32,
    }

    // Wrap the return type in a anyhow::Result<[ImageBuffer<Rgba<u8>, Vec<u8>>; 4]> and then you wont have to unwrap().
    pub async fn slice_image(url: &str) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
        let u = url.to_string();
        let img = download_image(u).await.unwrap();
        let single_img_size = get_single_image_dimensions(&img);
        slice_images_view(img, single_img_size)
    }

    // Use Subview to split image, more clean code, seems to be a bit faster
    // NOTE: allow(dead_code) not needed here
    #[allow(dead_code)]
    fn slice_images_view(
        img: DynamicImage,
        new_img_size: Dimension,
    ) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
        // I am questioning if maybe returning a Vec<ImageBuffer<Rgba<u8>, Vec<u8>>> rather than a static array.
        // Otherwise, this whole function creates an empty image, and then completely overwrites it with the `view`.
        // I have a hunch that it's more expensive than return a dynamic array.
        // Then you could do `(0..4).map(|pic| { ... }).collect::<Vec<_>>()` and return that. and skip the `initialize_output` function.
        let mut images = initialize_output(new_img_size.width, new_img_size.height);

        // Note: minor rustification
        images.iter_mut().enumerate().for_each(|(pic, new_img)| {
            let x = (pic as u32 % 2) * new_img_size.width;
            let y = (pic as u32 / 2) * new_img_size.height;
            let cur_image = img
                .view(x, y, new_img_size.width, new_img_size.height)
                .to_image();

            *new_img = cur_image;
        });
        images
    }

    // Split image by copying pixels one by one - initial approach.
    // Might be usable in future to alter some pixels while copying (watermaking?)
    #[allow(dead_code)]
    fn slice_images_copy_px(
        img: DynamicImage,
        new_img_size: Dimension,
    ) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
        let mut images = initialize_output(new_img_size.width, new_img_size.height);

        // Note: minor rustification
        images.iter_mut().enumerate().for_each(|(pic, new_img)| {
            for i in 0..new_img_size.height {
                for j in 0..new_img_size.width {
                    let x = i + ((pic as u32 % 2) * new_img_size.width);
                    let y = j + ((pic as u32 / 2) * new_img_size.height);
                    let px = img.get_pixel(x, y);
                    new_img.put_pixel(i, j, px);
                }
            }
        });

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

    // If yoyu switch to anyhow::Result, then you will be able to early-return the error in a much easier fashion.
    async fn download_image(url: String) -> Result<DynamicImage, ImageFetchingError> {
        let response = reqwest::get(&url).await;
        let img_bytes = response.unwrap().bytes().await;

        match img_bytes {
            Ok(b) => {
                println!("Got image {}", &url);
                let size = b.len() * std::mem::size_of::<u8>();
                println!(
                    "Initial image size: {:.2} MB",
                    size as f64 / 1024.0 / 1024.0
                );
                image::load_from_memory(&b).map_err(|e| ImageFetchingError {
                    message: e.to_string(),
                })
            }
            Err(e) => Err(ImageFetchingError {
                message: e.to_string(),
            }),
        }
    }

    fn get_single_image_dimensions(img: &DynamicImage) -> Dimension {
        let h = img.height() / 2;
        let w = img.width() / 2;
        Dimension {
            height: h,
            width: w,
        }
    }
}

pub mod image_processor {
    use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
    use reqwest;
    use std::error::Error;
    use std::fmt;

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

    pub async fn slice_image(url: &str) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
        let u = url.to_string();
        let img = get_image(u).await.unwrap();
        let single_img_size = calculate_image_dimensions(&img);
        slice_images_view(img, single_img_size)
    }

    #[allow(dead_code)]
    fn slice_images_view(img: DynamicImage, new_img_size: Dimension) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
        let mut images = prepare_output(new_img_size.width, new_img_size.height);

        for pic in 0..4 {
            let x = (pic % 2) * new_img_size.width;
            let y = (pic / 2 ) * new_img_size.height;

            let cur_image = img.view(
                x,
                y,
                new_img_size.width,
                new_img_size.height
            ).to_image();


            //Debug statement below
            // cur_image.clone().save(format!("{}.png", pic)).unwrap();
            images[pic as usize] = cur_image;
        }

        images
    }

    #[allow(dead_code)]
    fn slice_images_copy_px(img: DynamicImage, new_img_size: Dimension) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
        let mut images = prepare_output(new_img_size.width, new_img_size.height);

        for pic in 0..4 {
            // let mut new_img = ImageBuffer::new(img_size.width, img_size.height);
            let new_img = &mut images[pic as usize];
            for i in 0..new_img_size.height {
                for j in 0..new_img_size.width {
                    let x= i + ((pic % 2) * new_img_size.width);
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

    fn prepare_output(w: u32, h: u32) -> [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] {
        let images: [ImageBuffer<Rgba<u8>, Vec<u8>>; 4] = [
            ImageBuffer::new(w, h),
            ImageBuffer::new(w, h),
            ImageBuffer::new(w, h),
            ImageBuffer::new(w, h),
        ];
        images
    }


    async fn get_image(url: String) -> Result<DynamicImage, ImageFetchingError> {
        let response = reqwest::get(&url).await;
        let img_bytes = response.unwrap().bytes().await;

        match img_bytes {
            Ok(b) => {
                println!("Got image {}", &url);
                let size = b.len() * std::mem::size_of::<u8>();
                println!("Initial image size: {:.2} MB", size as f64 / 1024.0 / 1024.0);
                image::load_from_memory(&b).map_err(|e| ImageFetchingError {
                    message: e.to_string(),
                })
            },
            Err(e) => Err(ImageFetchingError {
                message: e.to_string(),
            }),
        }
    }

    fn calculate_image_dimensions(img: &DynamicImage) -> Dimension {
        let h = img.height() / 2;
        let w = img.width() / 2;
        Dimension {
            height: h,
            width: w,
        }
    }

    #[allow(dead_code)]
    pub fn debug_size(w:f64, h: f64) {
        let bytes_per_pixel = image::ColorType::Rgb8.bytes_per_pixel() as f64;
        let size_in_bytes = (w * h) * bytes_per_pixel;
        let size_in_mb = size_in_bytes / 1024.0 / 1024.0;
        println!("Image size: {} MB", size_in_mb);
    }
}
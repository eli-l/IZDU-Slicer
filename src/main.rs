mod image_slicer;

use actix_web::{error, post, web, App, HttpResponse, HttpServer};
use futures::stream::unfold;
use image::ImageFormat;
use serde::Deserialize;
use std::env;
use std::io::{BufWriter, Cursor};

#[derive(Deserialize)]
struct ImageUrlPayload {
    image_url: String,
}

#[derive(Deserialize)]
struct RequestQuery {
    scale: Option<u32>,
}

#[post("/")]
async fn index(
    payload: web::Json<ImageUrlPayload>,
    query: web::Query<RequestQuery>,
) -> HttpResponse {
    let scale = match &query.scale {
        Some(val) => *val,
        None => {
            println!("Missing scale, using default 0");
            0
        }
    };

    let images = image_slicer::process(&payload.image_url, scale).await;

    let images = match images {
        Ok(images) => images,
        Err(e) => {
            println!("Error: {}", e);
            return HttpResponse::BadRequest().finish();
        }
    };

    let response_images = images.into_iter().map(|img| {
        let mut buf = BufWriter::new(Cursor::new(Vec::new()));
        let written = img.write_to(&mut buf, ImageFormat::Png);
        if written.is_err() {
            return None;
        }
        Some(buf.into_inner())
    });

    let stream = unfold(response_images, |mut iter| async move {
        iter.next().flatten().transpose().ok().flatten().map(|x| {
            let bytes = actix_web::web::Bytes::from(x.into_inner());
            (Ok::<_, error::Error>(bytes), iter)
        })
    });

    println!("Done");
    HttpResponse::Ok()
        .content_type("application/octet-stream")
        .streaming(stream)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Running");

    let port_str = match env::var("PORT") {
        Ok(val) => val,
        Err(_) => {
            println!("PORT not set, using default 9090");
            String::from("9090")
        }
    };

    let port = port_str.trim().parse().unwrap();

    let server = HttpServer::new(|| App::new().service(index))
        .bind(("0.0.0.0", port))?
        .run()
        .await;

    server
}

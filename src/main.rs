mod image_processor;

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
struct SliceQuery {
    scale: Option<u32>,
}

#[derive(Deserialize)]
struct WatermarkQuery {
    image_url: String,
    text: Option<String>,
    transparency: Option<i32>,
}

#[post("/slice")]
async fn slice(payload: web::Json<ImageUrlPayload>, query: web::Query<SliceQuery>) -> HttpResponse {
    let scale = match &query.scale {
        Some(val) => *val,
        None => {
            println!("Missing scale, using default 0");
            0
        }
    };

    let images = image_processor::process(&payload.image_url, scale).await;

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

#[post("/watermark")]
async fn watermark(query: web::Query<WatermarkQuery>) -> HttpResponse {
    let wm_text = match &query.text {
        Some(val) => val,
        None => "watermark",
    };

    let alpha = match &query.transparency {
        Some(val) => *val as f32 / 100.0,
        None => 0.5,
    };

    let img = query.image_url.to_string();

    HttpResponse::Ok().content_type("application/text").body(
        "Request, image: ".to_owned()
            + &img
            + ", text: "
            + wm_text
            + ", transparency: "
            + &alpha.to_string(),
    )
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Running");

    let port_str = env::var("PORT").unwrap_or_else(|_| {
        println!("PORT not set, using default 9090");
        String::from("9090")
    });

    let port = port_str.trim().parse().unwrap();

    let server = HttpServer::new(|| App::new().service(slice).service(watermark))
        .bind(("0.0.0.0", port))?
        .run()
        .await;

    server
}

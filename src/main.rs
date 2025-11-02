mod image_processor;

use actix_web::{error, post, web, App, HttpRequest, HttpResponse, HttpServer};
use futures::stream::unfold;
use image::ImageFormat;
use serde::Deserialize;
use std::env;
use std::io::{BufWriter, Cursor};
use crate::image_processor::get_source;

#[derive(Deserialize)]
struct ImagePayload {
    image_url: Option<String>,
    image_base64: Option<String>,
}

#[derive(Deserialize)]
struct SliceQuery {
    scale: Option<u32>,
    watermark: Option<String>,
    transparency: Option<u16>,
}

#[derive(Deserialize)]
struct WatermarkQuery {
    text: Option<String>,
    transparency: Option<u16>,
}

#[post("/slice")]
async fn slice(
    req: HttpRequest,
    body: web::Bytes,
    query: web::Query<SliceQuery>,
) -> HttpResponse {
    let scale = query.scale.unwrap_or(300);

    let source = match get_source(req, body).await {
        Ok(src) => src,
        Err(e) => {
            println!("Error: {}", e);
            return HttpResponse::BadRequest().body(format!("Error getting image source: {}", e));
        }
    };

    let images = match query.watermark.as_ref() {
        Some(wm) => {
            image_processor::slice_with_watermark_text(
                source,
                scale,
                wm,
                query.transparency.unwrap_or(30),
            )
            .await
        }
        None => image_processor::slice(source, scale).await,
    };

    let images = match images {
        Ok(images) => images,
        Err(e) => {
            println!("Error: {}", e);
            return HttpResponse::BadRequest().body(format!("Error processing image: {}", e));
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
async fn watermark(
    req: HttpRequest,
    body: web::Bytes,
    query: web::Query<WatermarkQuery> ) -> HttpResponse {
    let wm_text = match &query.text {
        Some(val) => val,
        None => "IZDU-Slicer",
    };

    let alpha = match &query.transparency {
        Some(val) => *val as f32 / 100.0,
        None => 0.3,
    };

    // TODO: add logic
   // match get_source(req, body).await {
   //      Ok(img) => img,
   //      Err(e) => {
   //          println!("Error: {}", e);
   //          return HttpResponse::BadRequest().body(format!("Error getting image source: {}", e));
   //      }
   //  };

    HttpResponse::Ok().content_type("text/plain").body(
        format!("Request, image: , text: {}, transparency: {}", wm_text, alpha)
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

    let server = HttpServer::new(|| App::new()
            .service(watermark)
            .service(slice)
        )
        .bind(("0.0.0.0", port))?
        .run()
        .await;

    server
}

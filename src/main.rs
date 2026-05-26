#[cfg(test)]
mod tests;

mod grpc;
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
struct WatermarkTextQuery {
    text: String,
    transparency: Option<u16>,
}

#[derive(Deserialize)]
struct ResizeQuery {
    width: Option<u32>,
    height: Option<u32>,
    aspect_ratio: Option<String>,
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
    query: web::Query<WatermarkTextQuery>,
) -> HttpResponse {
    let source = match get_source(req, body).await {
        Ok(src) => src,
        Err(e) => {
            println!("Error: {}", e);
            return HttpResponse::BadRequest().body(format!("Error getting image source: {}", e));
        }
    };

    let text = query.text.trim();
    let alpha = query.transparency.unwrap_or(30) as f32 / 100.0;

    let img = match image_processor::load_image(source).await {
        Ok(img) => img,
        Err(e) => {
            println!("Error: {}", e);
            return HttpResponse::BadRequest().body(format!("Error loading image: {}", e));
        }
    };

    let (w, h) = (img.width(), img.height());
    let wm_image = image_processor::watermark::create_watermark(text, (w, h));
    let watermarked = image_processor::watermark::add_watermark(img.to_rgba8(), &wm_image, alpha);

    let mut buf = BufWriter::new(Cursor::new(Vec::new()));
    if watermarked.write_to(&mut buf, ImageFormat::Png).is_err() {
        return HttpResponse::InternalServerError().body("Error encoding image");
    }
    let cursor = match BufWriter::into_inner(buf) {
        Ok(c) => c,
        Err(_) => return HttpResponse::InternalServerError().body("Error finalizing image"),
    };
    let bytes = cursor.into_inner();

    println!("Watermarked image: {}x{}, text: \"{}\", transparency: {}", w, h, text, alpha);
    HttpResponse::Ok().content_type("image/png").body(bytes)
}

#[post("/resize")]
pub async fn resize_handler(
    req: HttpRequest,
    body: web::Bytes,
    query: web::Query<ResizeQuery>,
) -> HttpResponse {
    let source = match get_source(req, body).await {
        Ok(src) => src,
        Err(e) => {
            println!("Error: {}", e);
            return HttpResponse::BadRequest().body(format!("Error getting image source: {}", e));
        }
    };

    let ar = query.aspect_ratio.as_deref().unwrap_or("preserve");

    if ar == "ignore" && (query.width.is_none() || query.height.is_none()) {
        return HttpResponse::BadRequest().body(
            "aspect_ratio=ignore requires both width and height",
        );
    }

    let img = match image_processor::resize_image(source, query.width, query.height, ar).await {
        Ok(img) => img,
        Err(e) => {
            println!("Error: {}", e);
            return HttpResponse::BadRequest().body(format!("Error resizing image: {}", e));
        }
    };

    let mut buf = BufWriter::new(Cursor::new(Vec::new()));
    if img.write_to(&mut buf, ImageFormat::Png).is_err() {
        return HttpResponse::InternalServerError().body("Error encoding image");
    }
    let cursor = match BufWriter::into_inner(buf) {
        Ok(c) => c,
        Err(_) => {
            return HttpResponse::InternalServerError().body("Error finalizing image");
        }
    };
    let bytes = cursor.into_inner();

    println!("Resized image: {}x{}", img.width(), img.height());
    HttpResponse::Ok().content_type("image/png").body(bytes)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Running");

    let grpc_port: u16 = env::var("GRPC_PORT")
        .unwrap_or_else(|_| {
            println!("GRPC_PORT not set, using default 50051");
            String::from("50051")
        })
        .trim()
        .parse()
        .unwrap_or(50051);

    // Run gRPC server in background task (needs to be Send)
    let grpc_addr: std::net::SocketAddr = format!("0.0.0.0:{}", grpc_port)
        .parse()
        .expect("invalid gRPC address");

    tokio::spawn(async move {
        println!("Starting gRPC server on {}", grpc_addr);
        let module = grpc::server::GrpcServer::default();
        tonic::transport::Server::builder()
            .add_service(grpc::ImageProcessorServer::new(module))
            .serve(grpc_addr)
            .await
            .expect("gRPC server error");
    });

    // HTTP server on main actix thread
    let http_port: u16 = env::var("PORT")
        .unwrap_or_else(|_| {
            println!("PORT not set, using default 9090");
            String::from("9090")
        })
        .trim()
        .parse()
        .unwrap_or(9090);

    println!("Starting HTTP server on 0.0.0.0:{}", http_port);
    HttpServer::new(|| {
        App::new()
            .service(watermark)
            .service(slice)
            .service(resize_handler)
    })
    .bind(("0.0.0.0", http_port))?
    .run()
    .await
}
mod img;

use actix_web::{error, post, web, App, HttpResponse, HttpServer};
use futures::stream::unfold;
use image::{EncodableLayout, ImageFormat};
use serde::Deserialize;
use std::io::{BufWriter, Cursor, Seek, Write};

#[allow(dead_code)]
struct AppState {
    app_name: String,
}

#[derive(Deserialize)]
struct ImageUrlPayload {
    image_url: String,
}

// Probably look into how to handle errors with actix-web. So you could have something like Result<HttpResponse, Error> and then the error gets handled by actix-web and returned to the user.
// It's been a few years since I worked with actix web so I don't remember the exact details on how to do that.
// That way you can get rid of the `unwrap()` in the fnction body.
#[post("/")]
async fn index(payload: web::Json<ImageUrlPayload>) -> HttpResponse {
    let images = img::image_processor::slice_image(&payload.image_url).await;

    let response_images = images.into_iter().map(|img| {
        let mut buf = BufWriter::new(Cursor::new(Vec::new()));
        let written = img.write_to(&mut buf, ImageFormat::Png);
        if written.is_err() {
            return None;
        };
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

    // Take a look at the `tracing` crate and `tracing-subscriber` crate.
    // It's a really nice way to get logs from your application.
    // Then you can drop the ugly println!() statements.

    let server = HttpServer::new(|| {
        App::new()
            .app_data(web::Data::new(AppState {
                app_name: String::from("Actix Webs"),
            }))
            .service(index)
    })
    .bind(("0.0.0.0", 9090))?
    .run()
    .await;

    server
}

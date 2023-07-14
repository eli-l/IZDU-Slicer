mod img;

use actix_web::{post, HttpServer, HttpResponse, App, web, error};
use std::io::{BufWriter, Cursor, SeekFrom, Seek, Write};
use image::ImageFormat;
use futures::stream::unfold;
use serde::Deserialize;

#[allow(dead_code)]
struct AppState {
    app_name: String,
}

#[derive(Deserialize)]
struct ImageUrlPayload {
    image_url: String,
}


#[post("/")]
async fn index(payload: web::Json<ImageUrlPayload>) -> HttpResponse {
    let images = img::image_processor::slice_image(&payload.image_url)
        .await;

    let mut resp_buf: Vec<Vec<u8>> = Vec::new();
    for (i, image) in images.iter().enumerate() {
        let mut buf = BufWriter::new(Cursor::new(Vec::new()));
        let mut compressed = Cursor::new(Vec::new());
        image.write_to(&mut compressed, ImageFormat::Png).unwrap();
        buf.write_all(&compressed.into_inner()).unwrap();
        // Debug
        let wr = buf.get_mut();
        let pos = wr.seek(SeekFrom::Current(0)).expect("failed to seek");
        let size = pos;
        println!("Image {} size: {:.2} MB", i+1, size as f64 / 1024.0 / 1024.0);
        resp_buf.push(buf.into_inner().unwrap().into_inner());
    }

    let stream = unfold(resp_buf.into_iter(), |mut iter| async move {
        iter.next().map(|x| {
            let bytes = actix_web::web::Bytes::from(x);
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
    HttpServer::new(||{
        App::new()
            .app_data(web::Data::new(AppState {
                app_name: String::from("Actix Webs")
            }))
            .service(index)
    })
    .bind(("0.0.0.0", 9090))?
    .run()
    .await
}
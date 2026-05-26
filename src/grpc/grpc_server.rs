pub mod grpc {
    pub mod proto {
        tonic::include_proto!("izdu");
    }
}

use crate::image_processor::{self, ImageSource};
use crate::image_processor::watermark;
use crate::image_processor::image_slicer;
use bytes::Bytes;
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status, Streaming};
use image::ImageBuffer;

use self::proto::image_processor_server::ImageProcessor;
use self::proto::*;

pub struct GrpcServer;

impl GrpcServer {
    pub fn new() -> Self {
        Self
    }
}

fn image_source_to_source(src: ImageSource) -> crate::image_processor::ImageSource {
    match src {
        ImageSource { source: Some(proto::image_source::Source::Url(url))) } =>
            crate::image_processor::ImageSource::Url(url),
        ImageSource { source: Some(proto::image_source::Source::Data(bytes))) =>
            crate::image_processor::ImageSource::Binary(bytes),
        ImageSource { source: Some(proto::image_source::Source::Base64(b64))) =>
            crate::image_processor::ImageSource::Base64(b64),
        _ => crate::image_processor::ImageSource::Binary(vec![]),
    }
}

fn encode_to_png(img: ImageBuffer<image::Rgba<u8>, Vec<u8>>) -> Result<Bytes, String> {
    let mut buf = Vec::new();
    let mut writer = std::io::Cursor::new(&mut buf);
    img.write_to(&mut writer, image::ImageFormat::Png)
        .map_err(|e| format!("PNG encode error: {}", e))?;
    Ok(Bytes::from(buf))
}

fn decode_watermark_config(wm: Option<WatermarkConfig>) -> (String, u16) {
    wm.map(|w| (w.text, w.transparency))
        .unwrap_or_else(|| ("IZDU-Slicer".to_string(), 30))
}

fn decode_resize_config(rc: Option<ResizeConfig>) -> (Option<u32>, Option<u32>, String) {
    rc.map(|r| {
        let ar = if r.aspect_ratio.is_empty() { "preserve".to_string() } else { r.aspect_ratio };
        (Some(r.width), Some(r.height), ar)
    })
    .unwrap_or((None, None, "preserve".to_string()))
}

#[tonic::async_trait]
impl ImageProcessor for GrpcServer {
    type SliceStream = Pin<Box<dyn tokio_stream::Stream<Item = Result<SliceResponse, Status>> + Send>>;
    type ProcessBatchStream = Pin<Box<dyn tokio_stream::Stream<Item = Result<BatchResponse, Status>> + Send>>;

    async fn slice(
        &self,
        request: Request<SliceRequest>,
    ) -> Result<Response<Self::SliceStream>, Status> {
        let req = request.into_inner();
        let source = image_source_to_source(req.source);
        let scale = if req.scale == 0 { 300 } else { req.scale };

        let img = match image_processor::load_image(source).await {
            Ok(img) => img,
            Err(e) => {
                return Err(Status::internal(format!("failed to load image: {}", e)));
            }
        };

        let (wm_text, wm_alpha) = decode_watermark_config(req.watermark);
        let single_img_size = image_slicer::get_single_image_dimensions(&img);

        let sliced = if wm_text.is_empty() {
            image_slicer::slice_images_view(img, &single_img_size)
        } else {
            let wm = watermark::create_watermark(&wm_text, (single_img_size.width, single_img_size.height));
            let mut out = image_slicer::slice_images_view(img.clone(), &single_img_size);
            for slice_img in &mut out {
                *slice_img = watermark::add_watermark(slice_img.clone(), &wm, wm_alpha as f32 / 100.0);
            }
            out
        };

        let sliced = if scale > 0 && scale < single_img_size.smallest {
            image_slicer::resize(sliced, scale)
        } else {
            sliced
        };

        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(async move {
            for (i, img) in sliced.into_iter().enumerate() {
                let data = match encode_to_png(img) {
                    Ok(d) => d,
                    Err(e) => {
                        let _ = tx.send(Ok(SliceResponse {
                            index: i as u32,
                            data: vec![],
                            error: e,
                        })).await;
                        continue;
                    }
                };
                let _ = tx.send(Ok(SliceResponse {
                    index: i as u32,
                    data: data.to_vec(),
                    error: String::new(),
                })).await;
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream) as Self::SliceStream))
    }

    async fn watermark(
        &self,
        request: Request<WatermarkRequest>,
    ) -> Result<Response<WatermarkResponse>, Status> {
        let req = request.into_inner();
        let source = image_source_to_source(req.source);
        let (text, alpha) = decode_watermark_config(req.watermark);

        let img = match image_processor::load_image(source).await {
            Ok(img) => img,
            Err(e) => {
                return Err(Status::internal(format!("failed to load image: {}", e)));
            }
        };

        let (w, h) = (img.width(), img.height());
        let wm = watermark::create_watermark(&text, (w, h));
        let watermarked = watermark::add_watermark(img.to_rgba8(), &wm, alpha as f32 / 100.0);

        let data = encode_to_png(watermarked)
            .map_err(|e| Status::internal(e))?;

        Ok(Response::new(WatermarkResponse {
            data: data.to_vec(),
            error: String::new(),
        }))
    }

    async fn resize(
        &self,
        request: Request<ResizeRequest>,
    ) -> Result<Response<ResizeResponse>, Status> {
        let req = request.into_inner();
        let source = image_source_to_source(req.source);
        let (width, height, aspect_ratio) = decode_resize_config(req.resize);

        let img = match image_processor::load_image(source).await {
            Ok(img) => img,
            Err(e) => {
                return Err(Status::internal(format!("failed to load image: {}", e)));
            }
        };

        if aspect_ratio == "ignore" && (width.is_none() || height.is_none()) {
            return Err(Status::invalid_argument(
                "aspect_ratio=ignore requires both width and height",
            ));
        }

        let resized = image_slicer::resize_single(img, width, height, &aspect_ratio);
        let data = encode_to_png(resized.to_rgba8())
            .map_err(|e| Status::internal(e))?;

        Ok(Response::new(ResizeResponse {
            data: data.to_vec(),
            error: String::new(),
        }))
    }

    async fn process_batch(
        &self,
        request: Request<Streaming<BatchRequest>>,
    ) -> Result<Response<Self::ProcessBatchStream>, Status> {
        let mut stream = request.into_inner();
        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            while let Some(req) = stream.next().await {
                let req = match req {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx.send(Err(Status::internal(e.to_string()))).await;
                        continue;
                    }
                };

                let request_id = req.request_id.clone();
                let result: Result<BatchResponse, String> = match req.operation {
                    Some(proto::operation::Op::Slice(s)) => {
                        let req = SliceRequest {
                            source: Some(proto::ImageSource {
                                source: s.source.map(|src| match src {
                                    proto::slice_op::Source::Url(u) =>
                                        proto::image_source::Source::Url(u),
                                    proto::slice_op::Source::Data(d) =>
                                        proto::image_source::Source::Data(d),
                                    proto::slice_op::Source::Base64(b) =>
                                        proto::image_source::Source::Base64(b),
                                }),
                            }),
                            scale: s.scale,
                            watermark: s.watermark,
                        };
                        let source = image_source_to_source(req.source.clone().unwrap_or_default());
                        let scale = if req.scale == 0 { 300 } else { req.scale };
                        let (wm_text, wm_alpha) = decode_watermark_config(req.watermark.clone());

                        let img = match image_processor::load_image(source).await {
                            Ok(i) => i,
                            Err(e) => {
                                break BatchResponse { request_id, error: e.to_string(), result: None };
                            }
                        };

                        let single_img_size = image_slicer::get_single_image_dimensions(&img);
                        let mut sliced = image_slicer::slice_images_view(img, &single_img_size);

                        if !wm_text.is_empty() {
                            let wm = watermark::create_watermark(&wm_text, (single_img_size.width, single_img_size.height));
                            for slice_img in &mut sliced {
                                *slice_img = watermark::add_watermark(slice_img.clone(), &wm, wm_alpha as f32 / 100.0);
                            }
                        }

                        let sliced = if scale > 0 && scale < single_img_size.smallest {
                            image_slicer::resize(sliced, scale)
                        } else {
                            sliced
                        };

                        let slices: Vec<SliceResponse> = sliced
                            .into_iter()
                            .enumerate()
                            .map(|(i, img)| {
                                let data = encode_to_png(img).unwrap_or_default();
                                SliceResponse { index: i as u32, data: data.to_vec(), error: String::new() }
                            })
                            .collect();

                        break BatchResponse {
                            request_id,
                            error: String::new(),
                            result: Some(proto::batch_response::Result::Slice(BatchSliceResult { slices })),
                        };
                    }
                    Some(proto::operation::Op::Watermark(w)) => {
                        break BatchResponse { request_id, error: "not implemented".into(), result: None };
                    }
                    Some(proto::operation::Op::Resize(r)) => {
                        break BatchResponse { request_id, error: "not implemented".into(), result: None };
                    }
                    None => {
                        break BatchResponse { request_id, error: "no operation specified".into(), result: None };
                    }
                };

                let resp = match result {
                    Ok(r) => r,
                    Err(e) => BatchResponse { request_id: request_id.clone(), error: e, result: None },
                };
                let _ = tx.send(Ok(resp)).await;
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream) as Self::ProcessBatchStream))
    }
}

mod proto {
    tonic::include_proto!("izdu");
}
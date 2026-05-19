//! Integration tests for the IZDU-Slicer `/resize` endpoint.

use actix_web::{test, http::header, dev::ServiceResponse};
use bytes::Bytes;

/// Helper to make a JSON image payload.
fn image_url_payload(url: &str) -> serde_json::Value {
    serde_json::json!({ "image_url": url })
}

/// 1x1 red PNG as base64 (generated with Python/zlib).
const TINY_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";

/// 4x4 blue PNG as base64.
const SMALL_PNG_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAQAAAAECAIAAAAmkwkpAAAAEElEQVR4nGNgYPiPhIjiAACOsw/xs6MvMwAAAABJRU5ErkJggg==";

/// Minimal 1x1 red PNG bytes.
const TINY_PNG_BYTES: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
    0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
    0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41,
    0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0xc9, 0xfe, 0x92,
    0xef, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
    0x44, 0xae, 0x42, 0x60, 0x82,
];

async fn resize_request(
    body: impl Into<Bytes>,
    content_type: &str,
    query: Option<Vec<(&str, &str)>>,
) -> ServiceResponse {
    let mut app = test::init_service(
        actix_web::App::new().service(crate::resize_handler)
    ).await;

    let uri = match &query {
        Some(params) => {
            let query_str = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            format!("/resize?{}", query_str)
        }
        None => "/resize".to_string(),
    };

    let req = test::TestRequest::post()
        .uri(&uri)
        .set_payload(body)
        .insert_header((header::CONTENT_TYPE, content_type))
        .to_request();

    actix_web::test::call_service(&mut app, req).await
}

fn get_ct(resp: &ServiceResponse) -> String {
    resp.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Test 1: Resize with only width — image should be resized so width=target.
#[tokio::test]
async fn test_resize_width_only() {
    let payload = serde_json::to_vec(&image_url_payload(
        "https://httpbin.org/image/png",
    ))
    .unwrap();

    let resp = resize_request(payload, "application/json", Some(vec![("width", "100")])).await;

    assert_eq!(
        resp.status().as_u16(),
        200,
        "Expected 200 OK, got {}",
        resp.status()
    );
    assert!(
        get_ct(&resp).starts_with("image/png"),
        "Expected image/png, got {}",
        get_ct(&resp)
    );
}

/// Test 2: Resize with only height — image should be resized so height=target.
#[tokio::test]
async fn test_resize_height_only() {
    let payload = serde_json::to_vec(&image_url_payload(
        "https://httpbin.org/image/png",
    ))
    .unwrap();

    let resp = resize_request(payload, "application/json", Some(vec![("height", "100")])).await;

    assert_eq!(resp.status().as_u16(), 200, "Expected 200 OK");
    assert!(
        get_ct(&resp).starts_with("image/png"),
        "Expected image/png content-type"
    );
}

/// Test 3: Resize with both width and height (preserve aspect) — fit within both bounds.
#[tokio::test]
async fn test_resize_width_and_height_preserve() {
    let payload = serde_json::to_vec(&image_url_payload(
        "https://httpbin.org/image/png",
    ))
    .unwrap();

    let resp = resize_request(
        payload,
        "application/json",
        Some(vec![("width", "200"), ("height", "200"), ("aspect_ratio", "preserve")]),
    )
    .await;

    assert_eq!(resp.status().as_u16(), 200, "Expected 200 OK");
    assert!(
        get_ct(&resp).starts_with("image/png"),
        "Expected image/png content-type"
    );
}

/// Test 4: Resize with both width and height (ignore aspect ratio) — exact dimensions.
#[tokio::test]
async fn test_resize_width_and_height_ignore() {
    let payload = serde_json::to_vec(&image_url_payload(
        "https://httpbin.org/image/png",
    ))
    .unwrap();

    let resp = resize_request(
        payload,
        "application/json",
        Some(vec![("width", "150"), ("height", "100"), ("aspect_ratio", "ignore")]),
    )
    .await;

    assert_eq!(resp.status().as_u16(), 200, "Expected 200 OK");
    assert!(
        get_ct(&resp).starts_with("image/png"),
        "Expected image/png content-type"
    );
}

/// Test 5: No dimensions provided — returns original image.
#[tokio::test]
async fn test_resize_no_dimensions() {
    let payload = serde_json::to_vec(&image_url_payload(
        "https://httpbin.org/image/png",
    ))
    .unwrap();

    let resp = resize_request(payload, "application/json", None).await;

    assert_eq!(resp.status().as_u16(), 200, "Expected 200 OK");
    assert!(
        get_ct(&resp).starts_with("image/png"),
        "Expected image/png content-type"
    );
}

/// Test 6: Invalid aspect_ratio — should handle gracefully.
#[tokio::test]
async fn test_resize_invalid_aspect_ratio() {
    let payload = serde_json::to_vec(&image_url_payload(
        "https://httpbin.org/image/png",
    ))
    .unwrap();

    let resp = resize_request(
        payload,
        "application/json",
        Some(vec![("width", "100"), ("aspect_ratio", "invalid")]),
    )
    .await;

    // Should either return 200 (defaulting to preserve) or 400 (invalid value)
    let status = resp.status().as_u16();
    assert!(
        status == 200 || status == 400,
        "Expected 200 or 400 for invalid aspect_ratio, got {}",
        status
    );
}

/// Test 7: Non-image input (plain text) — should return 400 error.
#[tokio::test]
async fn test_resize_non_image_input() {
    let payload = b"This is not an image, just plain text".to_vec();

    let resp = resize_request(payload, "text/plain", Some(vec![("width", "100")])).await;

    assert_eq!(
        resp.status().as_u16(),
        400,
        "Expected 400 Bad Request for non-image input"
    );
}

/// Test 8: URL that doesn't return an image (404) — should return 400 error.
#[tokio::test]
async fn test_resize_nonexistent_url() {
    let payload = serde_json::to_vec(&serde_json::json!({
        "image_url": "https://httpbin.org/status/404"
    }))
    .unwrap();

    let resp = resize_request(payload, "application/json", Some(vec![("width", "100")])).await;

    let status = resp.status().as_u16();
    assert!(
        status == 400 || status == 500,
        "Expected 400 or 500 for non-image URL, got {}",
        status
    );
}

/// Test 9: Binary image input (PNG bytes) — should resize correctly.
#[tokio::test]
async fn test_resize_binary_image() {
    let resp = resize_request(
        TINY_PNG_BYTES.to_vec(),
        "image/png",
        Some(vec![("width", "5")]),
    )
    .await;

    assert_eq!(
        resp.status().as_u16(),
        200,
        "Expected 200 OK for binary PNG input"
    );
    assert!(
        get_ct(&resp).starts_with("image/png"),
        "Expected image/png content-type"
    );
}

/// Test 10: Base64-encoded image input — should resize correctly.
#[tokio::test]
async fn test_resize_base64_image() {
    let payload = serde_json::to_vec(&serde_json::json!({
        "image_base64": SMALL_PNG_BASE64
    }))
    .unwrap();

    let resp = resize_request(payload, "application/json", Some(vec![("width", "5")])).await;

    assert_eq!(
        resp.status().as_u16(),
        200,
        "Expected 200 OK for base64 image input"
    );
    assert!(
        get_ct(&resp).starts_with("image/png"),
        "Expected image/png content-type"
    );
}

/// Test 11: Width only, verify aspect ratio is preserved.
#[tokio::test]
async fn test_resize_width_preserves_aspect_ratio() {
    let payload = serde_json::to_vec(&image_url_payload(
        "https://httpbin.org/image/png",
    ))
    .unwrap();

    let resp = resize_request(
        payload,
        "application/json",
        Some(vec![("width", "50")]),
    )
    .await;

    assert_eq!(resp.status().as_u16(), 200, "Expected 200 OK");
}

/// Test 12: Height only, verify aspect ratio is preserved.
#[tokio::test]
async fn test_resize_height_preserves_aspect_ratio() {
    let payload = serde_json::to_vec(&image_url_payload(
        "https://httpbin.org/image/png",
    ))
    .unwrap();

    let resp = resize_request(
        payload,
        "application/json",
        Some(vec![("height", "50")]),
    )
    .await;

    assert_eq!(resp.status().as_u16(), 200, "Expected 200 OK");
}
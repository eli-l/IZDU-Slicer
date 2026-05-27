# Architecture

## Overview

IZDU Slicer (Image Zero Disk Usage Slicer) is a Rust-based HTTP service that splits a single image into **4 equal quadrants** (each half the width and half the height of the original). The result is streamed back as a continuous binary response, allowing callers to reconstruct the full image without ever storing it on disk.

The "zero disk usage" name refers to the fact that all image processing happens in memory — no intermediate files are written to disk.

---

## Tech Stack

| Component | Technology | Version | Purpose |
|-----------|-----------|---------|---------|
| Language | Rust | 2021 edition | Memory-safe, high-performance |
| Web framework | actix-web | 4 | Async HTTP server |
| Image processing | image | 0.24.6 | Load, decode, encode PNG |
| Image operations | imageproc | 0.24 | Resize, pixel manipulation |
| Font rendering | ab_glyph | (via imageproc) | Render watermark text to image |
| HTTP client | reqwest | 0.11.18 | Download images from URLs |
| Serialization | serde / serde_json | 1.0 | Parse JSON request payloads |
| Async streams | futures | 0.3 | Stream response chunks |
| Base64 decoding | base64 | 0.21 | Decode embedded image data |
| Error handling | anyhow | 1.0.71 | Contextual error types |

---

## Directory Structure

```
IZDU-Slicer/
├── src/
│   ├── main.rs                  # HTTP server entry point, /slice and /watermark handlers
│   └── image_processor/
│       ├── mod.rs               # Request dispatch: source detection, image loading, slicing orchestration
│       ├── image_slicer.rs      # Core slicing logic (view-based quadrant split)
│       └── watermark.rs         # Text rendering and overlay
├── resources/
│   ├── OpenSans-Regular.ttf     # Embedded font for watermark text (SIL Open Font License)
│   └── LICENSE.md               # Font license
├── Cargo.toml                   # Project manifest & dependencies
├── Cargo.lock                   # Locked dependency versions
├── LICENSE.md                   # GPLv3
├── README.md                    # Project readme
└── .gitignore                   # Ignores /target, *.png, *.jpg, etc.
```

---

## Key Components

### `src/main.rs` — HTTP Server

The application entry point. Bootstraps an actix-web `HttpServer` that listens on port `9090` (configurable via `PORT` env var).

Four endpoints are registered:

#### `POST /slice`

The primary endpoint. Accepts an image and returns 4 sliced quadrants.

**Request body** — JSON:
```json
{ "image_url": "https://example.com/image.png" }
```
or
```json
{ "image_base64": "<base64 string>" }
```

**Query parameters** (all optional):
- `scale` — target size in pixels (0 = no scaling). Images larger than this will be downscaled to fit within `scale × scale`. Aspect ratio is preserved using `Nearest` filter.
- `watermark` — text string to render as a watermark on each slice.
- `transparency` — watermark opacity (0–100), defaults to 30.

**Response** — `application/octet-stream`. A stream of raw PNG bytes for each of the 4 slices, one after another.

#### `POST /watermark`

Applies a text watermark to an image and returns the watermarked result as a single PNG. Same input sources as `/slice` (`image_url`, `image_base64`, or raw binary).

#### `POST /crop`

Crops an image using four points in image pixel coordinates, with origin at the top-left pixel. The current implementation requires the points to form an axis-aligned rectangle:

- A (`ax`, `ay`) — top-left
- B (`bx`, `by`) — top-right
- C (`cx`, `cy`) — bottom-left
- D (`dx`, `dy`) — bottom-right

All points must be within the image bounds, and the resulting crop width and height must be greater than zero. The response is a single `image/png`.

---

### `src/image_processor/mod.rs` — Image Processing Orchestrator

Handles all image loading and dispatch logic.

**`ImageSource` enum** — three possible input types:
- `Url(String)` — image downloaded via `reqwest`
- `Binary(Vec<u8>)` — raw image bytes passed directly
- `Base64(String)` — base64-encoded string decoded to bytes

**`get_source(req, body)`** — inspects the request's `Content-Type` header:
- `application/json` → parses `ImagePayload` for `image_url` or `image_base64`
- `image/*` or `application/octet-stream` or non-empty body → treats body as raw binary image data

**`load_image(source)`** — dispatches to the correct loader based on source type:
- URL → `download_image()` via reqwest
- Binary → `load_from_bytes()`
- Base64 → `load_from_base64()`

**`slice(source, scale_px)`** — main slicing pipeline:
1. Load image from source
2. Compute single-slice dimensions (`width/2 × height/2`)
3. Slice into 4 quadrants via `image_slicer::slice_images_view()`
4. If `scale_px > 0` and smaller than the slice dimensions, resize with `Nearest` filter
5. Return `[ImageBuffer<Rgba<u8>, Vec<u8>>; 4]`

**`slice_with_watermark_text(source, scale_px, text, transparency)`** — same as above, but renders `text` as a watermark using `watermark::create_watermark()` and overlays it onto each slice before optional resizing.

**`crop_image(source, a, b, c, d)`** — loads an image source, validates the four crop points, and returns a cropped `DynamicImage`.

---

### `src/image_processor/image_slicer.rs` — Core Slicing

**`Dimension` struct** — holds computed `{ width, height, smallest }` for a single quadrant.

**`get_single_image_dimensions(img)`** — halves width and height of the source image.

**`initialize_output(w, h)`** — allocates 4 empty `ImageBuffer<Rgba<u8>, Vec<u8>>` buffers of size `w × h`.

**`slice_images_view(img, size)`** — the active slicing implementation:
- Uses `GenericImageView::view()` to create sub-views of the source image (no pixel copying)
- Maps each sub-view to one of the 4 output buffers via `to_image()`
- Quadrant layout:
  - Slice 0: top-left   (x=0,              y=0)
  - Slice 1: top-right  (x=width,          y=0)
  - Slice 2: bottom-left   (x=0,           y=height)
  - Slice 3: bottom-right (x=width,        y=height)

**`slice_images_copy_px(img, size)`** — legacy pixel-by-pixel copy implementation. Kept for reference; unused.

**`resize(images, size)`** — resizes all 4 image buffers to `size × size` using `FilterType::Nearest`.

**`crop_image(img, a, b, c, d)`** — validates that all four points are in bounds, axis-aligned, and produce a non-empty crop rectangle, then returns `img.crop_imm(x, y, width, height)`.

---

### `src/image_processor/watermark.rs` — Watermark Rendering

**`create_watermark(text, size)`** — renders `text` to an RGBA image using the embedded OpenSans font:
- Uses `ab_glyph::FontRef` to rasterize glyphs to pixel positions
- Returns a `DynamicImage` scaled to fill the full slice dimensions
- The watermark image contains white glyphs on a transparent background

**`add_watermark(img, watermark, alpha)`** — alpha-blends the watermark onto each slice:
- Positions watermark centered on the slice
- Applies per-pixel alpha composite of watermark pixels over the image pixels
- `alpha` parameter controls overall opacity (blended with per-pixel alpha from the rendered text)

---

## Data Flow

```
Client POST /slice
    │
    ▼
main.rs: slice() handler
    │
    ├─ get_source(req, body)          ──► ImageSource::{Url, Binary, Base64}
    │
    ├─ load_image(source)              ──► DynamicImage
    │       │
    │       ├─ download_image()        (reqwest HTTP GET)
    │       ├─ load_from_bytes()      (image::load_from_memory)
    │       └─ load_from_base64()      (base64 decode → load_from_memory)
    │
    ├─ get_single_image_dimensions()  ──► Dimension { w/2, h/2, min(w/2, h/2) }
    │
    ├─ slice_images_view()            ──► [ImageBuffer; 4]  (4 quadrants via sub-views)
    │
    ├─ create_watermark() + add_watermark()  (if watermark param provided)
    │
    ├─ resize()                       (if scale_px > 0 and smaller than slice)
    │
    └─ write_to(PNG)  →  stream each buffer as a chunk
            │
            ▼
    HttpResponse (application/octet-stream)
            │
            ▼
    Client reads bytes, finds PNG headers ([0x89, 0x50, 0x4E, 0x47]),
    splits into 4 images, and reassembles to original dimensions
```

---

## Response Streaming

The 4 PNG images are streamed sequentially as `Bytes` chunks. The caller must split the byte stream by locating PNG file signatures.

**PNG signature** (identical for all 4 slices):
- **Hex:** `[0x89, 0x50, 0x4E, 0x47]`
- **Decimal:** `[137, 80, 78, 71]`

The last image has no terminating marker — it ends when the stream closes.

---

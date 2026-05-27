# Product

## What It Does

IZDU Slicer (Image Zero Disk Usage Slicer) is an HTTP service that **splits one image into 4 equally-sized quadrants** — top-left, top-right, bottom-left, and bottom-right — and streams them back to the caller as raw PNG bytes. All processing happens in memory; nothing is written to disk.

This is useful anywhere an image needs to be tiled or distributed across multiple surfaces (displays, frames, print panels) where the original aspect ratio must be preserved across the full set of tiles.

---

## Who It's For

- **Digital display systems** — content creators who need to split a single image into tiles for a multi-panel display or video wall
- **E-commerce** — splitting product photography for use across multiple catalog surfaces
- **Print production** — pre-press workflows that need quadrant-separated image files
- **Developers building tiled-image tooling** — as a reusable microservice component

---

## Features

### Core

- **Image slicing** — split one image into 4 equal quadrants, each `width/2 × height/2`
- **URL input** — pass an image by HTTP URL in the JSON body
- **Base64 input** — pass an image as a base64-encoded string
- **Binary input** — send raw image bytes directly (no wrapping JSON)
- **PNG output** — all slices encoded as PNG
- **Rectangular cropping** — extract an axis-aligned region using 4 image-space points

### Resizing

- **Optional downscaling** — pass `?scale=300` to resize each slice to fit within a target pixel size
- `scale=0` disables resizing
- Uses `Nearest` filter for speed (appropriate for pixel-art or sharp-edged content)

### Watermarking

- **Text watermark** — pass `?watermark=my_text` to stamp each slice with custom text
- **Configurable opacity** — `?transparency=0` (fully opaque) to `?transparency=100` (fully invisible), default 30
- Uses the Open Sans font (bundled, SIL OFL license)
- Watermark text is rendered at a fixed font size and scaled to fit the slice dimensions
- Centered on each slice individually

### Streaming

- Responses are streamed as chunks (not buffered in full)
- Caller receives a continuous byte stream and splits it by locating PNG file signatures

### Cropping

- **4-point crop** — pass A, B, C, and D corner coordinates in image pixel space
- **Top-left origin** — `x` increases left to right, `y` increases top to bottom
- **Validation** — all points must be within image bounds, form an axis-aligned rectangle, and produce a non-empty crop area
- **PNG output** — returns one cropped PNG image

---

## Use Cases

### Tile a photo for a 2×2 display

```
Original: 1920×1080
Slices:   960×540  × 4
```

Upload the source image once; receive 4 tiles that fill the same total area when arranged back into a 2×2 grid.

### Resize + tile in one step

```
Original: 5120×2880  →  scale=400  →  slices: 400×400 × 4
```

Useful for generating thumbnails or preview tiles without a separate resizing step.

### Add watermark to all tiles

```
?scale=300&watermark=©+MyBrand&transparency=20
```

Each of the 4 slices gets the same centered watermark, ensuring brand coverage across all output tiles.

### Use in CI/CD pipelines

Call the `/slice` endpoint from a script to automate image tiling as part of an asset processing pipeline.

### Extract a region of interest

Crop a known rectangular area from an image without writing intermediate files:

```
/crop?ax=100&ay=50&bx=500&by=50&cx=100&cy=350&dx=500&dy=350
```

---

## Command-Line Interface

There is **no built-in CLI**. A separate Go-based CLI client is available for testing and integration:

**Repository:** [github.com/eli-l/IZDU-slicer-test-client](https://github.com/eli-l/IZDU-slicer-test-client)

Pre-compiled binaries are available in the [latest release](https://github.com/eli-l/IZDU-slicer-test-client/releases).

---

## HTTP API Reference

### `POST /slice`

Slice an image and stream 4 PNG quadrants back.

**Request body** — one of:

```json
{ "image_url": "https://example.com/image.png" }
```

```json
{ "image_base64": "iVBORw0KGgoAAAANSUhEUgAA..." }
```

Or send raw image bytes directly with any appropriate `Content-Type`.

**Query parameters** (all optional):

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `scale` | integer | 300 | Target size in px (0 = no scaling) |
| `watermark` | string | — | Text to render as watermark |
| `transparency` | integer | 30 | Watermark opacity 0–100 (0=opaque, 100=invisible) |

**Response:** `application/octet-stream` — stream of 4 raw PNG byte sequences.

**Response parsing:**

Find each image by locating the PNG signature:
```
Hex:     [0x89, 0x50, 0x4E, 0x47]
Decimal: [137, 80, 78, 71]
```

Each slice is a complete, standalone PNG file. The last slice ends when the stream closes.

---

### `POST /watermark`

Dedicated watermark endpoint. Applies text to the provided image and returns the watermarked result as `image/png`.

**Query parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `text` | string | "IZDU-Slicer" | Watermark text |
| `transparency` | integer | 30 | Opacity 0–100 |

**Response:** `image/png` — watermarked PNG bytes.

Use `POST /slice?watermark=...` to watermark all four generated slices.

---

### `POST /crop`

Crop a rectangular region from an image and return it as `image/png`.

Coordinates are in image pixel space with origin `(0,0)` at the top-left pixel. The cropped rectangle uses half-open intervals `[A.x, B.x)` and `[A.y, C.y)`, so right and bottom edge coordinates may equal `image_width` and `image_height`. Coordinate ordering: `A.x < B.x` and `A.y < C.y`.

| Point | Params | Meaning |
|-------|--------|---------|
| A | `ax`, `ay` | Top-left |
| B | `bx`, `by` | Top-right |
| C | `cx`, `cy` | Bottom-left |
| D | `dx`, `dy` | Bottom-right |

Axis-aligned constraints: `A.x == C.x`, `A.y == B.y`, `B.x == D.x`, `C.y == D.y`.

**Response:** `image/png` — cropped PNG bytes.

---

## Configuration

| Environment Variable | Default | Description |
|---------------------|---------|-------------|
| `PORT` | `9090` | TCP port the server listens on |
| `GRPC_PORT` | `50051` | TCP port for the gRPC API |

**Running locally:**

```bash
PORT=8080 cargo run
```

---

## Input Format Support

The underlying `image` crate (v0.24.6) supports many formats at input. The primary output format is PNG.

The demo image in the repository shows the expected input → 4-output mapping:
- One image with a visual marker in each quadrant
- Slicing produces 4 images, each containing only the content from their quadrant

---

## Project Info

- **License:** GPLv3
- **Language:** Rust (2021 edition)
- **Version:** 0.2.0

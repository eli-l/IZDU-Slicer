#IZDU Slicer

Image Zero Disk Usage Slicer

## Usage
Runs on port :9090 by default.

Send `POST` to `:9090/` with the following JSON body:
```json
{
    "image_url": "<url to image>"
}
```

In return you'll get Response Stream, each chunk represents a single image, so make sure you save it.

## Testing
For testing purposes `test-client` is included (M1 macs and Linux AMD64).

Run:
`test-client-macos-arm64 http://localhost:9090/ <url to image>`
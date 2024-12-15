use std::sync::Arc;
use tokio::sync::Mutex;
use warp::{Filter, Reply, Rejection};
use raspicam::{Camera, CameraConfig, Exposure, ImageEffect};
use futures::stream::{Stream, StreamExt};
use bytes::Bytes;

#[tokio::main]
async fn main() {
    // Camera configuration
    let camera_config = CameraConfig::new()
        .width(640)
        .height(480)
        .framerate(30)
        .exposure(Exposure::Auto)
        .image_effect(ImageEffect::None);

    // Initialize camera
    let camera = Arc::new(Mutex::new(Camera::new(camera_config).expect("Failed to init camera")));

    // Stream route
    let stream_route = warp::path("stream")
        .and(warp::get())
        .and_then(move || {
            let camera_clone = Arc::clone(&camera);
            async move {
                // Create a server-sent events (SSE) stream
                let stream = tokio_stream::wrappers::IntervalStream::new(
                    tokio::time::interval(std::time::Duration::from_millis(33))
                )
                .map(move |_| {
                    let cam = camera_clone.clone();
                    async move {
                        let mut camera = cam.lock().await;
                        match camera.take_raw() {
                            Ok(frame) => {
                                // Convert raw frame to JPEG for streaming
                                let jpeg = convert_to_jpeg(&frame);
                                Some(Bytes::from(jpeg))
                            },
                            Err(_) => None
                        }
                    }
                })
                .buffered(1)
                .filter_map(|x| async { x });

                // Convert stream to Server-Sent Events
                let sse_stream = stream.map(|frame| {
                    Ok(warp::sse::data(base64::encode(frame)))
                });

                Ok::<_, Rejection>(warp::sse::reply(sse_stream))
            }
        });

    // Static files route
    let static_route = warp::fs::dir("./static");

    // Combine routes
    let routes = stream_route.or(static_route);

    // Start server
    println!("Starting camera stream server on http://localhost:3030");
    warp::serve(routes).run(([0, 0, 0, 0], 3030)).await;
}

// Helper function to convert raw frame to JPEG
fn convert_to_jpeg(raw_frame: &[u8]) -> Vec<u8> {
    // Note: This is a placeholder. You'll need to use an image processing library 
    // like image or imagemagick to convert raw frame to JPEG
    // Actual implementation depends on your specific camera frame format
    vec![]
}
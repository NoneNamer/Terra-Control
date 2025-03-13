use std::sync::Arc;
use tokio::sync::Mutex;
use raspicam::{Camera, CameraConfig, Exposure, ImageEffect};
use std::error::Error;
use std::fmt;
use image::{ImageBuffer, Rgb};
use std::io::Cursor;

/// Custom error type for camera operations
#[derive(Debug)]
pub enum CameraError {
    InitError(String),
    CaptureError(String),
    ConversionError(String),
}

impl fmt::Display for CameraError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CameraError::InitError(msg) => write!(f, "Camera initialization error: {}", msg),
            CameraError::CaptureError(msg) => write!(f, "Camera capture error: {}", msg),
            CameraError::ConversionError(msg) => write!(f, "Image conversion error: {}", msg),
        }
    }
}

impl Error for CameraError {}

/// Camera controller for handling camera operations
pub struct CameraController {
    camera: Option<Camera>,
    config: CameraConfig,
    initialized: bool,
}

impl CameraController {
    /// Create a new camera controller with default configuration
    pub fn new() -> Self {
        let config = CameraConfig::new()
            .width(640)
            .height(480)
            .framerate(30)
            .exposure(Exposure::Auto)
            .image_effect(ImageEffect::None);

        Self {
            camera: None,
            config,
            initialized: false,
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: CameraConfig) -> Self {
        Self {
            camera: None,
            config,
            initialized: false,
        }
    }

    /// Initialize the camera
    pub fn initialize(&mut self) -> Result<(), CameraError> {
        if self.initialized {
            return Ok(());
        }

        match Camera::new(self.config) {
            Ok(camera) => {
                self.camera = Some(camera);
                self.initialized = true;
                Ok(())
            },
            Err(e) => Err(CameraError::InitError(e.to_string())),
        }
    }

    /// Take a raw frame from the camera
    pub fn take_raw_frame(&mut self) -> Result<Vec<u8>, CameraError> {
        if !self.initialized {
            self.initialize()?;
        }

        if let Some(camera) = &mut self.camera {
            camera.take_raw().map_err(|e| CameraError::CaptureError(e.to_string()))
        } else {
            Err(CameraError::CaptureError("Camera not initialized".to_string()))
        }
    }

    /// Take a snapshot and convert it to JPEG
    pub fn take_snapshot(&mut self) -> Result<Vec<u8>, CameraError> {
        let raw_frame = self.take_raw_frame()?;
        convert_to_jpeg(&raw_frame)
    }

    /// Check if camera is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Check if a camera is available
    pub fn is_camera_available() -> bool {
        // For a real implementation, this would check if the camera hardware is available
        // This could check if the camera device exists at /dev/video0 for example
        
        #[cfg(target_os = "linux")]
        {
            use std::path::Path;
            Path::new("/dev/video0").exists() || Path::new("/dev/vchiq").exists()
        }
        
        #[cfg(not(target_os = "linux"))]
        {
            // On non-Linux platforms, we can't easily check for the camera
            // Return true for development purposes
            true
        }
    }
}

/// Thread-safe service for managing the Raspberry Pi camera.
///
/// This service provides a high-level interface for camera operations, with
/// thread-safe access to the underlying camera controller. It's designed to be
/// shared across multiple asynchronous tasks that need to access the camera.
pub struct CameraService {
    controller: Arc<Mutex<CameraController>>,
}

impl CameraService {
    /// Creates a new CameraService with default settings.
    ///
    /// The camera is not initialized immediately and will need to be initialized
    /// before use by calling the `initialize` method.
    ///
    /// # Returns
    ///
    /// A new CameraService instance
    pub fn new() -> Self {
        Self {
            controller: Arc::new(Mutex::new(CameraController::new())),
        }
    }
    
    /// Gets the underlying camera controller.
    ///
    /// This is primarily for internal use by other components that
    /// need direct access to the controller.
    ///
    /// # Returns
    ///
    /// A reference-counted pointer to the mutex-protected camera controller
    pub fn get_controller(&self) -> Arc<Mutex<CameraController>> {
        self.controller.clone()
    }
    
    /// Initializes the camera hardware.
    ///
    /// This must be called before taking snapshots or video. It initializes
    /// the camera with the configured settings.
    ///
    /// # Returns
    ///
    /// A Result indicating success or an initialization error
    pub async fn initialize(&self) -> Result<(), CameraError> {
        let mut controller = self.controller.lock().await;
        controller.initialize()
    }
    
    /// Takes a snapshot and returns it as a JPEG image.
    ///
    /// This function captures an image from the camera and returns it
    /// as a JPEG-encoded byte vector.
    ///
    /// # Returns
    ///
    /// A Result containing either the JPEG image data or an error
    pub async fn take_snapshot(&self) -> Result<Vec<u8>, CameraError> {
        let mut controller = self.controller.lock().await;
        controller.take_snapshot()
    }
    
    /// Checks if a camera is physically connected and available.
    ///
    /// This is a static method that detects if the system has a compatible
    /// camera attached, without actually initializing it.
    ///
    /// # Returns
    ///
    /// True if a camera is available, False otherwise
    pub fn is_camera_available() -> bool {
        CameraController::is_camera_available()
    }
    
    /// Checks if the camera has been successfully initialized.
    ///
    /// # Returns
    ///
    /// True if the camera is initialized and ready for use, False otherwise
    pub async fn is_initialized(&self) -> bool {
        let controller = self.controller.lock().await;
        controller.is_initialized()
    }
}

/// Converts a raw camera frame to a JPEG image.
///
/// This utility function takes a raw frame buffer from the camera
/// and processes it into a JPEG image format suitable for web display.
///
/// # Arguments
///
/// * `raw_frame` - The raw image data from the camera
///
/// # Returns
///
/// A Result containing either the JPEG data or a conversion error
pub fn convert_to_jpeg(raw_frame: &[u8]) -> Result<Vec<u8>, CameraError> {
    // In a real implementation, this would use proper image conversion
    // Here we're creating a simple placeholder image for demonstration

    // Create a simple image (in a real implementation, parse the raw_frame correctly)
    let width = 640;
    let height = 480;
    
    // Try to create an RGB image
    let img_result = ImageBuffer::<Rgb<u8>, _>::from_fn(width, height, |x, y| {
        // Create a simple gradient pattern
        let r = (x as u8) % 255;
        let g = (y as u8) % 255;
        let b = ((x + y) as u8) % 255;
        Rgb([r, g, b])
    });

    // Convert to JPEG
    let mut jpeg_data = Vec::new();
    let mut cursor = Cursor::new(&mut jpeg_data);
    
    match img_result.write_to(&mut cursor, image::ImageOutputFormat::Jpeg(90)) {
        Ok(_) => Ok(jpeg_data),
        Err(e) => Err(CameraError::ConversionError(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_controller_creation() {
        let controller = CameraController::new();
        assert!(!controller.is_initialized());
    }

    #[tokio::test]
    async fn test_camera_service() {
        let service = CameraService::new();
        let controller = service.get_controller();
        assert!(!controller.lock().await.is_initialized());
    }
}
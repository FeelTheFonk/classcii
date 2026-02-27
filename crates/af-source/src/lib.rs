/// Visual source modules for clasSCII (image, video, webcam, procedural).
pub mod image;
pub mod resize;

#[cfg(feature = "procedural")]
pub mod procedural;
#[cfg(feature = "video")]
pub mod video;
#[cfg(feature = "webcam")]
pub mod webcam;

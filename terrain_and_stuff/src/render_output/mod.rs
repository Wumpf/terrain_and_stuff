//! Handling the rendering output pipeline
//! -> HDR, display transform (tonemapping), screenshot capturing etc.

mod hdr_backbuffer;
mod screen;

pub use hdr_backbuffer::HdrBackbuffer;
pub use screen::Screen;

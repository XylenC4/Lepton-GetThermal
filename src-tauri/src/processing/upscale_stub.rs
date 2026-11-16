//! Android: ONNX Runtime is not bundled, so neural upscaling is unavailable.

pub struct Upscaler {
    pub scale: usize,
}

impl Upscaler {
    pub fn new() -> Result<Self, String> {
        Err("AI upscaling is not available on Android".into())
    }

    #[allow(dead_code)]
    pub fn from_file(_path: &str) -> Result<Self, String> {
        Self::new()
    }

    pub fn upscale(&mut self, _gs: &[u8], _w: usize, _h: usize) -> Result<(Vec<u8>, usize, usize), String> {
        Err("unavailable".into())
    }
}

use std::sync::Arc;

mod hash;

#[cfg(feature = "http")]
mod providers;

pub use hash::{FastEmbedOnnxEmbedder, HashEmbedder};

#[cfg(feature = "http")]
pub use providers::{OllamaEmbedder, OpenAiCompatibleEmbedder};

use crate::Result;

pub trait Embedder: Send + Sync {
    fn name(&self) -> &'static str;
    fn dimensions(&self) -> usize;
    fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

pub type SharedEmbedder = Arc<dyn Embedder>;

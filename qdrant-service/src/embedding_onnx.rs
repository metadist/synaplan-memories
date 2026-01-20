#![cfg(feature = "native_onnx")]

use crate::config::Config;
use crate::embedding::Embedder;
use crate::error::AppError;
use async_trait::async_trait;
use std::sync::Arc;
use tokenizers::Tokenizer;

/// Native ONNX Runtime embedder.
///
/// Notes:
/// - Uses tokenizer.json for XLM-R based tokenizer.
/// - Runs ONNX model and returns dense embedding vector (1024 floats for bge-m3).
/// - Uses CLS pooling (first token) + L2 normalization.
pub struct OnnxRuntimeEmbedder {
    tokenizer: Tokenizer,
    session: Arc<ort::Session>,
    model: String,
    device: String,
}

impl OnnxRuntimeEmbedder {
    pub fn try_new(config: &Config, model_name: String, device: String) -> anyhow::Result<Self> {
        let onnx_path = config
            .embedding_onnx_model_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("EMBEDDING_ONNX_MODEL_PATH is required for native_onnx"))?;
        let tokenizer_path = config
            .embedding_tokenizer_path
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("EMBEDDING_TOKENIZER_PATH is required for native_onnx"))?;

        // Load tokenizer
        let tokenizer =
            Tokenizer::from_file(tokenizer_path).map_err(|e| anyhow::anyhow!("Tokenizer load failed: {e}"))?;

        // Load ORT dynamic library (the container must provide libonnxruntime.so)
        // `ort` will use ORT_DYLIB_PATH or system loader.
        ort::init()
            .commit()
            .map_err(|e| anyhow::anyhow!("ORT init failed: {e}"))?;

        // Build session
        let mut builder = ort::Session::builder()
            .map_err(|e| anyhow::anyhow!("ORT session builder failed: {e}"))?;

        // If the runtime has CUDA EP, it will be used automatically when the CUDA-enabled ORT lib is present.
        // We keep device selection in capabilities only for now.
        builder = builder
            .with_optimization_level(ort::GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow::anyhow!("ORT optimization level failed: {e}"))?;

        let session = builder
            .commit_from_file(onnx_path)
            .map_err(|e| anyhow::anyhow!("ORT load model failed: {e}"))?;

        Ok(Self {
            tokenizer,
            session: Arc::new(session),
            model: model_name,
            device,
        })
    }
}

#[async_trait]
impl Embedder for OnnxRuntimeEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, AppError> {
        if text.trim().is_empty() {
            return Err(AppError::InvalidRequest("Text must not be empty".to_string()));
        }

        // Tokenize
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| AppError::Internal(format!("Tokenization failed: {e}")))?;

        let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&x| x as i64)
            .collect();

        if ids.is_empty() {
            return Err(AppError::Internal("Tokenization returned empty ids".to_string()));
        }

        // Create inputs: [1, seq_len]
        let seq_len = ids.len();
        let input_ids =
            ort::value::Value::from_array((vec![1_i64, seq_len as i64], ids)).map_err(|e| {
                AppError::Internal(format!("Failed to build input_ids tensor: {e}"))
            })?;
        let attention_mask =
            ort::value::Value::from_array((vec![1_i64, seq_len as i64], mask)).map_err(|e| {
                AppError::Internal(format!("Failed to build attention_mask tensor: {e}"))
            })?;

        // Run model. We assume standard transformer input names.
        // If your exported ONNX uses different names, we should add config options.
        let outputs = self
            .session
            .run(ort::inputs! {
                "input_ids" => input_ids,
                "attention_mask" => attention_mask,
            })
            .map_err(|e| AppError::Internal(format!("ORT inference failed: {e}")))?;

        // Take first output as last_hidden_state: [1, seq_len, hidden]
        let output0 = outputs
            .get(0)
            .ok_or_else(|| AppError::Internal("No outputs returned from model".to_string()))?;

        let tensor = output0
            .try_extract_tensor::<f32>()
            .map_err(|e| AppError::Internal(format!("Failed to extract output tensor: {e}")))?;

        let shape = tensor.shape();
        if shape.len() != 3 || shape[0] != 1 || shape[1] != seq_len {
            return Err(AppError::Internal(format!(
                "Unexpected output shape: {:?} (expected [1, seq_len, hidden])",
                shape
            )));
        }

        let hidden = shape[2] as usize;
        let data = tensor.as_slice().ok_or_else(|| {
            AppError::Internal("Output tensor is not contiguous".to_string())
        })?;

        // CLS pooling (first token at position 0)
        let mut emb = vec![0f32; hidden];
        let base = 0usize; // token 0
        for i in 0..hidden {
            emb[i] = data[base * hidden + i];
        }

        // L2 normalize
        let norm = emb.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>().sqrt();
        if norm > 0.0 {
            let inv = (1.0 / norm) as f32;
            for v in emb.iter_mut() {
                *v *= inv;
            }
        }

        Ok(emb)
    }

    fn backend(&self) -> String {
        "onnxruntime".to_string()
    }

    fn model(&self) -> Option<String> {
        Some(self.model.clone())
    }

    fn device(&self) -> String {
        self.device.clone()
    }
}



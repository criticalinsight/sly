use anyhow::{anyhow, Result};
use candle_core::{DType, Device, Tensor, IndexOp};
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{api::sync::Api, Repo, RepoType};
use tokenizers::Tokenizer;

pub struct EmbeddingEngine {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl EmbeddingEngine {
    pub fn new() -> Result<Self> {
        // Initialize Candle with Metal support (MacOS GPU)
        let device = Device::new_metal(0).unwrap_or(Device::Cpu);
        println!("🧠 Initializing Embedding Engine on device: {:?}", device);

        // Load model from HF Hub (BGE-Small-en-v1.5)
        let model_id = "BAAI/bge-small-en-v1.5".to_string();
        let api = Api::new()?;
        let repo = api.repo(Repo::new(model_id, RepoType::Model));

        let config_filename = repo.get("config.json")?;
        let tokenizer_filename = repo.get("tokenizer.json")?;
        let weights_filename = repo.get("model.safetensors")?;

        let config: Config = serde_json::from_str(&std::fs::read_to_string(config_filename)?)?;
        let mut tokenizer = Tokenizer::from_file(tokenizer_filename).map_err(|e| anyhow!(e))?;

        if let Some(pp) = tokenizer.get_padding_mut() {
            pp.strategy = tokenizers::PaddingStrategy::BatchLongest;
        }

        // Attempt to load and test the model on the chosen device
        let (model, final_device) = match (|| -> Result<(BertModel, Device)> {
            let tensors = candle_core::safetensors::load(&weights_filename, &device)?;
            let vb = candle_nn::VarBuilder::from_tensors(tensors, DType::F32, &device);
            let model = BertModel::load(vb, &config)?;
            
            // Diagnostic check: dummy forward pass
            let dummy_input = Tensor::zeros((1, 1), DType::U32, &device)?;
            let dummy_token_type = Tensor::zeros((1, 1), DType::U32, &device)?;
            model.forward(&dummy_input, &dummy_token_type, None)?;
            
            Ok((model, device.clone()))
        })() {
            Ok(res) => res,
            Err(e) => {
                if device.is_metal() {
                    println!("Metal implementation incomplete for this model ({}). Falling back to CPU.", e);
                    let cpu_device = Device::Cpu;
                    let tensors = candle_core::safetensors::load(&weights_filename, &cpu_device)?;
                    let vb = candle_nn::VarBuilder::from_tensors(tensors, DType::F32, &cpu_device);
                    let model = BertModel::load(vb, &config)?;
                    (model, cpu_device)
                } else {
                    return Err(e);
                }
            }
        };

        Ok(Self {
            model,
            tokenizer,
            device: final_device,
        })
    }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let tokens = self.tokenizer.encode(text, true).map_err(|e| anyhow!(e))?;
        let token_ids = Tensor::new(tokens.get_ids(), &self.device)?.unsqueeze(0)?;
        let token_type_ids = Tensor::new(tokens.get_type_ids(), &self.device)?.unsqueeze(0)?;

        let embeddings = self.model.forward(&token_ids, &token_type_ids, None)?;
        let cls_embedding = embeddings.get_on_dim(1, 0)?;
        
        // Manual L2 Norm on Tensor (keeps it on device)
        let normalized = normalize_tensor_l2(&cls_embedding)?;
        let vector: Vec<f32> = normalized.flatten_all()?.to_vec1()?;

        Ok(vector)
    }

    pub fn batch_embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        // 1. Tokenize all texts
        let tokens = self.tokenizer.encode_batch(texts.to_vec(), true)
            .map_err(|e| anyhow!(e))?;

        // 2. Prepare tensors
        let token_ids_vec: Vec<u32> = tokens.iter()
            .flat_map(|t| t.get_ids().to_vec())
            .collect();
        let token_type_ids_vec: Vec<u32> = tokens.iter()
            .flat_map(|t| t.get_type_ids().to_vec())
            .collect();

        let batch_size = texts.len();
        let seq_len = tokens[0].get_ids().len();

        let token_ids = Tensor::from_vec(token_ids_vec, (batch_size, seq_len), &self.device)?;
        let token_type_ids = Tensor::from_vec(token_type_ids_vec, (batch_size, seq_len), &self.device)?;

        // 3. Forward pass
        let embeddings = self.model.forward(&token_ids, &token_type_ids, None)?;
        
        // 4. Extract CLS embeddings (first token)
        // embeddings is [batch_size, seq_len, hidden_size]
        let cls_embeddings = embeddings.i((.., 0, ..))?;
        
        // 5. Normalize
        let mut results = Vec::new();
        for i in 0..batch_size {
            let row = cls_embeddings.i(i)?;
            let normalized = normalize_tensor_l2(&row)?;
            results.push(normalized.flatten_all()?.to_vec1()?);
        }

        Ok(results)
    }
}

// Helper for L2 Normalization (pure calculation)
fn normalize_tensor_l2(v: &Tensor) -> Result<Tensor> {
    // v is shape [hidden_size]
    let sq_sum = v.sqr()?.sum_all()?;
    let norm = sq_sum.sqrt()?;
    let normalized = v.broadcast_div(&norm)?;
    Ok(normalized)
}

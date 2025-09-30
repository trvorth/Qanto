use crate::qanto_standalone::hash::QantoHash;
use crate::qanhash::{Difficulty, Target, MIX_BYTES, get_qdag, is_solution_valid};
use log::{info, warn, error};
use std::sync::Arc;

#[cfg(feature = "cuda")]
use cudarc::driver::{CudaDevice, DriverError, LaunchAsync, LaunchConfig};
#[cfg(feature = "cuda")]
use cudarc::nvrtc::compile_ptx;

#[cfg(feature = "cuda")]
pub struct CudaGpuContext {
    device: Arc<CudaDevice>,
    module: cudarc::driver::CudaModule,
    kernel_name: String,
}

#[cfg(feature = "cuda")]
impl CudaGpuContext {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Qanhash-CUDA] Initializing CUDA GPU context...");
        
        // Initialize CUDA device
        let device = CudaDevice::new(0)?; // Use first GPU
        
        info!("[Qanhash-CUDA] Using device: {}", device.name());
        
        // CUDA kernel source for Qanhash algorithm
        let kernel_source = include_str!("qanhash.cu");
        
        // Compile CUDA kernel
        let ptx = compile_ptx(kernel_source)?;
        let module = device.load_ptx(ptx, "qanhash_module", &["qanhash_kernel"])?;
        
        info!("[Qanhash-CUDA] CUDA GPU context initialized successfully");
        
        Ok(CudaGpuContext {
            device,
            module,
            kernel_name: "qanhash_kernel".to_string(),
        })
    }
    
    pub fn hash_batch(
        &self,
        header_hash: &QantoHash,
        start_nonce: u64,
        batch_size: usize,
        target: Target,
    ) -> Result<Option<(u64, [u8; 32])>, Box<dyn std::error::Error>> {
        let dag = get_qdag(0); // Use epoch 0 for now
        let dag_len_mask = dag.len() - 1;
        
        // Allocate GPU memory
        let header_gpu = self.device.htod_copy(header_hash.as_bytes().to_vec())?;
        let dag_gpu = self.device.htod_copy(dag.as_slice())?;
        let target_gpu = self.device.htod_copy(target.to_vec())?;
        
        // Result buffers
        let mut result_nonce = vec![0u64; 1];
        let mut result_hash = vec![0u8; 32];
        let mut result_found = vec![0u32; 1];
        
        let result_nonce_gpu = self.device.htod_copy(result_nonce.clone())?;
        let result_hash_gpu = self.device.htod_copy(result_hash.clone())?;
        let result_found_gpu = self.device.htod_copy(result_found.clone())?;
        
        // Launch configuration
        let threads_per_block = 256;
        let blocks = (batch_size + threads_per_block - 1) / threads_per_block;
        
        let cfg = LaunchConfig {
            grid_dim: (blocks as u32, 1, 1),
            block_dim: (threads_per_block as u32, 1, 1),
            shared_mem_bytes: 0,
        };
        
        // Launch kernel
        let kernel = self.module.get_func(&self.kernel_name)?;
        unsafe {
            kernel.launch(
                cfg,
                (
                    &header_gpu,
                    &start_nonce,
                    &dag_gpu,
                    &(dag_len_mask as u64),
                    &target_gpu,
                    &result_nonce_gpu,
                    &result_hash_gpu,
                    &result_found_gpu,
                ),
            )?;
        }
        
        // Synchronize and copy results back
        self.device.synchronize()?;
        
        self.device.dtoh_sync_copy_into(&result_found_gpu, &mut result_found)?;
        
        if result_found[0] != 0 {
            self.device.dtoh_sync_copy_into(&result_nonce_gpu, &mut result_nonce)?;
            self.device.dtoh_sync_copy_into(&result_hash_gpu, &mut result_hash)?;
            
            let winning_nonce = result_nonce[0];
            let mut final_hash = [0u8; 32];
            final_hash.copy_from_slice(&result_hash);
            
            return Ok(Some((winning_nonce, final_hash)));
        }
        
        Ok(None)
    }
    
    pub fn get_device_info(&self) -> String {
        format!("CUDA GPU: {}", self.device.name())
    }
}

#[cfg(not(feature = "cuda"))]
pub struct CudaGpuContext;

#[cfg(not(feature = "cuda"))]
impl CudaGpuContext {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Err("CUDA GPU support is not enabled. Compile with --features cuda".into())
    }
    
    pub fn hash_batch(
        &self,
        _header_hash: &QantoHash,
        _start_nonce: u64,
        _batch_size: usize,
        _target: Target,
    ) -> Result<Option<(u64, [u8; 32])>, Box<dyn std::error::Error>> {
        Err("CUDA GPU support is not enabled".into())
    }
    
    pub fn get_device_info(&self) -> String {
        "CUDA GPU not available (feature not enabled)".to_string()
    }
}

// Lazy static for CUDA GPU context
use std::sync::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref CUDA_GPU_CONTEXT: Mutex<Option<CudaGpuContext>> = {
        match CudaGpuContext::new() {
            Ok(context) => {
                info!("[Qanhash-CUDA] CUDA GPU context initialized successfully");
                Mutex::new(Some(context))
            }
            Err(e) => {
                warn!("[Qanhash-CUDA] Failed to initialize CUDA GPU context: {}", e);
                Mutex::new(None)
            }
        }
    };
}

pub fn is_cuda_available() -> bool {
    #[cfg(feature = "cuda")]
    {
        CUDA_GPU_CONTEXT.lock().unwrap().is_some()
    }
    #[cfg(not(feature = "cuda"))]
    {
        false
    }
}

pub fn cuda_hash_batch(
    header_hash: &QantoHash,
    start_nonce: u64,
    batch_size: usize,
    target: Target,
) -> Result<Option<(u64, [u8; 32])>, Box<dyn std::error::Error>> {
    let context_guard = CUDA_GPU_CONTEXT.lock().unwrap();
    if let Some(ref context) = *context_guard {
        context.hash_batch(header_hash, start_nonce, batch_size, target)
    } else {
        Err("CUDA GPU context not available".into())
    }
}
use crate::qanto_standalone::hash::QantoHash;
use crate::qanhash::{Difficulty, Target, MIX_BYTES, get_qdag, is_solution_valid};
use log::{info, warn, error};
use std::sync::Arc;

#[cfg(target_os = "macos")]
use metal::*;

#[cfg(target_os = "macos")]
pub struct MetalGpuContext {
    device: Device,
    command_queue: CommandQueue,
    library: Library,
    compute_pipeline: ComputePipelineState,
}

#[cfg(target_os = "macos")]
impl MetalGpuContext {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        info!("[Qanhash-Metal] Initializing Metal GPU context...");
        
        let device = Device::system_default()
            .ok_or("No Metal-compatible device found")?;
        
        info!("[Qanhash-Metal] Using device: {}", device.name());
        
        let command_queue = device.new_command_queue();
        
        // Metal shader source for Qanhash algorithm
        let shader_source = include_str!("qanhash.metal");
        let library = device.new_library_with_source(shader_source, &CompileOptions::new())?;
        
        let kernel_function = library.get_function("qanhash_kernel", None)?;
        let compute_pipeline = device.new_compute_pipeline_state_with_function(&kernel_function)?;
        
        info!("[Qanhash-Metal] Metal GPU context initialized successfully");
        
        Ok(MetalGpuContext {
            device,
            command_queue,
            library,
            compute_pipeline,
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
        
        // Create Metal buffers
        let header_buffer = self.device.new_buffer_with_data(
            header_hash.as_bytes().as_ptr() as *const std::ffi::c_void,
            header_hash.as_bytes().len() as u64,
            MTLResourceOptions::StorageModeShared,
        );
        
        let dag_buffer = self.device.new_buffer_with_data(
            dag.as_ptr() as *const std::ffi::c_void,
            (dag.len() * MIX_BYTES) as u64,
            MTLResourceOptions::StorageModeShared,
        );
        
        let target_buffer = self.device.new_buffer_with_data(
            target.as_ptr() as *const std::ffi::c_void,
            target.len() as u64,
            MTLResourceOptions::StorageModeShared,
        );
        
        // Result buffers
        let result_nonce_buffer = self.device.new_buffer(
            8, // u64 size
            MTLResourceOptions::StorageModeShared,
        );
        
        let result_hash_buffer = self.device.new_buffer(
            32, // 32 bytes for hash
            MTLResourceOptions::StorageModeShared,
        );
        
        let result_found_buffer = self.device.new_buffer(
            4, // u32 size for found flag
            MTLResourceOptions::StorageModeShared,
        );
        
        // Initialize result buffers
        unsafe {
            let found_ptr = result_found_buffer.contents() as *mut u32;
            *found_ptr = 0; // Not found initially
        }
        
        // Create command buffer and encoder
        let command_buffer = self.command_queue.new_command_buffer();
        let encoder = command_buffer.new_compute_command_encoder();
        
        encoder.set_compute_pipeline_state(&self.compute_pipeline);
        
        // Set arguments
        encoder.set_buffer(0, Some(&header_buffer), 0);
        encoder.set_bytes(1, 8, &start_nonce.to_le_bytes() as *const _ as *const std::ffi::c_void);
        encoder.set_buffer(2, Some(&dag_buffer), 0);
        encoder.set_bytes(3, 8, &(dag_len_mask as u64).to_le_bytes() as *const _ as *const std::ffi::c_void);
        encoder.set_buffer(4, Some(&target_buffer), 0);
        encoder.set_buffer(5, Some(&result_nonce_buffer), 0);
        encoder.set_buffer(6, Some(&result_hash_buffer), 0);
        encoder.set_buffer(7, Some(&result_found_buffer), 0);
        
        // Calculate thread group sizes
        let threads_per_group = MTLSize::new(256, 1, 1);
        let thread_groups = MTLSize::new(
            (batch_size + 255) / 256, // Round up division
            1,
            1,
        );
        
        encoder.dispatch_thread_groups(thread_groups, threads_per_group);
        encoder.end_encoding();
        
        // Execute and wait
        command_buffer.commit();
        command_buffer.wait_until_completed();
        
        // Check results
    unsafe {
        let found_ptr = result_found_buffer.contents() as *const u32;
        if *found_ptr != 0 {
            let nonce_ptr = result_nonce_buffer.contents() as *const u64;
            let hash_ptr = result_hash_buffer.contents() as *const [u8; 32];
            let winning_nonce = *nonce_ptr;
            let final_hash = *hash_ptr;
            return Ok(Some((winning_nonce, final_hash)));
        }
    }
    Ok(None)
}

pub fn hash_batch_with_dag(
    &self,
    header_hash: &QantoHash,
    start_nonce: u64,
    batch_size: usize,
    target: Target,
    dag: &Vec<[u8; MIX_BYTES]>,
) -> Result<Option<(u64, [u8; 32])>, Box<dyn std::error::Error>> {
    let dag_len_mask = dag.len() - 1;

    // Create Metal buffers
    let header_buffer = self.device.new_buffer_with_data(
        header_hash.as_bytes().as_ptr() as *const std::ffi::c_void,
        header_hash.as_bytes().len() as u64,
        MTLResourceOptions::StorageModeShared,
    );

    let dag_buffer = self.device.new_buffer_with_data(
        dag.as_ptr() as *const std::ffi::c_void,
        (dag.len() * MIX_BYTES) as u64,
        MTLResourceOptions::StorageModeShared,
    );

    let target_buffer = self.device.new_buffer_with_data(
        target.as_ptr() as *const std::ffi::c_void,
        target.len() as u64,
        MTLResourceOptions::StorageModeShared,
    );

    // Result buffers
    let result_nonce_buffer = self.device.new_buffer(
        8, // u64 size
        MTLResourceOptions::StorageModeShared,
    );

    let result_hash_buffer = self.device.new_buffer(
        32, // 32 bytes for hash
        MTLResourceOptions::StorageModeShared,
    );

    let result_found_buffer = self.device.new_buffer(
        4, // u32 size for found flag
        MTLResourceOptions::StorageModeShared,
    );

    // Initialize result buffers
    unsafe {
        let found_ptr = result_found_buffer.contents() as *mut u32;
        *found_ptr = 0; // Not found initially
    }

    // Create command buffer and encoder
    let command_buffer = self.command_queue.new_command_buffer();
    let encoder = command_buffer.new_compute_command_encoder();

    encoder.set_compute_pipeline_state(&self.compute_pipeline);

    // Set arguments
    encoder.set_buffer(0, Some(&header_buffer), 0);
    encoder.set_bytes(1, 8, &start_nonce.to_le_bytes() as *const _ as *const std::ffi::c_void);
    encoder.set_buffer(2, Some(&dag_buffer), 0);
    encoder.set_bytes(3, 8, &(dag_len_mask as u64).to_le_bytes() as *const _ as *const std::ffi::c_void);
    encoder.set_buffer(4, Some(&target_buffer), 0);
    encoder.set_buffer(5, Some(&result_nonce_buffer), 0);
    encoder.set_buffer(6, Some(&result_hash_buffer), 0);
    encoder.set_buffer(7, Some(&result_found_buffer), 0);

    // Calculate thread group sizes
    let threads_per_group = MTLSize::new(256, 1, 1);
    let thread_groups = MTLSize::new(
        (batch_size + 255) / 256, // Round up division
        1,
        1,
    );

    encoder.dispatch_thread_groups(thread_groups, threads_per_group);
    encoder.end_encoding();

    // Execute and wait
    command_buffer.commit();
    command_buffer.wait_until_completed();

    // Check results
    unsafe {
        let found_ptr = result_found_buffer.contents() as *const u32;
        if *found_ptr != 0 {
            let nonce_ptr = result_nonce_buffer.contents() as *const u64;
            let hash_ptr = result_hash_buffer.contents() as *const [u8; 32];
            let winning_nonce = *nonce_ptr;
            let final_hash = *hash_ptr;
            return Ok(Some((winning_nonce, final_hash)));
        }
    }

    Ok(None)
}
    
    pub fn get_device_info(&self) -> String {
        format!("Metal GPU: {}", self.device.name())
    }
}

#[cfg(not(target_os = "macos"))]
pub struct MetalGpuContext;

#[cfg(not(target_os = "macos"))]
impl MetalGpuContext {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Err("Metal GPU support is only available on macOS".into())
    }
    
    pub fn hash_batch(
        &self,
        _header_hash: &QantoHash,
        _start_nonce: u64,
        _batch_size: usize,
        _target: Target,
    ) -> Result<Option<(u64, [u8; 32])>, Box<dyn std::error::Error>> {
        Err("Metal GPU support is only available on macOS".into())
    }
    
    pub fn get_device_info(&self) -> String {
        "Metal GPU not available on this platform".to_string()
    }
}

// Lazy static for Metal GPU context
use std::sync::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref METAL_GPU_CONTEXT: Mutex<Option<MetalGpuContext>> = {
        match MetalGpuContext::new() {
            Ok(context) => {
                info!("[Qanhash-Metal] Metal GPU context initialized successfully");
                Mutex::new(Some(context))
            }
            Err(e) => {
                warn!("[Qanhash-Metal] Failed to initialize Metal GPU context: {}", e);
                Mutex::new(None)
            }
        }
    };
}

pub fn is_metal_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        METAL_GPU_CONTEXT.lock().unwrap().is_some()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn metal_hash_batch(
    header_hash: &QantoHash,
    start_nonce: u64,
    batch_size: usize,
    target: Target,
) -> Result<Option<(u64, [u8; 32])>, Box<dyn std::error::Error>> {
    let context_guard = METAL_GPU_CONTEXT.lock().unwrap();
    if let Some(ref context) = *context_guard {
        context.hash_batch(header_hash, start_nonce, batch_size, target)
    } else {
        Err("Metal GPU context not available".into())
    }
}
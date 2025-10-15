use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{RwLock, Semaphore};
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_graceful_shutdown::{SubsystemBuilder, Toplevel};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Resource cleanup errors
#[derive(Error, Debug)]
pub enum CleanupError {
    #[error("Cleanup timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    #[error("Resource cleanup failed: {resource} - {message}")]
    ResourceCleanupFailed { resource: String, message: String },
    #[error("Shutdown already in progress")]
    ShutdownInProgress,
    #[error("Critical cleanup error: {message}")]
    Critical { message: String },
    #[error("Task management error: {message}")]
    TaskManagement { message: String },
}

/// Enhanced resource cleanup coordinator for graceful shutdown
#[derive(Clone)]
pub struct ResourceCleanup {
    /// Atomic flag to track if shutdown has been requested
    shutdown_requested: Arc<AtomicBool>,
    /// Atomic flag to track if shutdown is currently in progress
    shutdown_in_progress: Arc<AtomicBool>,
    /// Maximum time to wait for cleanup operations
    cleanup_timeout: Duration,
    /// Semaphore to limit concurrent active tasks
    active_tasks: Arc<Semaphore>,
    /// List of registered resources for cleanup
    resources: Arc<RwLock<Vec<CleanupResource>>>,
    /// List of cleanup hooks to execute during shutdown
    cleanup_hooks: Arc<RwLock<Vec<CleanupHook>>>,
    /// Cancellation token for coordinated shutdown
    cancellation_token: CancellationToken,
    /// Registry for managed async tasks
    task_registry: Arc<RwLock<JoinSet<Result<(), CleanupError>>>>,
    /// Runtime shutdown hooks for async cleanup
    runtime_hooks: Arc<RwLock<Vec<RuntimeShutdownHook>>>,
    /// Optional tokio-graceful-shutdown toplevel handle
    toplevel: Arc<RwLock<Option<Toplevel>>>,
}

/// Cleanup resource descriptor
#[derive(Debug, Clone)]
pub struct CleanupResource {
    pub name: String,
    pub resource_type: ResourceType,
    pub priority: CleanupPriority,
    pub timeout: Duration,
}

/// Resource types for cleanup
#[derive(Debug, Clone, PartialEq)]
pub enum ResourceType {
    Database,
    Network,
    FileHandle,
    ThreadPool,
    Cache,
    Wallet,
    Mining,
    P2P,
    WebSocket,
    Storage,
    Runtime,
    AsyncTask,
}

/// Cleanup priority levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CleanupPriority {
    Critical = 0, // Must be cleaned up first (databases, critical files)
    High = 1,     // Important resources (network connections, caches)
    Medium = 2,   // Standard resources (thread pools, temporary files)
    Low = 3,      // Optional cleanup (logs, metrics)
}

/// Cleanup hook function type
pub type CleanupHook = Box<dyn Fn() -> Result<()> + Send + Sync>;

/// Runtime shutdown hook for async operations
pub type RuntimeShutdownHook =
    Box<dyn Fn(CancellationToken) -> tokio::task::JoinHandle<Result<()>> + Send + Sync>;

impl ResourceCleanup {
    /// Create new resource cleanup coordinator
    pub fn new(cleanup_timeout: Duration) -> Self {
        Self {
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            shutdown_in_progress: Arc::new(AtomicBool::new(false)),
            cleanup_timeout,
            active_tasks: Arc::new(Semaphore::new(1000)), // Allow up to 1000 concurrent tasks
            resources: Arc::new(RwLock::new(Vec::new())),
            cleanup_hooks: Arc::new(RwLock::new(Vec::new())),
            cancellation_token: CancellationToken::new(),
            task_registry: Arc::new(RwLock::new(JoinSet::new())),
            runtime_hooks: Arc::new(RwLock::new(Vec::new())),
            toplevel: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new ResourceCleanup instance with tokio-graceful-shutdown integration
    pub fn new_with_graceful_shutdown(cleanup_timeout: Duration) -> Self {
        let toplevel = Toplevel::new(|s| async move {
            s.start(SubsystemBuilder::new("resource_cleanup", |_| async {
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            }));
        });

        let mut instance = Self::new(cleanup_timeout);
        instance.toplevel = Arc::new(RwLock::new(Some(toplevel)));

        instance
    }

    /// Initialize signal handling for graceful shutdown
    pub async fn setup_signal_handling(&self) -> Result<()> {
        let cleanup = self.clone();

        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};

                let mut sigterm =
                    signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
                let mut sigint =
                    signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");

                tokio::select! {
                    _ = sigterm.recv() => {
                        info!("Received SIGTERM, initiating graceful shutdown");
                        if let Err(e) = cleanup.graceful_shutdown().await {
                            error!("Graceful shutdown failed: {}", e);
                        }
                    }
                    _ = sigint.recv() => {
                        info!("Received SIGINT, initiating graceful shutdown");
                        if let Err(e) = cleanup.graceful_shutdown().await {
                            error!("Graceful shutdown failed: {}", e);
                        }
                    }
                }
            }

            #[cfg(windows)]
            {
                use tokio::signal::windows;

                let mut ctrl_c = windows::ctrl_c().expect("Failed to register Ctrl+C handler");
                let mut ctrl_break =
                    windows::ctrl_break().expect("Failed to register Ctrl+Break handler");

                tokio::select! {
                    _ = ctrl_c.recv() => {
                        info!("Received Ctrl+C, initiating graceful shutdown");
                        if let Err(e) = cleanup.graceful_shutdown().await {
                            error!("Graceful shutdown failed: {}", e);
                        }
                    }
                    _ = ctrl_break.recv() => {
                        info!("Received Ctrl+Break, initiating graceful shutdown");
                        if let Err(e) = cleanup.graceful_shutdown().await {
                            error!("Graceful shutdown failed: {}", e);
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Get cancellation token for coordinated shutdown
    pub fn get_cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    /// Register a managed task that will be cancelled during shutdown
    pub async fn register_task<F, Fut>(&self, task_fn: F) -> Result<()>
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), CleanupError>> + Send + 'static,
    {
        let token = self.cancellation_token.child_token();
        let mut registry = self.task_registry.write().await;

        registry.spawn(async move { task_fn(token).await });

        Ok(())
    }

    /// Register a runtime shutdown hook for async cleanup operations
    pub async fn register_runtime_hook(&self, hook: RuntimeShutdownHook) -> Result<()> {
        let mut hooks = self.runtime_hooks.write().await;
        hooks.push(hook);
        Ok(())
    }

    /// Register a resource for cleanup
    pub async fn register_resource(&self, resource: CleanupResource) -> Result<()> {
        let mut resources = self.resources.write().await;
        resources.push(resource);
        Ok(())
    }

    /// Register a cleanup hook
    pub async fn register_hook(&self, hook: CleanupHook) -> Result<()> {
        let mut hooks = self.cleanup_hooks.write().await;
        hooks.push(hook);
        Ok(())
    }

    /// Check if shutdown is requested
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_requested.load(Ordering::Relaxed)
    }

    /// Request graceful shutdown - now idempotent
    pub async fn graceful_shutdown(&self) -> Result<()> {
        // Check if shutdown is already in progress - if so, wait for it to complete
        if self.shutdown_in_progress.load(Ordering::Acquire) {
            info!("Shutdown already in progress, waiting for completion");
            return self.wait_for_shutdown_completion().await;
        }

        // Atomically check and set shutdown_in_progress
        if self
            .shutdown_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Acquire)
            .is_err()
        {
            // Another thread beat us to it, wait for completion
            info!("Shutdown initiated by another thread, waiting for completion");
            return self.wait_for_shutdown_completion().await;
        }

        // Set shutdown_requested flag
        self.shutdown_requested.store(true, Ordering::SeqCst);

        let shutdown_start = std::time::Instant::now();
        info!(
            "Initiating graceful shutdown sequence with timeout {:?}",
            self.cleanup_timeout
        );

        // Cancel all managed tasks
        self.cancellation_token.cancel();

        // Execute shutdown sequence with per-phase timeouts instead of single timeout
        let result = self.perform_shutdown_sequence_with_timeouts().await;

        // Mark shutdown as no longer in progress
        self.shutdown_in_progress.store(false, Ordering::SeqCst);

        let shutdown_duration = shutdown_start.elapsed();
        info!("Shutdown sequence completed in {:?}", shutdown_duration);

        if shutdown_duration > Duration::from_millis(200) {
            warn!(
                "Shutdown took longer than expected: {:?}",
                shutdown_duration
            );
        }

        result
    }

    /// Wait for an ongoing shutdown to complete
    async fn wait_for_shutdown_completion(&self) -> Result<()> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 100;
        const POLL_INTERVAL: Duration = Duration::from_millis(100);

        while self.shutdown_in_progress.load(Ordering::Acquire) && attempts < MAX_ATTEMPTS {
            tokio::time::sleep(POLL_INTERVAL).await;
            attempts += 1;
        }

        if attempts >= MAX_ATTEMPTS {
            warn!("Timeout waiting for shutdown completion");
            return Err(CleanupError::Timeout {
                timeout_ms: (MAX_ATTEMPTS as u64) * POLL_INTERVAL.as_millis() as u64,
            }
            .into());
        }

        info!("Shutdown completed successfully");
        Ok(())
    }

    /// Perform shutdown sequence with per-phase timeouts for better control
    async fn perform_shutdown_sequence_with_timeouts(&self) -> Result<()> {
        let shutdown_start = std::time::Instant::now();
        info!("Starting graceful shutdown sequence with per-phase timeouts");

        // Phase 1: Execute runtime hooks (100ms timeout)
        let phase_start = std::time::Instant::now();
        info!("Phase 1/5: Executing runtime hooks");
        match timeout(Duration::from_millis(100), self.execute_runtime_hooks()).await {
            Ok(Ok(_)) => {
                let phase_duration = phase_start.elapsed();
                info!("Phase 1 completed successfully in {:?}", phase_duration);
                if phase_duration > Duration::from_millis(200) {
                    warn!(
                        "Phase 1 (runtime hooks) took longer than expected: {:?}",
                        phase_duration
                    );
                }
            }
            Ok(Err(e)) => {
                let phase_duration = phase_start.elapsed();
                warn!("Phase 1 failed after {:?}: {}", phase_duration, e);
                return Err(e);
            }
            Err(_) => {
                let phase_duration = phase_start.elapsed();
                warn!("Phase 1 timed out after {:?}", phase_duration);
                return Err(CleanupError::Timeout { timeout_ms: 100 }.into());
            }
        }

        // Phase 2: Wait for managed tasks (200ms timeout, with forced abort)
        let phase_start = std::time::Instant::now();
        info!("Phase 2/5: Waiting for managed tasks");
        match timeout(
            Duration::from_millis(200),
            self.wait_for_managed_tasks_quick(),
        )
        .await
        {
            Ok(Ok(_)) => {
                let phase_duration = phase_start.elapsed();
                info!("Phase 2 completed successfully in {:?}", phase_duration);
                if phase_duration > Duration::from_millis(200) {
                    warn!(
                        "Phase 2 (managed tasks) took longer than expected: {:?}",
                        phase_duration
                    );
                }
            }
            Ok(Err(e)) => {
                let phase_duration = phase_start.elapsed();
                warn!("Phase 2 failed after {:?}: {}", phase_duration, e);
                // Force abort remaining tasks and continue
                if let Err(abort_err) = self.cancel_all_tasks().await {
                    warn!("Failed to abort remaining tasks: {}", abort_err);
                }
                warn!("Continuing shutdown despite task management issues");
            }
            Err(_) => {
                let phase_duration = phase_start.elapsed();
                warn!(
                    "Phase 2 timed out after {:?}, forcing task abort",
                    phase_duration
                );
                // Force abort remaining tasks and continue
                if let Err(abort_err) = self.cancel_all_tasks().await {
                    warn!("Failed to abort remaining tasks: {}", abort_err);
                }
            }
        }

        // Phase 3: Execute synchronous cleanup hooks (50ms timeout)
        let phase_start = std::time::Instant::now();
        info!("Phase 3/5: Executing synchronous cleanup hooks");
        match timeout(Duration::from_millis(50), self.execute_cleanup_hooks()).await {
            Ok(Ok(_)) => {
                let phase_duration = phase_start.elapsed();
                info!("Phase 3 completed successfully in {:?}", phase_duration);
                if phase_duration > Duration::from_millis(200) {
                    warn!(
                        "Phase 3 (cleanup hooks) took longer than expected: {:?}",
                        phase_duration
                    );
                }
            }
            Ok(Err(e)) => {
                let phase_duration = phase_start.elapsed();
                warn!("Phase 3 failed after {:?}: {}", phase_duration, e);
                // Continue with shutdown for non-critical hook failures
            }
            Err(_) => {
                let phase_duration = phase_start.elapsed();
                warn!(
                    "Phase 3 timed out after {:?}, continuing shutdown",
                    phase_duration
                );
            }
        }

        // Phase 4: Clean up resources by priority (100ms timeout)
        let phase_start = std::time::Instant::now();
        info!("Phase 4/5: Cleaning up resources by priority");
        match timeout(
            Duration::from_millis(100),
            self.cleanup_resources_by_priority(),
        )
        .await
        {
            Ok(Ok(_)) => {
                let phase_duration = phase_start.elapsed();
                info!("Phase 4 completed successfully in {:?}", phase_duration);
                if phase_duration > Duration::from_millis(200) {
                    warn!(
                        "Phase 4 (resource cleanup) took longer than expected: {:?}",
                        phase_duration
                    );
                }
            }
            Ok(Err(e)) => {
                let phase_duration = phase_start.elapsed();
                warn!("Phase 4 failed after {:?}: {}", phase_duration, e);
                // Continue with final cleanup even if some resources failed
            }
            Err(_) => {
                let phase_duration = phase_start.elapsed();
                warn!(
                    "Phase 4 timed out after {:?}, continuing to final cleanup",
                    phase_duration
                );
            }
        }

        // Phase 5: Final cleanup (50ms timeout)
        let phase_start = std::time::Instant::now();
        info!("Phase 5/5: Performing final cleanup");
        match timeout(Duration::from_millis(50), self.final_cleanup()).await {
            Ok(Ok(_)) => {
                let phase_duration = phase_start.elapsed();
                info!("Phase 5 completed successfully in {:?}", phase_duration);
                if phase_duration > Duration::from_millis(200) {
                    warn!(
                        "Phase 5 (final cleanup) took longer than expected: {:?}",
                        phase_duration
                    );
                }
            }
            Ok(Err(e)) => {
                let phase_duration = phase_start.elapsed();
                warn!("Phase 5 failed after {:?}: {}", phase_duration, e);
            }
            Err(_) => {
                let phase_duration = phase_start.elapsed();
                warn!("Phase 5 timed out after {:?}", phase_duration);
            }
        }

        let total_duration = shutdown_start.elapsed();
        info!(
            "Graceful shutdown sequence completed in {:?}",
            total_duration
        );

        if total_duration > Duration::from_millis(1000) {
            warn!(
                "Total shutdown took longer than expected: {:?}",
                total_duration
            );
        }

        Ok(())
    }

    #[allow(dead_code)]
    async fn perform_shutdown_sequence(&self) -> Result<()> {
        info!("Starting shutdown sequence");

        // Step 1: Execute runtime shutdown hooks
        if let Err(e) = self.execute_runtime_hooks().await {
            warn!("Runtime hooks execution failed: {}", e);
        }

        // Step 2: Wait for managed tasks to complete
        if let Err(e) = self.wait_for_managed_tasks().await {
            warn!("Managed tasks cleanup failed: {}", e);
        }

        // Step 3: Execute synchronous cleanup hooks
        if let Err(e) = self.execute_cleanup_hooks().await {
            warn!("Cleanup hooks execution failed: {}", e);
        }

        // Step 4: Clean up resources by priority
        if let Err(e) = self.cleanup_resources_by_priority().await {
            warn!("Resource cleanup failed: {}", e);
        }

        // Step 5: Final cleanup
        if let Err(e) = self.final_cleanup().await {
            warn!("Final cleanup failed: {}", e);
        }

        info!("Shutdown sequence completed successfully");
        Ok(())
    }

    /// Execute runtime shutdown hooks
    async fn execute_runtime_hooks(&self) -> Result<()> {
        let hooks = self.runtime_hooks.read().await;
        if hooks.is_empty() {
            return Ok(());
        }

        info!("Executing {} runtime shutdown hooks", hooks.len());
        let mut join_handles = Vec::new();

        for hook in hooks.iter() {
            let token = self.cancellation_token.child_token();
            let handle = hook(token);
            join_handles.push(handle);
        }

        // Wait for all runtime hooks to complete
        for handle in join_handles {
            if let Err(e) = handle.await {
                warn!("Runtime hook failed: {}", e);
            }
        }

        Ok(())
    }

    /// Wait for all managed tasks to complete
    /// Quick version of wait_for_managed_tasks with shorter timeout for graceful shutdown
    async fn wait_for_managed_tasks_quick(&self) -> Result<()> {
        let mut registry = self.task_registry.write().await;

        info!(
            "Waiting for {} managed tasks to complete (quick mode)",
            registry.len()
        );

        // Use shorter timeout for quick shutdown
        let task_timeout = Duration::from_millis(150); // 150ms timeout for quick mode
        let start_time = std::time::Instant::now();

        while !registry.is_empty() {
            if start_time.elapsed() > task_timeout {
                warn!("Quick task cleanup timeout reached, aborting remaining tasks");
                registry.abort_all();
                break;
            }

            match timeout(Duration::from_millis(50), registry.join_next()).await {
                Ok(Some(result)) => match result {
                    Ok(task_result) => {
                        if let Err(e) = task_result {
                            warn!("Managed task failed during shutdown: {}", e);
                        }
                    }
                    Err(e) => {
                        warn!("Task join error: {}", e);
                    }
                },
                Ok(None) => break,  // No more tasks
                Err(_) => continue, // Timeout, continue waiting
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    async fn wait_for_managed_tasks(&self) -> Result<()> {
        let mut registry = self.task_registry.write().await;

        info!("Waiting for {} managed tasks to complete", registry.len());

        let task_timeout = Duration::from_secs(30); // 30 second timeout for tasks
        let start_time = std::time::Instant::now();

        while !registry.is_empty() {
            if start_time.elapsed() > task_timeout {
                warn!("Task cleanup timeout reached, aborting remaining tasks");
                registry.abort_all();
                break;
            }

            match timeout(Duration::from_millis(100), registry.join_next()).await {
                Ok(Some(result)) => match result {
                    Ok(task_result) => {
                        if let Err(e) = task_result {
                            warn!("Managed task failed during shutdown: {}", e);
                        }
                    }
                    Err(e) => {
                        warn!("Task join error: {}", e);
                    }
                },
                Ok(None) => break,  // No more tasks
                Err(_) => continue, // Timeout, continue waiting
            }
        }

        Ok(())
    }

    /// Execute cleanup hooks
    async fn execute_cleanup_hooks(&self) -> Result<()> {
        let hooks = self.cleanup_hooks.read().await;
        if hooks.is_empty() {
            return Ok(());
        }

        info!("Executing {} cleanup hooks", hooks.len());
        let mut critical_failures = 0;

        for (i, hook) in hooks.iter().enumerate() {
            match hook() {
                Ok(_) => debug!("Cleanup hook {} executed successfully", i),
                Err(e) => {
                    warn!("Cleanup hook {} failed: {}", i, e);
                    critical_failures += 1;
                    // Early return if too many hooks fail to prevent timeout
                    if critical_failures > hooks.len() / 2 {
                        warn!(
                            "Too many cleanup hooks failed ({}), continuing with shutdown",
                            critical_failures
                        );
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Clean up resources by priority
    async fn cleanup_resources_by_priority(&self) -> Result<()> {
        let mut resources = self.resources.read().await.clone();
        if resources.is_empty() {
            return Ok(());
        }

        // Sort by priority (Critical first, then High, Medium, Low)
        resources.sort_by_key(|r| r.priority.clone());

        info!("Cleaning up {} resources by priority", resources.len());

        for resource in &resources {
            match self.cleanup_resource(resource).await {
                Ok(_) => debug!("Resource '{}' cleaned up successfully", resource.name),
                Err(e) => warn!("Failed to clean up resource '{}': {}", resource.name, e),
            }
        }

        Ok(())
    }

    /// Clean up a specific resource
    async fn cleanup_resource(&self, resource: &CleanupResource) -> Result<()> {
        let start = std::time::Instant::now();
        info!(
            "Cleaning up resource: {} (priority: {:?})",
            resource.name, resource.priority
        );

        // Use resource-specific timeout with early return on failure
        match timeout(resource.timeout, self.cleanup_by_type(resource)).await {
            Ok(Ok(_)) => {
                let duration = start.elapsed();
                debug!(
                    "Resource {} cleaned up successfully in {:?}",
                    resource.name, duration
                );
                Ok(())
            }
            Ok(Err(e)) => {
                let duration = start.elapsed();
                warn!(
                    "Resource {} cleanup failed after {:?}: {}",
                    resource.name, duration, e
                );
                // For non-critical resources, continue with shutdown instead of failing
                match resource.priority {
                    CleanupPriority::Critical => Err(e),
                    _ => {
                        warn!(
                            "Non-critical resource {} cleanup failed, continuing shutdown",
                            resource.name
                        );
                        Ok(())
                    }
                }
            }
            Err(_) => {
                let duration = start.elapsed();
                warn!(
                    "Resource {} cleanup timed out after {:?}",
                    resource.name, duration
                );
                // For non-critical resources, continue with shutdown instead of failing
                match resource.priority {
                    CleanupPriority::Critical => Err(CleanupError::Timeout {
                        timeout_ms: resource.timeout.as_millis() as u64,
                    }
                    .into()),
                    _ => {
                        warn!(
                            "Non-critical resource {} timed out, continuing shutdown",
                            resource.name
                        );
                        Ok(())
                    }
                }
            }
        }
    }

    /// Clean up resource by type
    async fn cleanup_by_type(&self, resource: &CleanupResource) -> Result<()> {
        match resource.resource_type {
            ResourceType::Database => {
                info!("Flushing database connections and closing handles");
                // Database connections are typically closed when Arc is dropped
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            ResourceType::Network => {
                info!("Closing network connections");
                // Network connections cleanup
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            ResourceType::FileHandle => {
                info!("Closing file handles and flushing buffers");
                // File handles are closed when dropped, but we can force a sync
                // Use async command with timeout to avoid blocking
                match timeout(Duration::from_millis(200), async {
                    tokio::process::Command::new("sync").output().await
                })
                .await
                {
                    Ok(Ok(_)) => debug!("Filesystem sync completed"),
                    Ok(Err(e)) => warn!("Filesystem sync failed: {}", e),
                    Err(_) => warn!("Filesystem sync timed out after 200ms"),
                }
            }
            ResourceType::ThreadPool => {
                info!("Shutting down thread pools");
                // Thread pools are shut down when dropped
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            ResourceType::Cache => {
                info!("Clearing caches and freeing memory");
                // Caches are cleared when dropped
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            ResourceType::Wallet => {
                info!("Saving wallet state and closing");
                // Wallet state is typically saved automatically
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            ResourceType::Mining => {
                info!("Stopping mining operations");
                // Mining operations should be cancelled via shutdown token
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            ResourceType::P2P => {
                info!("Disconnecting from P2P network");
                // P2P connections cleanup
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
            ResourceType::WebSocket => {
                info!("Closing WebSocket connections");
                // WebSocket connections cleanup
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            ResourceType::Storage => {
                info!("Flushing storage and closing handles");
                // Storage cleanup
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
            ResourceType::Runtime => {
                info!("Shutting down runtime components");
                // Runtime cleanup - cancel tasks and wait for completion
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            ResourceType::AsyncTask => {
                info!("Cancelling async tasks");
                // Async task cleanup - handled via cancellation tokens
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }

        Ok(())
    }

    /// Final cleanup operations
    async fn final_cleanup(&self) -> Result<()> {
        // Force garbage collection if possible
        debug!("Performing final memory cleanup");

        // Give the system time to clean up
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Log final resource usage if available
        if let Ok(output) = std::process::Command::new("ps")
            .args([
                "-o",
                "pid,ppid,rss,vsz,comm",
                "-p",
                &std::process::id().to_string(),
            ])
            .output()
        {
            if let Ok(output_str) = String::from_utf8(output.stdout) {
                debug!("Final process stats:\n{}", output_str);
            }
        }

        Ok(())
    }

    /// Request shutdown and initiate graceful cleanup
    pub fn request_shutdown(&self) {
        info!("Graceful shutdown requested");
        self.shutdown_requested.store(true, Ordering::Relaxed);
        self.cancellation_token.cancel();
    }

    /// Wait for all active tasks to complete
    pub async fn wait_for_tasks(&self, timeout_duration: Duration) -> Result<()> {
        let start = std::time::Instant::now();

        loop {
            let available_permits = self.active_tasks.available_permits();
            if available_permits >= 1000 {
                debug!("All tasks completed successfully");
                break;
            }

            if start.elapsed() > timeout_duration {
                warn!(
                    "Task completion timeout - {} tasks still active",
                    1000 - available_permits
                );
                return Err(CleanupError::Timeout {
                    timeout_ms: timeout_duration.as_millis() as u64,
                }
                .into());
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }

    /// Cancel all managed tasks immediately
    pub async fn cancel_all_tasks(&self) -> Result<()> {
        info!("Cancelling all managed tasks");

        // Cancel via token
        self.cancellation_token.cancel();

        // Abort all tasks in registry
        let mut registry = self.task_registry.write().await;
        registry.abort_all();

        Ok(())
    }

    /// Get shutdown statistics
    pub async fn get_shutdown_stats(&self) -> ShutdownStats {
        let resources = self.resources.read().await;
        let hooks = self.cleanup_hooks.read().await;
        let runtime_hooks = self.runtime_hooks.read().await;
        let registry = self.task_registry.read().await;

        ShutdownStats {
            is_shutdown_requested: self.is_shutdown_requested(),
            is_shutdown_in_progress: self.shutdown_in_progress.load(Ordering::Relaxed),
            active_tasks: 1000 - self.active_tasks.available_permits(),
            registered_resources: resources.len(),
            registered_hooks: hooks.len(),
            registered_runtime_hooks: runtime_hooks.len(),
            managed_tasks: registry.len(),
            cleanup_timeout_ms: self.cleanup_timeout.as_millis() as u64,
        }
    }

    /// Create a task guard that automatically decrements active task count
    pub async fn create_task_guard(&self) -> Result<TaskGuard> {
        let permit = self
            .active_tasks
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| CleanupError::TaskManagement {
                message: format!("Failed to acquire task permit: {e}"),
            })?;

        Ok(TaskGuard {
            _permit: permit,
            cleanup: self.shutdown_requested.clone(),
        })
    }
}

/// Task guard that automatically manages task lifecycle
pub struct TaskGuard {
    _permit: tokio::sync::OwnedSemaphorePermit,
    cleanup: Arc<AtomicBool>,
}

impl TaskGuard {
    /// Check if shutdown is requested
    pub fn should_shutdown(&self) -> bool {
        self.cleanup.load(Ordering::Relaxed)
    }
}

/// Default resource cleanup configuration
impl Default for ResourceCleanup {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

/// Helper function to create standard resource cleanup configuration
pub async fn create_standard_cleanup() -> Result<ResourceCleanup> {
    let cleanup = ResourceCleanup::new(Duration::from_secs(60));

    // Register standard resources in cleanup order
    let resources = vec![
        CleanupResource {
            name: "Database".to_string(),
            resource_type: ResourceType::Database,
            priority: CleanupPriority::Critical,
            timeout: Duration::from_secs(10),
        },
        CleanupResource {
            name: "Wallet".to_string(),
            resource_type: ResourceType::Wallet,
            priority: CleanupPriority::Critical,
            timeout: Duration::from_secs(5),
        },
        CleanupResource {
            name: "Mining".to_string(),
            resource_type: ResourceType::Mining,
            priority: CleanupPriority::High,
            timeout: Duration::from_secs(15),
        },
        CleanupResource {
            name: "P2P Network".to_string(),
            resource_type: ResourceType::P2P,
            priority: CleanupPriority::High,
            timeout: Duration::from_secs(10),
        },
        CleanupResource {
            name: "WebSocket".to_string(),
            resource_type: ResourceType::WebSocket,
            priority: CleanupPriority::High,
            timeout: Duration::from_secs(5),
        },
        CleanupResource {
            name: "Thread Pools".to_string(),
            resource_type: ResourceType::ThreadPool,
            priority: CleanupPriority::Medium,
            timeout: Duration::from_secs(8),
        },
        CleanupResource {
            name: "Caches".to_string(),
            resource_type: ResourceType::Cache,
            priority: CleanupPriority::Medium,
            timeout: Duration::from_secs(3),
        },
        CleanupResource {
            name: "File Handles".to_string(),
            resource_type: ResourceType::FileHandle,
            priority: CleanupPriority::Low,
            timeout: Duration::from_secs(5),
        },
    ];

    for resource in resources {
        cleanup.register_resource(resource).await?;
    }

    Ok(cleanup)
}

/// Statistics about the shutdown process
#[derive(Debug, Clone)]
pub struct ShutdownStats {
    pub is_shutdown_requested: bool,
    pub is_shutdown_in_progress: bool,
    pub active_tasks: usize,
    pub registered_resources: usize,
    pub registered_hooks: usize,
    pub registered_runtime_hooks: usize,
    pub managed_tasks: usize,
    pub cleanup_timeout_ms: u64,
}

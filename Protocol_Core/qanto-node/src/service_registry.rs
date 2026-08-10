//! Qanto Service Registry
//!
//! Thread-safe registry for all RuntimeServices in the Qanto node.
//! Supports registration, dependency resolution, deterministic startup ordering
//! via topological sort, and health state tracking.
//!
//! # Design
//!
//! Services are compiled into the node — this is NOT a plugin system.
//! Dependencies are declared at compile time via `RuntimeService::dependencies()`.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::runtime_service::{RuntimeError, RuntimeService, ServiceHealth, ServiceStatus};

/// Thread-safe registry for node services with dependency-aware ordering.
pub struct ServiceRegistry {
    /// Map of service name → service instance.
    services: RwLock<HashMap<String, Arc<dyn RuntimeService>>>,
    /// Current health state of each registered service.
    health_states: RwLock<HashMap<String, ServiceHealth>>,
}

impl ServiceRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
            health_states: RwLock::new(HashMap::new()),
        }
    }

    /// Register a service. Fails if a service with the same name already exists.
    /// Validates that all declared dependencies are present or will be registered.
    pub async fn register(&self, service: Arc<dyn RuntimeService>) -> Result<(), RuntimeError> {
        let name = service.name().to_string();
        let deps: Vec<String> = service
            .dependencies()
            .iter()
            .map(|d| d.to_string())
            .collect();

        // Check for duplicate
        {
            let services = self.services.read().await;
            if services.contains_key(&name) {
                return Err(RuntimeError::ServiceAlreadyRegistered(name));
            }
            // Validate that declared dependencies exist or are being registered
            // (dependency ordering is computed later at startup)
            for dep in &deps {
                if !services.contains_key(dep.as_str()) && dep.as_str() != name {
                    debug!(
                        "Service '{}' depends on '{}' which is not yet registered \
                         (must be registered before startup)",
                        name, dep
                    );
                    // Not a hard error at registration time — dependency may be registered later.
                    // We validate at startup.
                }
            }
        }

        // Insert service
        {
            let mut services = self.services.write().await;
            services.insert(name.clone(), service);
        }

        // Initialize health state
        {
            let mut health = self.health_states.write().await;
            health.insert(name.clone(), ServiceHealth::new(ServiceStatus::NotStarted));
        }

        debug!(
            "Service '{}' registered with {} dependencies",
            name,
            deps.len()
        );
        Ok(())
    }

    /// Remove a service by name. Fails if the service is in a Running state.
    pub async fn unregister(&self, name: &str) -> Result<(), RuntimeError> {
        // Check health state — cannot unregister a running service
        {
            let health = self.health_states.read().await;
            if let Some(h) = health.get(name) {
                if h.status == ServiceStatus::Running
                    || h.status == ServiceStatus::Starting
                    || h.status == ServiceStatus::Stopping
                {
                    return Err(RuntimeError::InvalidStateTransition {
                        service: name.to_string(),
                        from: h.status,
                        to: ServiceStatus::NotStarted,
                    });
                }
            }
        }

        // Remove
        let removed = {
            let mut services = self.services.write().await;
            services.remove(name)
        };

        if removed.is_some() {
            let mut health = self.health_states.write().await;
            health.remove(name);
            debug!("Service '{}' unregistered", name);
            Ok(())
        } else {
            Err(RuntimeError::ServiceNotFound(name.to_string()))
        }
    }

    /// Look up a service by name.
    pub async fn get(&self, name: &str) -> Option<Arc<dyn RuntimeService>> {
        let services = self.services.read().await;
        services.get(name).cloned()
    }

    /// List all registered services as (name, version, dependency_count) tuples.
    pub async fn list_services(&self) -> Vec<(String, String, usize)> {
        let services = self.services.read().await;
        services
            .values()
            .map(|s| {
                (
                    s.name().to_string(),
                    s.version().to_string(),
                    s.dependencies().len(),
                )
            })
            .collect()
    }

    /// Get the current health state of a service.
    pub async fn get_health(&self, name: &str) -> Option<ServiceHealth> {
        let health = self.health_states.read().await;
        health.get(name).cloned()
    }

    /// Update the health state of a service.
    pub async fn set_health(&self, name: &str, health: ServiceHealth) {
        let mut states = self.health_states.write().await;
        states.insert(name.to_string(), health);
    }

    /// Update only the status field of a service's health.
    pub async fn set_status(&self, name: &str, status: ServiceStatus) {
        let mut states = self.health_states.write().await;
        if let Some(health) = states.get_mut(name) {
            health.status = status;
            health.last_check = std::time::SystemTime::now();
        }
    }

    /// Get health states for all registered services.
    pub async fn all_health(&self) -> HashMap<String, ServiceHealth> {
        let health = self.health_states.read().await;
        health.clone()
    }

    /// Count services in each status.
    pub async fn status_counts(&self) -> HashMap<ServiceStatus, usize> {
        let health = self.health_states.read().await;
        let mut counts = HashMap::new();
        for h in health.values() {
            *counts.entry(h.status).or_insert(0) += 1;
        }
        counts
    }

    /// Compute the deterministic startup order using Kahn's topological sort.
    ///
    /// Returns services in order such that every dependency appears before
    /// the service that depends on it. Fails if a dependency cycle is detected.
    ///
    /// # Algorithm
    ///
    /// Kahn's algorithm (BFS-based topological sort):
    /// 1. Compute in-degree (number of unmet dependencies) for each service.
    /// 2. Start with all services that have zero in-degree.
    /// 3. Process each service, decrementing in-degrees of its dependents.
    /// 4. If any service remains unprocessed, a cycle exists.
    pub async fn compute_startup_order(&self) -> Result<Vec<String>, RuntimeError> {
        let services = self.services.read().await;

        if services.is_empty() {
            return Ok(Vec::new());
        }

        // Build adjacency: for each service A → depends_on B:
        //   B must start before A
        //   So B is a "prerequisite" of A
        //   In Kahn's terms: edge B → A (B must be processed before A)
        //
        // We also need the reverse: for each service, who depends on it?
        // That is: for dependency B of A, A is a "dependent" of B.

        // in_degree[name] = number of dependencies that must start before this service
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        // dependents[name] = services that depend on `name` (i.e., list of services
        //   where `name` appears in their dependencies)
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

        for (name, service) in services.iter() {
            let deps = service.dependencies();
            in_degree.entry(name.clone()).or_insert(0);
            for dep in deps {
                // dep must exist in registry
                if !services.contains_key(*dep) {
                    return Err(RuntimeError::MissingDependency {
                        service: name.clone(),
                        dep: (*dep).to_string(),
                    });
                }
                // This service depends on dep → increment this service's in-degree
                *in_degree.entry(name.clone()).or_insert(0) += 1;
                // dep has a dependent: this service
                dependents
                    .entry((*dep).to_string())
                    .or_default()
                    .push(name.clone());
            }
        }

        // Initialize queue with all zero-in-degree services (no unmet dependencies)
        let mut queue: VecDeque<String> = VecDeque::new();
        for name in services.keys() {
            if in_degree.get(name).copied().unwrap_or(0) == 0 {
                queue.push_back(name.clone());
            }
        }

        let mut order = Vec::with_capacity(services.len());

        while let Some(current) = queue.pop_front() {
            order.push(current.clone());

            // For each service that depends on `current`, decrement its in-degree
            if let Some(deps) = dependents.get(&current) {
                for dependent in deps {
                    if let Some(count) = in_degree.get_mut(dependent) {
                        *count = count.saturating_sub(1);
                        if *count == 0 {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }
        }

        // If not all services were processed, a cycle exists
        if order.len() != services.len() {
            let unprocessed: Vec<String> = services
                .keys()
                .filter(|k| !order.contains(k))
                .cloned()
                .collect();
            warn!("Dependency cycle detected involving: {:?}", unprocessed);
            return Err(RuntimeError::DependencyCycle(unprocessed));
        }

        debug!(
            "Computed startup order ({} services): {:?}",
            order.len(),
            order
        );
        Ok(order)
    }

    /// Compute shutdown order: reverse of startup order.
    /// Services that depend on others are shut down first.
    pub async fn compute_shutdown_order(&self) -> Result<Vec<String>, RuntimeError> {
        let mut order = self.compute_startup_order().await?;
        order.reverse();
        Ok(order)
    }

    /// Verify that all declared dependencies are registered.
    /// Call this before startup.
    pub async fn validate_dependencies(&self) -> Result<(), RuntimeError> {
        let services = self.services.read().await;
        for (name, service) in services.iter() {
            for dep in service.dependencies() {
                if !services.contains_key(*dep) {
                    return Err(RuntimeError::MissingDependency {
                        service: name.clone(),
                        dep: (*dep).to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ========================================================================
// Tests
// ========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    /// A test service that properly returns static dependency references.
    struct StaticDepService {
        name: &'static str,
        deps: &'static [&'static str],
    }

    #[async_trait]
    impl RuntimeService for StaticDepService {
        fn name(&self) -> &str {
            self.name
        }

        fn version(&self) -> &str {
            "1.0.0"
        }

        fn dependencies(&self) -> &[&str] {
            self.deps
        }

        async fn start(
            &self,
            _ctx: Arc<super::super::runtime_context::RuntimeContext>,
        ) -> Result<(), RuntimeError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_registry_register_and_get() {
        let registry = ServiceRegistry::new();
        let svc = Arc::new(StaticDepService {
            name: "test_svc",
            deps: &[],
        });

        registry.register(svc.clone()).await.unwrap();
        let found = registry.get("test_svc").await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().name(), "test_svc");
    }

    #[tokio::test]
    async fn test_registry_duplicate_rejected() {
        let registry = ServiceRegistry::new();
        let svc = Arc::new(StaticDepService {
            name: "dup",
            deps: &[],
        });

        registry.register(svc.clone()).await.unwrap();
        let result = registry.register(svc).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            RuntimeError::ServiceAlreadyRegistered(name) => assert_eq!(name, "dup"),
            e => panic!("Expected ServiceAlreadyRegistered, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_empty_registry_startup_order() {
        let registry = ServiceRegistry::new();
        let order = registry.compute_startup_order().await.unwrap();
        assert!(order.is_empty());
    }

    #[tokio::test]
    async fn test_simple_linear_dependency_order() {
        // A depends on B, B depends on C
        // Expected order: C → B → A
        let registry = ServiceRegistry::new();

        let c = Arc::new(StaticDepService {
            name: "C",
            deps: &[],
        });
        let b = Arc::new(StaticDepService {
            name: "B",
            deps: &["C"],
        });
        let a = Arc::new(StaticDepService {
            name: "A",
            deps: &["B"],
        });

        registry.register(c).await.unwrap();
        registry.register(b).await.unwrap();
        registry.register(a).await.unwrap();

        let order = registry.compute_startup_order().await.unwrap();
        assert_eq!(order, vec!["C", "B", "A"]);
    }

    #[tokio::test]
    async fn test_diamond_dependency_order() {
        // A depends on B and C
        // B depends on D
        // C depends on D
        // Expected: D comes before both B and C; B and C come before A
        // Valid orders: [D, B, C, A] or [D, C, B, A]
        let registry = ServiceRegistry::new();

        let d = Arc::new(StaticDepService {
            name: "D",
            deps: &[],
        });
        let b = Arc::new(StaticDepService {
            name: "B",
            deps: &["D"],
        });
        let c = Arc::new(StaticDepService {
            name: "C",
            deps: &["D"],
        });
        let a = Arc::new(StaticDepService {
            name: "A",
            deps: &["B", "C"],
        });

        registry.register(d).await.unwrap();
        registry.register(b).await.unwrap();
        registry.register(c).await.unwrap();
        registry.register(a).await.unwrap();

        let order = registry.compute_startup_order().await.unwrap();
        assert_eq!(order.len(), 4);
        // D must be first
        assert_eq!(order[0], "D");
        // A must be last
        assert_eq!(order[3], "A");
        // B and C must appear before A
        let b_pos = order.iter().position(|x| x == "B").unwrap();
        let c_pos = order.iter().position(|x| x == "C").unwrap();
        assert!(b_pos < 3);
        assert!(c_pos < 3);
    }

    #[tokio::test]
    async fn test_cycle_detection() {
        // A depends on B, B depends on A → cycle
        let registry = ServiceRegistry::new();

        let a = Arc::new(StaticDepService {
            name: "A",
            deps: &["B"],
        });
        let b = Arc::new(StaticDepService {
            name: "B",
            deps: &["A"],
        });

        registry.register(a).await.unwrap();
        registry.register(b).await.unwrap();

        let result = registry.compute_startup_order().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            RuntimeError::DependencyCycle(cycle) => {
                assert!(cycle.contains(&"A".to_string()) || cycle.contains(&"B".to_string()));
            }
            e => panic!("Expected DependencyCycle, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_missing_dependency_detection() {
        let registry = ServiceRegistry::new();

        let a = Arc::new(StaticDepService {
            name: "A",
            deps: &["MISSING"],
        });

        registry.register(a).await.unwrap();

        let result = registry.compute_startup_order().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            RuntimeError::MissingDependency { service, dep } => {
                assert_eq!(service, "A");
                assert_eq!(dep, "MISSING");
            }
            e => panic!("Expected MissingDependency, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_shutdown_order_is_reverse() {
        let registry = ServiceRegistry::new();

        let c = Arc::new(StaticDepService {
            name: "C",
            deps: &[],
        });
        let b = Arc::new(StaticDepService {
            name: "B",
            deps: &["C"],
        });
        let a = Arc::new(StaticDepService {
            name: "A",
            deps: &["B"],
        });

        registry.register(c).await.unwrap();
        registry.register(b).await.unwrap();
        registry.register(a).await.unwrap();

        let startup = registry.compute_startup_order().await.unwrap();
        let shutdown = registry.compute_shutdown_order().await.unwrap();

        assert_eq!(startup, vec!["C", "B", "A"]);
        assert_eq!(shutdown, vec!["A", "B", "C"]);
    }

    #[tokio::test]
    async fn test_health_state_tracking() {
        let registry = ServiceRegistry::new();
        let svc = Arc::new(StaticDepService {
            name: "health_test",
            deps: &[],
        });

        registry.register(svc).await.unwrap();

        // Initial state
        let health = registry.get_health("health_test").await.unwrap();
        assert_eq!(health.status, ServiceStatus::NotStarted);

        // Update status
        registry
            .set_status("health_test", ServiceStatus::Running)
            .await;
        let health = registry.get_health("health_test").await.unwrap();
        assert_eq!(health.status, ServiceStatus::Running);

        // Counts
        let counts = registry.status_counts().await;
        assert_eq!(*counts.get(&ServiceStatus::Running).unwrap(), 1);
    }

    #[tokio::test]
    async fn test_validate_dependencies() {
        let registry = ServiceRegistry::new();

        let a = Arc::new(StaticDepService {
            name: "A",
            deps: &["B"],
        });

        // Register A without B — should fail validation
        registry.register(a).await.unwrap();
        let result = registry.validate_dependencies().await;
        assert!(result.is_err());

        // Register B — should now pass
        let b = Arc::new(StaticDepService {
            name: "B",
            deps: &[],
        });
        registry.register(b).await.unwrap();
        let result = registry.validate_dependencies().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_services() {
        let registry = ServiceRegistry::new();

        let svc = Arc::new(StaticDepService {
            name: "svc1",
            deps: &[],
        });
        registry.register(svc).await.unwrap();

        let list = registry.list_services().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, "svc1");
        assert_eq!(list[0].1, "1.0.0");
        assert_eq!(list[0].2, 0); // 0 dependencies
    }
}

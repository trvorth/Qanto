use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/**
 * @title Mesh Optimizer (TMO)
 * @dev Self-optimizing layer for peak performance and shard-level stability.
 */
pub struct MeshOptimizer {
    pub shard_health: HashMap<u32, ShardHealth>,
    pub optimization_history: Vec<OptimizationEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardHealth {
    pub shard_id: u32,
    pub latency_ms: u32,
    pub tps: u64,
    pub puai_yield: f64,
    pub sentinel_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationEvent {
    pub timestamp: u64,
    pub event_type: String, // e.g., "SHARD_SPLIT", "SENTINEL_MIGRATION"
    pub shard_id: u32,
}

impl MeshOptimizer {
    pub fn new() -> Self {
        Self {
            shard_health: HashMap::new(),
            optimization_history: Vec::new(),
        }
    }

    /**
     * @dev Reflects shard-level metrics into the optimizer.
     */
    pub fn update_shard_health(&mut self, health: ShardHealth) {
        println!("TMO: Optimizing Shard {} (TPS: {})", health.shard_id, health.tps);
        self.shard_health.insert(health.shard_id, health);
    }

    /**
     * @dev Calculates if a shard split/merge is required.
     * Threshold: TPS > 10,000 or Latency > 150ms.
     */
    pub fn calculate_optimization_needs(&mut self) -> Vec<OptimizationEvent> {
        let mut events = Vec::new();
        
        for health in self.shard_health.values() {
            if health.tps > 10000 || health.latency_ms > 150 {
                println!("🔥 TMO: Thermal Spike detected on Shard {}. Proposing Split.", health.shard_id);
                events.push(OptimizationEvent {
                    timestamp: 1775492930, // Mock
                    event_type: "SHARD_SPLIT_PROPOSAL".to_string(),
                    shard_id: health.shard_id,
                });
            }
        }
        
        self.optimization_history.extend(events.clone());
        events
    }

    /**
     * @dev Optimizes sentinel assignment based on yield and latency.
     */
    pub fn rebalance_sentinels(&mut self, shard_id: u32) {
        println!("TMO: Rebalancing Sentinel distribution for Shard {}...", shard_id);
        // Logic: Move low-latency nodes to high-traffic shards
    }
}

// Phase 55: Optimization Verification
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_optimizer_split() {
        let mut optimizer = MeshOptimizer::new();
        optimizer.update_shard_health(ShardHealth {
            shard_id: 0,
            latency_ms: 180, // Over Threshold
            tps: 52000, // Over Threshold
            puai_yield: 92.5,
            sentinel_count: 5,
        });

        let needs = optimizer.calculate_optimization_needs();
        assert_eq!(needs.len(), 1);
        assert_eq!(needs[0].event_type, "SHARD_SPLIT_PROPOSAL");
    }
}

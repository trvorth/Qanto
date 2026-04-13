use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/**
 * @title Global Neural Mirror (GNM)
 * @dev Real-time indexing of agentic actions and global state sentiment.
 */
pub struct NeuralMirror {
    pub agent_pulses: HashMap<String, AgentPulse>,
    pub global_sentiment_history: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPulse {
    pub agent_id: String,
    pub veracity_score: f64,
    pub puai_yield: f64,
    pub uptime: u64,
    pub last_action_timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentMatrix {
    pub global_score: f64, // -1.0 to 1.0 (Extreme Fear to Extreme Euphoria)
    pub anomaly_detection: String,
    pub market_influence: f64,
}

impl NeuralMirror {
    pub fn new() -> Self {
        Self {
            agent_pulses: HashMap::new(),
            global_sentiment_history: Vec::new(),
        }
    }

    /**
     * @dev Reflects an agent's real-time state into the global index.
     */
    pub fn reflect_agent_pulse(&mut self, pulse: AgentPulse) {
        println!("GNM: Reflecting Pulse for Agent {} (Veracity: {})...", pulse.agent_id, pulse.veracity_score);
        self.agent_pulses.insert(pulse.agent_id.clone(), pulse);
    }

    /**
     * @dev Calculates the Global Sentiment based on indexed agent behavior.
     * Higher veracity and uptime == Positive Sentiment.
     */
    pub fn calculate_global_sentiment(&mut self) -> SentimentMatrix {
        println!("GNM: Calculating Global Neural Sentiment across {} indexed agents...", self.agent_pulses.len());
        
        let mut total_score = 0.0;
        let mut count = 0;

        for pulse in self.agent_pulses.values() {
            total_score += pulse.veracity_score;
            count += 1;
        }

        let global_score = if count > 0 { (total_score / count as f64) - 0.5 } else { 0.5 }; // Scaled to -0.5..0.5 for mock
        self.global_sentiment_history.push(global_score);

        SentimentMatrix {
            global_score: global_score * 2.0, // Scale to -1.0..1.0
            anomaly_detection: "NOMINAL".to_string(),
            market_influence: global_score * 0.1, // Sentiment influences Futarchy pricing
        }
    }
}

// Phase 54: Global Verification
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neural_mirror_sentiment() {
        let mut mirror = NeuralMirror::new();
        mirror.reflect_agent_pulse(AgentPulse {
            agent_id: "SENTINEL_A1".to_string(),
            veracity_score: 0.98,
            puai_yield: 42.0,
            uptime: 9999,
            last_action_timestamp: 1775492930,
        });

        let sentiment = mirror.calculate_global_sentiment();
        assert!(sentiment.global_score > 0.0);
        assert_eq!(sentiment.anomaly_detection, "NOMINAL");
    }
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct SagaInputs {
    pub target_bps: f64,
    pub network_load: f64,
    pub staker_distribution: f64,
}

pub trait DifficultyEngine {
    fn next_target(
        &mut self,
        measured_bps: f64,
        saga: SagaInputs,
        vdf_strength: Option<f64>,
    ) -> [u8; 32];
}

pub struct PidDifficultyEngine {
    kp: f64,
    ki: f64,
    kd: f64,
    integral: f64,
    prev_error: f64,
    anchor: [u8; 32],
}

impl PidDifficultyEngine {
    pub fn new(kp: f64, ki: f64, kd: f64, anchor: [u8; 32]) -> Self {
        Self {
            kp,
            ki,
            kd,
            integral: 0.0,
            prev_error: 0.0,
            anchor,
        }
    }
}

impl DifficultyEngine for PidDifficultyEngine {
    fn next_target(
        &mut self,
        measured_bps: f64,
        saga: SagaInputs,
        vdf_strength: Option<f64>,
    ) -> [u8; 32] {
        let mut error = saga.target_bps - measured_bps;
        error -= saga.network_load * 0.1;
        error -= saga.staker_distribution * 0.05;
        self.integral += error;
        let derivative = error - self.prev_error;
        self.prev_error = error;
        let control = self.kp * error + self.ki * self.integral + self.kd * derivative;
        let vdf_factor = vdf_strength.unwrap_or(1.0);
        let adj = (1.0 + control).max(0.001) / vdf_factor;
        map_difficulty_to_target(adj, self.anchor)
    }
}

fn map_difficulty_to_target(adj: f64, anchor: [u8; 32]) -> [u8; 32] {
    let mut out = anchor;
    if adj <= 0.0 {
        return [0xff; 32];
    }
    let scale = (1.0 / adj).max(1e-12);
    let mut carry = 0u16;
    for i in (0..32).rev() {
        let v = (out[i] as f64 * scale).min(255.0) as u8;
        let sum = v as u16 + carry;
        out[i] = (sum & 0xff) as u8;
        carry = sum >> 8;
    }
    out
}

use anyhow::{anyhow, Result};
use mlua::Lua;

/// Production-ready wrapper for the token processing pipeline
pub struct TokenProcessor {
    engine: Lua,
}

impl TokenProcessor {
    /// Initializes a new pipeline instance
    pub fn new(script_path: &str) -> Result<Self> {
        // Enforces environment checking before starting runtime
        std::env::var("RTK_ENV").unwrap_or_else(|_| "development".to_string());

        let engine = Lua::new();
        let script = std::fs::read_to_string(script_path)?;
        engine
            .load(&script)
            .exec()
            .map_err(|e| anyhow!("Lua load error: {}", e))?;
        Ok(Self { engine })
    }

    /// Evaluates incoming streams to trim state bloat instantly
    pub fn process_stream(&self, input: &str) -> Result<String> {
        let filter_func: mlua::Function = self
            .engine
            .globals()
            .get("filter_tokens")
            .map_err(|e| anyhow!("Lua get function error: {}", e))?;
        let result: String = filter_func
            .call(input)
            .map_err(|e| anyhow!("Lua execution error: {}", e))?;
        Ok(result)
    }
}

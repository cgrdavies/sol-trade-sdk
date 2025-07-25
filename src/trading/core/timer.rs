use std::time::Instant;

/// Trade timer for measuring execution time
#[derive(Clone)]
pub struct TradeTimer {
    start_time: Instant,
    stage: String,
}

impl TradeTimer {
    /// Create new timer
    pub fn new(stage: impl Into<String>) -> Self {
        Self {
            start_time: Instant::now(),
            stage: stage.into(),
        }
    }
    
    /// Record current stage execution time and start new stage
    pub fn stage(&mut self, new_stage: impl Into<String>) {
        let elapsed = self.start_time.elapsed();
        println!(" {} took: {:?}", self.stage, elapsed);
        
        self.start_time = Instant::now();
        self.stage = new_stage.into();
    }
    
    /// Finish timing and output final execution time
    pub fn finish(mut self) {
        let elapsed = self.start_time.elapsed();
        println!(" {} took: {:?}", self.stage, elapsed);
        self.stage.clear(); // Clear stage to avoid duplicate printing on Drop
    }
    
    /// Get current stage execution time (without resetting timer)
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
}

impl Drop for TradeTimer {
    fn drop(&mut self) {
        if !self.stage.is_empty() {
            let elapsed = self.start_time.elapsed();
            println!(" {} took: {:?}", self.stage, elapsed);
        }
    }
} 
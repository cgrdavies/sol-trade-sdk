use anyhow::Result;
use solana_sdk::{instruction::Instruction, signature::Signature};
use super::params::{BuyParams, BuyWithTipParams, SellParams, SellWithTipParams};

/// Trade executor trait - defines core methods that all trading protocols must implement
#[async_trait::async_trait]
pub trait TradeExecutor: Send + Sync {
    /// Execute buy transaction
    async fn buy(&self, params: BuyParams) -> Result<Signature>;

    /// Execute buy transaction using MEV service - returns the first successfully confirmed transaction signature
    async fn buy_with_tip(&self, params: BuyWithTipParams) -> Result<Signature>;

    /// Execute sell transaction
    async fn sell(&self, params: SellParams) -> Result<Signature>;

    /// Execute sell transaction using MEV service - returns the first successfully confirmed transaction signature
    async fn sell_with_tip(&self, params: SellWithTipParams) -> Result<Signature>;

    /// Get protocol name
    fn protocol_name(&self) -> &'static str;
}

/// Instruction builder trait - responsible for building protocol-specific transaction instructions
#[async_trait::async_trait]
pub trait InstructionBuilder: Send + Sync {
    /// Build buy instructions
    async fn build_buy_instructions(&self, params: &BuyParams) -> Result<Vec<Instruction>>;

    /// Build sell instructions
    async fn build_sell_instructions(&self, params: &SellParams) -> Result<Vec<Instruction>>;
}

/// Protocol specific parameter trait - allows each protocol to define its own parameters
pub trait ProtocolParams: Send + Sync {
    /// Convert parameters to Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any;

    /// Clone parameters
    fn clone_box(&self) -> Box<dyn ProtocolParams>;
}

impl Clone for Box<dyn ProtocolParams> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

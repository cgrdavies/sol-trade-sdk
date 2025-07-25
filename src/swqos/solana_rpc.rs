use std::sync::Arc;

use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::{
    commitment_config::CommitmentLevel,
    transaction::VersionedTransaction,
    signature::Signature,
};
use solana_transaction_status::UiTransactionEncoding;

use crate::common::{SolanaRpcClient, types::TransactionResult};
use crate::swqos::{SwqosType, TradeType};
use crate::swqos::SwqosClientTrait;
use anyhow::{anyhow, Result};
use std::time::Instant;

#[derive(Clone)]
pub struct SolRpcClient {
    pub rpc_client: Arc<SolanaRpcClient>,
}

#[async_trait::async_trait]
impl SwqosClientTrait for SolRpcClient {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<TransactionResult> {
        let start_time = Instant::now();
        let signature = self.rpc_client
            .send_transaction(transaction)
            .await
            .map_err(|e| anyhow!("SolanaRPC transaction submission failed: {}", e))?;

        println!(" solana_rpc{} submission: {:?}", trade_type, start_time.elapsed());

        // Poll for confirmation
        match crate::swqos::common::poll_transaction_confirmation(&self.rpc_client, signature).await {
            Ok(transaction_result) => {
                println!(" solana_rpc{} confirmation: {:?}", trade_type, start_time.elapsed());
                Ok(transaction_result)
            }
            Err(e) => {
                eprintln!(" solana_rpc{} confirmation failed: {:?}", trade_type, e);
                Err(e)
            }
        }
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<TransactionResult>> {
        let mut results = Vec::new();
        for transaction in transactions {
            let signature = self.rpc_client.send_transaction_with_config(transaction, RpcSendTransactionConfig{
                skip_preflight: true,
                preflight_commitment: Some(CommitmentLevel::Processed),
                encoding: Some(UiTransactionEncoding::Base64),
                max_retries: Some(3),
                min_context_slot: Some(0),
            }).await?;
            
            // Poll for confirmation to get slot information
            match crate::swqos::common::poll_transaction_confirmation(&self.rpc_client, signature).await {
                Ok(transaction_result) => results.push(transaction_result),
                Err(e) => return Err(e),
            }
        }
        println!(" rpc{} transaction count: {}", trade_type, results.len());
        Ok(results)
    }

    fn get_tip_account(&self) -> Result<String> {
        Ok("".to_string())
    }

    fn get_swqos_type(&self) -> SwqosType {
        SwqosType::Default
    }
}

impl SolRpcClient {
    pub fn new(rpc_client: Arc<SolanaRpcClient>) -> Self {
        Self { rpc_client }
    }
}
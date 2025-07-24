use std::sync::Arc;

use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::{
    commitment_config::CommitmentLevel,
    transaction::VersionedTransaction,
    signature::Signature,
};
use solana_transaction_status::UiTransactionEncoding;

use crate::{common::SolanaRpcClient, swqos::{SwqosType, TradeType}};
use crate::swqos::SwqosClientTrait;
use anyhow::Result;

#[derive(Clone)]
pub struct SolRpcClient {
    pub rpc_client: Arc<SolanaRpcClient>,
}

#[async_trait::async_trait]
impl SwqosClientTrait for SolRpcClient {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<Signature> {
        let signature = self.rpc_client.send_transaction_with_config(transaction, RpcSendTransactionConfig{
            skip_preflight: true,
            preflight_commitment: Some(CommitmentLevel::Processed),
            encoding: Some(UiTransactionEncoding::Base64),
            max_retries: Some(3),
            min_context_slot: Some(0),
        }).await?;

        println!(" rpc{}签名: {:?}", trade_type, signature);
        Ok(signature)
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<Signature>> {
        let mut signatures = Vec::new();
        for transaction in transactions {
            let signature = self.rpc_client.send_transaction_with_config(transaction, RpcSendTransactionConfig{
                skip_preflight: true,
                preflight_commitment: Some(CommitmentLevel::Processed),
                encoding: Some(UiTransactionEncoding::Base64),
                max_retries: Some(3),
                min_context_slot: Some(0),
            }).await?;
            signatures.push(signature);
        }
        println!(" rpc{}签名数量: {}", trade_type, signatures.len());
        Ok(signatures)
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
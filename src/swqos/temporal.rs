
use crate::swqos::common::serialize_transaction_and_encode;
use rand::seq::IndexedRandom;
use reqwest::Client;
use serde_json::json;
use std::{sync::Arc, time::Instant};
use std::time::Duration;
use solana_transaction_status::UiTransactionEncoding;
use solana_sdk::signature::Signature;

use anyhow::Result;
use solana_sdk::transaction::VersionedTransaction;
use crate::swqos::{SwqosType, TradeType};
use crate::swqos::SwqosClientTrait;
use crate::common::types::TransactionResult;

use crate::{common::SolanaRpcClient, constants::swqos::NOZOMI_TIP_ACCOUNTS};


#[derive(Clone)]
pub struct TemporalClient {
    pub rpc_client: Arc<SolanaRpcClient>,
    pub endpoint: String,
    pub auth_token: String,
    pub http_client: Client,
}

#[async_trait::async_trait]
impl SwqosClientTrait for TemporalClient {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<TransactionResult> {
        TemporalClient::send_transaction(self, trade_type, transaction).await
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<TransactionResult>> {
        TemporalClient::send_transactions(self, trade_type, transactions).await
    }

    fn get_tip_account(&self) -> Result<String> {
        let tip_account = *NOZOMI_TIP_ACCOUNTS.choose(&mut rand::rng()).or_else(|| NOZOMI_TIP_ACCOUNTS.first()).unwrap();
        Ok(tip_account.to_string())
    }

    fn get_swqos_type(&self) -> SwqosType {
        SwqosType::Temporal
    }
}

impl TemporalClient {
    pub fn new(rpc_url: String, endpoint: String, auth_token: String) -> Self {
        let rpc_client = SolanaRpcClient::new(rpc_url);
        let http_client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(64)
            .tcp_keepalive(Some(Duration::from_secs(1200)))
            .http2_keep_alive_interval(Duration::from_secs(15))
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        Self { rpc_client: Arc::new(rpc_client), endpoint, auth_token, http_client }
    }

    pub async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<TransactionResult> {
        let start_time = Instant::now();
        let (content, signature) = serialize_transaction_and_encode(transaction, UiTransactionEncoding::Base64).await?;
        println!(" Transaction base64 encoding: {:?}", start_time.elapsed());

        let request_body = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendTransaction",
            "params": [
                content,
                { "encoding": "base64" }
            ]
        }))?;

        let mut url = String::with_capacity(self.endpoint.len() + self.auth_token.len() + 20);
        url.push_str(&self.endpoint);
        url.push_str("/?c=");
        url.push_str(&self.auth_token);

        let response_text = self.http_client.post(&url)
            .body(request_body)
            .header("Content-Type", "application/json")
            .send()
            .await?
            .text()
            .await?;

        let response_json = serde_json::from_str::<serde_json::Value>(&response_text)
            .map_err(|e| anyhow::anyhow!("Failed to parse temporal response as JSON: {} - Response: {}", e, response_text))?;
        
        if response_json.get("result").is_some() {
            println!(" nozomi{} submission: {:?}", trade_type, start_time.elapsed());
            
            // Poll for confirmation
            match crate::swqos::common::poll_transaction_confirmation(&self.rpc_client, signature).await {
                Ok(transaction_result) => {
                    println!(" nozomi{} confirmation: {:?}", trade_type, start_time.elapsed());
                    Ok(transaction_result)
                }
                Err(e) => {
                    eprintln!(" nozomi{} confirmation failed: {:?}", trade_type, e);
                    Err(e)
                }
            }
        } else if let Some(_error) = response_json.get("error") {
            eprintln!(" nozomi{} submission failed: {:?}", trade_type, _error);
            Err(anyhow::anyhow!("nozomi submission failed: {:?}", _error))
        } else {
            eprintln!(" nozomi{} unexpected response format: {}", trade_type, response_text);
            Err(anyhow::anyhow!("nozomi unexpected response format: {}", response_text))
        }
    }

    pub async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<TransactionResult>> {
        let mut results = Vec::new();
        for transaction in transactions {
            let result = self.send_transaction(trade_type, transaction).await?;
            results.push(result);
        }
        Ok(results)
    }
}
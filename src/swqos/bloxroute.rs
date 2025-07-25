use crate::swqos::common::{serialize_transaction_and_encode, FormatBase64VersionedTransaction};
use rand::seq::IndexedRandom;
use reqwest::Client;
use std::{sync::Arc, time::Instant};

use std::time::Duration;
use solana_transaction_status::UiTransactionEncoding;
use solana_sdk::signature::Signature;

use anyhow::Result;
use solana_sdk::transaction::VersionedTransaction;
use crate::swqos::{SwqosType, TradeType};
use crate::swqos::SwqosClientTrait;
use crate::common::types::TransactionResult;

use crate::{common::SolanaRpcClient, constants::swqos::BLOX_TIP_ACCOUNTS};


#[derive(Clone)]
pub struct BloxrouteClient {
    pub endpoint: String,
    pub auth_token: String,
    pub rpc_client: Arc<SolanaRpcClient>,
    pub http_client: Client,
}

#[async_trait::async_trait]
impl SwqosClientTrait for BloxrouteClient {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<TransactionResult> {
        BloxrouteClient::send_transaction(self, trade_type, transaction).await
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<TransactionResult>> {
        BloxrouteClient::send_transactions(self, trade_type, transactions).await
    }

    fn get_tip_account(&self) -> Result<String> {
        let tip_account = *BLOX_TIP_ACCOUNTS.choose(&mut rand::rng()).or_else(|| BLOX_TIP_ACCOUNTS.first()).unwrap();
        Ok(tip_account.to_string())
    }

    fn get_swqos_type(&self) -> SwqosType {
        SwqosType::Bloxroute
    }
}

impl BloxrouteClient {
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

        let body = serde_json::json!({
            "transaction": {
                "content": content,
            },
            "frontRunningProtection": false,
            "useStakedRPCs": true,
        });

        let endpoint = format!("{}/api/v2/submit", self.endpoint);
        let response_text = self.http_client.post(&endpoint)
            .body(body.to_string())
            .header("Content-Type", "application/json")
            .header("Authorization", self.auth_token.clone())
            .send()
            .await?
            .text()
            .await?;

        let response_json = serde_json::from_str::<serde_json::Value>(&response_text)
            .map_err(|e| anyhow::anyhow!("Failed to parse bloxroute response as JSON: {} - Response: {}", e, response_text))?;
        
        if response_json.get("result").is_some() {
            println!(" bloxroute{} submission: {:?}", trade_type, start_time.elapsed());
            
            // Poll for confirmation
            match crate::swqos::common::poll_transaction_confirmation(&self.rpc_client, signature).await {
                Ok(transaction_result) => {
                    println!(" bloxroute{} confirmation: {:?}", trade_type, start_time.elapsed());
                    Ok(transaction_result)
                }
                Err(e) => {
                    eprintln!(" bloxroute{} confirmation failed: {:?}", trade_type, e);
                    Err(e)
                }
            }
        } else if let Some(_error) = response_json.get("error") {
            eprintln!(" bloxroute{} submission failed: {:?}", trade_type, _error);
            Err(anyhow::anyhow!("bloxroute submission failed: {:?}", _error))
        } else {
            eprintln!(" bloxroute{} unexpected response format: {}", trade_type, response_text);
            Err(anyhow::anyhow!("bloxroute unexpected response format: {}", response_text))
        }
    }

    pub async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<TransactionResult>> {
        let start_time = Instant::now();
        println!(" Transaction base64 encoding: {:?}", start_time.elapsed());

        // Extract signatures from all transactions
        let mut signatures = Vec::new();
        for tx in transactions {
            let (_, signature) = serialize_transaction_and_encode(tx, UiTransactionEncoding::Base64).await?;
            signatures.push(signature);
        }

        let body = serde_json::json!({
            "entries":  transactions
                .iter()
                .map(|tx| {
                    serde_json::json!({
                        "transaction": {
                            "content": tx.to_base64_string(),
                        },
                    })
                })
                .collect::<Vec<_>>(),
        });

        let endpoint = format!("{}/api/v2/submit-batch", self.endpoint);
        let response_text = self.http_client.post(&endpoint)
            .body(body.to_string())
            .header("Content-Type", "application/json")
            .header("Authorization", self.auth_token.clone())
            .send()
            .await?
            .text()
            .await?;

        let response_json = serde_json::from_str::<serde_json::Value>(&response_text)
            .map_err(|e| anyhow::anyhow!("Failed to parse bloxroute batch response as JSON: {} - Response: {}", e, response_text))?;
        
        if response_json.get("result").is_some() {
            println!(" bloxroute{} submission: {:?}", trade_type, start_time.elapsed());
            
            // Convert signatures to TransactionResults by polling for confirmation
            let mut results = Vec::new();
            for signature in signatures {
                match crate::swqos::common::poll_transaction_confirmation(&self.rpc_client, signature).await {
                    Ok(transaction_result) => results.push(transaction_result),
                    Err(e) => {
                        eprintln!(" bloxroute{} confirmation failed for signature {}: {:?}", trade_type, signature, e);
                        return Err(e);
                    }
                }
            }
            
            println!(" bloxroute{} transaction count: {}", trade_type, results.len());
            Ok(results)
        } else if let Some(_error) = response_json.get("error") {
            eprintln!(" bloxroute{} submission failed: {:?}", trade_type, _error);
            Err(anyhow::anyhow!("bloxroute batch submission failed: {:?}", _error))
        } else {
            eprintln!(" bloxroute{} unexpected response format: {}", trade_type, response_text);
            Err(anyhow::anyhow!("bloxroute batch unexpected response format: {}", response_text))
        }
    }
}
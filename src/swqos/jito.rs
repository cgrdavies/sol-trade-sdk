
use crate::swqos::common::{serialize_transaction_and_encode, FormatBase64VersionedTransaction};
use rand::seq::IndexedRandom;
use reqwest::Client;
use serde_json::json;
use std::{sync::Arc, time::Instant};

use std::time::Duration;
use solana_transaction_status::UiTransactionEncoding;
use solana_sdk::signature::Signature;

use anyhow::{anyhow, Result};
use solana_sdk::transaction::VersionedTransaction;
use crate::swqos::{SwqosType, TradeType};
use crate::swqos::SwqosClientTrait;
use crate::common::types::TransactionResult;

use crate::{common::SolanaRpcClient, constants::swqos::JITO_TIP_ACCOUNTS};


pub struct JitoClient {
    pub endpoint: String,
    pub auth_token: String,
    pub rpc_client: Arc<SolanaRpcClient>,
    pub http_client: Client,
}

#[async_trait::async_trait]
impl SwqosClientTrait for JitoClient {
    async fn send_transaction(&self, trade_type: TradeType, transaction: &VersionedTransaction) -> Result<TransactionResult> {
        JitoClient::send_transaction(self, trade_type, transaction).await
    }

    async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<TransactionResult>> {
        JitoClient::send_transactions(self, trade_type, transactions).await
    }

    fn get_tip_account(&self) -> Result<String> {
        if let Some(acc) = JITO_TIP_ACCOUNTS.choose(&mut rand::rng()) {
            Ok(acc.to_string())
        } else {
            Err(anyhow::anyhow!("no valid tip accounts found"))
        }
    }

    fn get_swqos_type(&self) -> SwqosType {
        SwqosType::Jito
    }
}

impl JitoClient {
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
            "id": 1,
            "jsonrpc": "2.0", 
            "method": "sendTransaction",
            "params": [
                content,
                {
                    "encoding": "base64"
                }
            ]
        }))?;

        let endpoint = format!("{}/api/v1/transactions", self.endpoint);
        let response_text = self.http_client.post(&endpoint)
            .body(request_body)
            .header("Content-Type", "application/json")
            .send()
            .await?
            .text()
            .await?;

        let response_json = serde_json::from_str::<serde_json::Value>(&response_text)
            .map_err(|e| anyhow::anyhow!("Failed to parse jito response as JSON: {} - Response: {}", e, response_text))?;
        
        if response_json.get("result").is_some() {
            println!(" jito{} submission: {:?}", trade_type, start_time.elapsed());
            
            // Poll for confirmation
            match crate::swqos::common::poll_transaction_confirmation(&self.rpc_client, signature).await {
                Ok(transaction_result) => {
                    println!(" jito{} confirmation: {:?}", trade_type, start_time.elapsed());
                    Ok(transaction_result)
                }
                Err(e) => {
                    eprintln!(" jito{} confirmation failed: {:?}", trade_type, e);
                    Err(e)
                }
            }
        } else if let Some(_error) = response_json.get("error") {
            eprintln!(" jito{} submission failed: {:?}", trade_type, _error);
            Err(anyhow::anyhow!("jito submission failed: {:?}", _error))
        } else {
            eprintln!(" jito{} unexpected response format: {}", trade_type, response_text);
            Err(anyhow::anyhow!("jito unexpected response format: {}", response_text))
        }
    }

    pub async fn send_transactions(&self, trade_type: TradeType, transactions: &Vec<VersionedTransaction>) -> Result<Vec<TransactionResult>> {
        let start_time = Instant::now();
        let txs_base64 = transactions.iter().map(|tx| tx.to_base64_string()).collect::<Vec<String>>();
        
        // Extract signatures from all transactions
        let mut signatures = Vec::new();
        for tx in transactions {
            let (_, signature) = serialize_transaction_and_encode(tx, UiTransactionEncoding::Base64).await?;
            signatures.push(signature);
        }
        
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "sendBundle",
            "params": [
                txs_base64,
                { "encoding": "base64" }
            ],
            "id": 1,
        });

        let endpoint = format!("{}/api/v1/bundles", self.endpoint);
        let response_text = self.http_client.post(&endpoint)
            .body(body.to_string())
            .header("Content-Type", "application/json")
            .send()
            .await?
            .text()
            .await?;

        let response_json = serde_json::from_str::<serde_json::Value>(&response_text)
            .map_err(|e| anyhow::anyhow!("Failed to parse jito bundle response as JSON: {} - Response: {}", e, response_text))?;
        
        if response_json.get("result").is_some() {
            println!(" jito{} submission: {:?}", trade_type, start_time.elapsed());
        } else if let Some(_error) = response_json.get("error") {
            eprintln!(" jito{} submission failed: {:?}", trade_type, _error);
            return Err(anyhow::anyhow!("jito bundle submission failed: {:?}", _error));
        } else {
            eprintln!(" jito{} unknown response format: {}", trade_type, response_text);
            return Err(anyhow::anyhow!("jito bundle unexpected response format: {}", response_text));
        }

        // Convert signatures to TransactionResults by polling for confirmation
        let mut results = Vec::new();
        for signature in signatures {
            match crate::swqos::common::poll_transaction_confirmation(&self.rpc_client, signature).await {
                Ok(transaction_result) => results.push(transaction_result),
                Err(e) => {
                    eprintln!(" jito{} confirmation failed for signature {}: {:?}", trade_type, signature, e);
                    return Err(e);
                }
            }
        }
        
        println!(" jito{} transaction count: {}", trade_type, results.len());
        Ok(results)
    }
}
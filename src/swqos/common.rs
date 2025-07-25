use bincode::serialize;
use serde_json::json;
use solana_client::rpc_client::SerializableTransaction;
use solana_sdk::signature::Signature;
use solana_sdk::transaction::Transaction;
use solana_transaction_status::{TransactionConfirmationStatus, UiTransactionEncoding};
use std::str::FromStr;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use crate::common::types::{SolanaRpcClient, TransactionResult};
use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::{self, STANDARD};
use reqwest::Client;
use solana_sdk::transaction::VersionedTransaction;

pub trait FormatBase64VersionedTransaction {
    fn to_base64_string(&self) -> String;
}

impl FormatBase64VersionedTransaction for VersionedTransaction {
    fn to_base64_string(&self) -> String {
        let tx_bytes = bincode::serialize(self).unwrap();
        general_purpose::STANDARD.encode(tx_bytes)
    }
}

pub async fn poll_transaction_confirmation(rpc: &SolanaRpcClient, txt_sig: Signature) -> Result<TransactionResult> {
    let timeout: Duration = Duration::from_secs(5);
    let interval: Duration = Duration::from_millis(1000);
    let start: Instant = Instant::now();

    loop {
        if start.elapsed() >= timeout {
            return Err(anyhow::anyhow!("Transaction {}'s confirmation timed out", txt_sig));
        }

        let status = rpc.get_signature_statuses(&[txt_sig]).await?;

        match status.value[0].clone() {
            Some(status) => {
                if status.err.is_none()
                    && (status.confirmation_status == Some(TransactionConfirmationStatus::Confirmed)
                        || status.confirmation_status == Some(TransactionConfirmationStatus::Finalized))
                {
                    return Ok(TransactionResult::new(txt_sig, status.slot));
                }
                if status.err.is_some() {
                    return Err(anyhow::anyhow!(status.err.unwrap()));
                }
            }
            None => {
                sleep(interval).await;
            }
        }
    }
}

pub async fn send_nb_transaction(client: Client, endpoint: &str, auth_token: &str, transaction: &Transaction) -> Result<Signature, anyhow::Error> {
    // Serialize transaction
    let serialized = bincode::serialize(transaction)
        .map_err(|e| anyhow::anyhow!("Failed to serialize transaction: {}", e))?;
    
    // Base64 encoding
    let encoded = STANDARD.encode(serialized);

    let request_data = json!({
        "transaction": {
            "content": encoded
        },
        "frontRunningProtection": true
    });

    let url = format!("{}/api/v2/submit", endpoint);
    let response = client
        .post(url)
        .header("Authorization", auth_token)
        .header("Content-Type", "application/json")
        .json(&request_data)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Request failed: {}", e))?;

    let resp = response.json::<serde_json::Value>().await
        .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

    if let Some(reason) = resp["reason"].as_str() {
        return Err(anyhow::anyhow!(reason.to_string()));
    }

    let signature = resp["signature"].as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing signature field in response"))?;

    let signature = Signature::from_str(signature)
        .map_err(|e| anyhow::anyhow!("Invalid signature: {}", e))?;

    Ok(signature)
}

pub async fn serialize_and_encode(
    transaction: &Vec<u8>,
    encoding: UiTransactionEncoding,
) -> Result<String> {
    let serialized = match encoding {
        UiTransactionEncoding::Base58 => bs58::encode(transaction).into_string(),
        UiTransactionEncoding::Base64 => STANDARD.encode(transaction),
        _ => return Err(anyhow::anyhow!("Unsupported encoding")),
    };
    Ok(serialized)
}

pub async fn serialize_transaction_and_encode(
    transaction: &impl SerializableTransaction,
    encoding: UiTransactionEncoding,
) -> Result<(String, Signature)> {
    let signature = transaction.get_signature();
    let serialized_tx = serialize(transaction)?;
    let serialized = match encoding {
        UiTransactionEncoding::Base58 => bs58::encode(serialized_tx).into_string(),
        UiTransactionEncoding::Base64 => STANDARD.encode(serialized_tx),
        _ => return Err(anyhow::anyhow!("Unsupported encoding")),
    };
    Ok((serialized, *signature))
}

pub async fn serialize_smart_transaction_and_encode(
    transaction: &impl SerializableTransaction,
    encoding: UiTransactionEncoding,
) -> Result<(String, Signature)> {
    let signature = transaction.get_signature();
    let serialized_tx = serialize(transaction)?;
    let serialized = match encoding {
        UiTransactionEncoding::Base58 => bs58::encode(serialized_tx).into_string(),
        UiTransactionEncoding::Base64 => STANDARD.encode(serialized_tx),
        _ => return Err(anyhow::anyhow!("Unsupported encoding")),
    };
    Ok((serialized, *signature))
}
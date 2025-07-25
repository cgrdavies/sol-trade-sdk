use anyhow::{anyhow, Result};
use futures::future::select_all;
use solana_hash::Hash;
use solana_sdk::{instruction::Instruction, pubkey::Pubkey, signature::Keypair};
use std::{str::FromStr, sync::Arc};
use tokio::task::JoinHandle;

use crate::{
    common::{PriorityFee, types::TransactionResult},
    swqos::{SwqosType, SwqosClient, TradeType},
    trading::core::timer::TradeTimer,
    trading::common::{
        build_rpc_transaction, build_sell_tip_transaction_with_priority_fee,
        build_sell_transaction, build_tip_transaction_with_priority_fee,
    },
};

/// Parallel execution function for transactions - returns the first successfully confirmed transaction result (signature and slot)
pub async fn parallel_execute_with_tips(
    swqos_clients: Vec<Arc<SwqosClient>>,
    payer: Arc<Keypair>,
    instructions: Vec<Instruction>,
    priority_fee: PriorityFee,
    lookup_table_key: Option<Pubkey>,
    recent_blockhash: Hash,
    data_size_limit: u32,
    trade_type: TradeType,
) -> Result<TransactionResult> {
    let cores = core_affinity::get_core_ids().unwrap();
    let mut handles: Vec<JoinHandle<Result<TransactionResult>>> = vec![];

    for i in 0..swqos_clients.len() {
        let swqos_client = swqos_clients[i].clone();
        let payer = payer.clone();
        let instructions = instructions.clone();
        let mut priority_fee = priority_fee.clone();
        let core_id = cores[i % cores.len()];

        let handle = tokio::spawn(async move {
            core_affinity::set_for_current(core_id);

            let mut timer = TradeTimer::new(format!("Building transaction instruction: {:?}", swqos_client.get_swqos_type()));

            let transaction = if matches!(trade_type, TradeType::Sell)
                && swqos_client.get_swqos_type() == SwqosType::Default
            {
                build_sell_transaction(
                    payer,
                    &priority_fee,
                    instructions,
                    lookup_table_key,
                    recent_blockhash,
                )
                .await?
            } else if swqos_client.get_swqos_type() == SwqosType::Default {
                build_rpc_transaction(
                    payer,
                    &priority_fee,
                    instructions,
                    lookup_table_key,
                    recent_blockhash,
                    data_size_limit,
                )
                .await?
            } else {
                let tip_account = Pubkey::from_str(&swqos_client.get_tip_account()?)?;

                if matches!(trade_type, TradeType::Sell) {
                    build_sell_tip_transaction_with_priority_fee(
                        payer.clone(),
                        &priority_fee,
                        instructions,
                        &tip_account,
                        lookup_table_key,
                        recent_blockhash,
                    )
                    .await?
                } else {
                    build_tip_transaction_with_priority_fee(
                        payer.clone(),
                        &priority_fee,
                        instructions,
                        &tip_account,
                        lookup_table_key,
                        recent_blockhash,
                        data_size_limit,
                    )
                    .await?
                }
            };

            timer.stage(format!("Submitting transaction instruction: {:?}", swqos_client.get_swqos_type()));

            let transaction_result = swqos_client
                .send_transaction(trade_type, &transaction)
                .await?;

            timer.finish();
            Ok::<TransactionResult, anyhow::Error>(transaction_result)
        });

        handles.push(handle);
    }

    // Wait for the first successful task to complete
    let mut errors = Vec::new();
    let mut remaining_handles = handles;
    
    while !remaining_handles.is_empty() {
        let (result, _index, remaining) = select_all(remaining_handles).await;
        remaining_handles = remaining;
        
        match result {
            Ok(Ok(transaction_result)) => {
                println!("Successfully obtained first confirmed transaction - signature: {}, slot: {}", 
                         transaction_result.signature, transaction_result.slot);
                // Cancel remaining tasks
                for handle in remaining_handles {
                    handle.abort();
                }
                return Ok(transaction_result);
            }
            Ok(Err(e)) => {
                errors.push(format!("Task error: {}", e));
                println!("Task failed: {}", e);
            }
            Err(e) => {
                errors.push(format!("Join error: {}", e));
                println!("Task join error: {}", e);
            }
        }
    }

    Err(anyhow!("All tasks failed: {:?}", errors))
}

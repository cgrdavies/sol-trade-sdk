use anyhow::{anyhow, Result};
use std::sync::Arc;
use solana_sdk::signature::Signature;
use crate::common::types::TransactionResult;

use super::{
    parallel::parallel_execute_with_tips,
    params::{BuyParams, BuyWithTipParams, SellParams, SellWithTipParams},
    timer::TradeTimer,
    traits::{InstructionBuilder, TradeExecutor},
};
use crate::{
    swqos::TradeType,
    trading::common::{build_rpc_transaction, build_sell_transaction},
};

const MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT: u32 = 256 * 1024;

/// 通用交易执行器实现
pub struct GenericTradeExecutor {
    instruction_builder: Arc<dyn InstructionBuilder>,
    protocol_name: &'static str,
}

impl GenericTradeExecutor {
    pub fn new(
        instruction_builder: Arc<dyn InstructionBuilder>,
        protocol_name: &'static str,
    ) -> Self {
        Self {
            instruction_builder,
            protocol_name,
        }
    }
}

#[async_trait::async_trait]
impl TradeExecutor for GenericTradeExecutor {
    async fn buy(&self, mut params: BuyParams) -> Result<Signature> {
        if params.data_size_limit == 0 {
            params.data_size_limit = MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT;
        }
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }
        let rpc = params.rpc.as_ref().unwrap().clone();
        let mut timer = TradeTimer::new("Building buy transaction instruction");
        // Build instructions
        let instructions = self
            .instruction_builder
            .build_buy_instructions(&params)
            .await?;
        timer.stage("Building RPC transaction instruction");

        // Build transaction
        let transaction = build_rpc_transaction(
            params.payer.clone(),
            &params.priority_fee,
            instructions,
            params.lookup_table_key,
            params.recent_blockhash,
            params.data_size_limit,
        )
        .await?;
        timer.stage("RPC submission and confirmation");

        // Send transaction
        let signature = rpc.send_and_confirm_transaction(&transaction).await?;
        timer.finish();

        Ok(signature)
    }

    async fn buy_with_tip(&self, mut params: BuyWithTipParams) -> Result<TransactionResult> {
        if params.data_size_limit == 0 {
            params.data_size_limit = MAX_LOADED_ACCOUNTS_DATA_SIZE_LIMIT;
        }
        let timer = TradeTimer::new("Building buy transaction instruction");

        // 验证参数 - 转换为BuyParams进行验证
        let buy_params = BuyParams {
            rpc: params.rpc,
            payer: params.payer.clone(),
            mint: params.mint,
            creator: params.creator,
            sol_amount: params.sol_amount,
            slippage_basis_points: params.slippage_basis_points,
            priority_fee: params.priority_fee.clone(),
            lookup_table_key: params.lookup_table_key,
            recent_blockhash: params.recent_blockhash,
            data_size_limit: params.data_size_limit,
            protocol_params: params.protocol_params.clone(),
        };

        // 构建指令
        let instructions = self
            .instruction_builder
            .build_buy_instructions(&buy_params)
            .await?;

        timer.finish();

        // 并行执行交易
        let signature = parallel_execute_with_tips(
            params.swqos_clients,
            params.payer,
            instructions,
            params.priority_fee,
            params.lookup_table_key,
            params.recent_blockhash,
            params.data_size_limit,
            TradeType::Buy,
        )
        .await?;

        Ok(signature)
    }

    async fn sell(&self, params: SellParams) -> Result<Signature> {
        if params.rpc.is_none() {
            return Err(anyhow!("RPC is not set"));
        }
        let rpc = params.rpc.as_ref().unwrap().clone();
        let mut timer = TradeTimer::new("Building sell transaction instruction");

        // 构建指令
        let instructions = self
            .instruction_builder
            .build_sell_instructions(&params)
            .await?;
        timer.stage("Sell transaction instruction");

        // 构建交易
        let transaction = build_sell_transaction(
            params.payer.clone(),
            &params.priority_fee,
            instructions,
            params.lookup_table_key,
            params.recent_blockhash,
        )
        .await?;
        timer.stage("Sell transaction signature");

        // 发送交易
        let signature = rpc.send_and_confirm_transaction(&transaction).await?;
        timer.finish();

        Ok(signature)
    }

    async fn sell_with_tip(&self, params: SellWithTipParams) -> Result<TransactionResult> {
        let timer = TradeTimer::new("Building sell transaction instruction");

        // 转换为SellParams进行指令构建
        let sell_params = SellParams {
            rpc: params.rpc,
            payer: params.payer.clone(),
            mint: params.mint,
            creator: params.creator,
            token_amount: params.token_amount,
            slippage_basis_points: params.slippage_basis_points,
            priority_fee: params.priority_fee.clone(),
            lookup_table_key: params.lookup_table_key,
            recent_blockhash: params.recent_blockhash,
            protocol_params: params.protocol_params.clone(),
        };

        // 构建指令
        let instructions = self
            .instruction_builder
            .build_sell_instructions(&sell_params)
            .await?;

        timer.finish();

        // 并行执行交易
        let signature = parallel_execute_with_tips(
            params.swqos_clients,
            params.payer,
            instructions,
            params.priority_fee,
            params.lookup_table_key,
            params.recent_blockhash,
            0,
            TradeType::Sell,
        )
        .await?;

        Ok(signature)
    }

    fn protocol_name(&self) -> &'static str {
        self.protocol_name
    }
}

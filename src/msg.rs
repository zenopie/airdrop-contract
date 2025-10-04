use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Binary, Uint128};

/// Instantiate message
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,
    pub erth_token_contract: String,
    pub erth_token_hash: String,
}

/// Execute messages
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Claim airdrop tokens with merkle proof
    Claim {
        amount: Uint128,
        proof: Vec<String>,
    },
    /// Reset airdrop with new merkle root (owner only)
    ResetAirdrop {
        merkle_root: String,
        total_stake: Uint128,
    },
    /// SNIP-20 Receive hook
    Receive {
        sender: String,
        from: String,
        amount: Uint128,
        msg: Binary,
        #[serde(skip_serializing_if = "Option::is_none")]
        memo: Option<String>,
    },
}

/// Messages sent within Receive
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveMsg {
    AllocationSend { allocation_id: u32 },
}

/// Query messages
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Get current airdrop round info
    GetCurrentRound {},
    /// Check if address has claimed
    HasClaimed { address: String },
    /// Get config
    GetConfig {},
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CurrentRoundResponse {
    pub round_id: u64,
    pub merkle_root: String,
    pub total_amount: Uint128,
    pub total_stake: Uint128,
    pub claimed_amount: Uint128,
    pub start_time: u64,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct HasClaimedResponse {
    pub has_claimed: bool,
    pub amount: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfigResponse {
    pub owner: String,
    pub erth_token_contract: String,
    pub erth_token_hash: String,
}

/// Migration message
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct MigrateMsg {}

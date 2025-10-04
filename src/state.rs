use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use secret_toolkit::storage::{Item, Keymap};
use cosmwasm_std::{Addr, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub erth_token_contract: Addr,
    pub erth_token_hash: String,
    pub allocation_contract: Addr,
    pub allocation_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct State {
    pub pending_reward: Uint128,
    pub current_round_id: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AirdropRound {
    pub round_id: u64,
    pub merkle_root: String,
    pub total_amount: Uint128,
    pub total_stake: Uint128,
    pub claimed_amount: Uint128,
    pub start_time: u64,
}

// Singleton storage using Secret Network toolkit
pub const CONFIG: Item<Config> = Item::new(b"config");
pub const STATE: Item<State> = Item::new(b"state");
pub const CURRENT_ROUND: Item<AirdropRound> = Item::new(b"current_round");

// Claims storage with composite key (round_id, address)
pub const CLAIMS: Keymap<(u64, Addr), String> = Keymap::new(b"claims");

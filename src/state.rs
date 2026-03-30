use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use secret_toolkit::storage::{Item, Keymap};
use cosmwasm_std::{Addr, Uint128, Deps, StdResult, to_binary, QueryRequest, WasmQuery};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub owner: Addr,
    pub backend_wallet: Addr,
    pub registry_contract: Addr,
    pub registry_hash: String,
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

// Minimal registry types
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistryQueryMsg {
    GetContracts { names: Vec<String> },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ContractInfo {
    pub address: Addr,
    pub code_hash: String,
}

#[derive(Serialize, Deserialize)]
pub struct ContractResponse {
    pub name: String,
    pub info: ContractInfo,
}

#[derive(Serialize, Deserialize)]
pub struct AllContractsResponse {
    pub contracts: Vec<ContractResponse>,
}

pub fn query_registry(
    deps: &Deps,
    registry_addr: &Addr,
    registry_hash: &str,
    names: Vec<&str>,
) -> StdResult<Vec<ContractInfo>> {
    let query_msg = RegistryQueryMsg::GetContracts {
        names: names.iter().map(|n| n.to_string()).collect(),
    };
    let expected_count = names.len();
    let response: AllContractsResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: registry_addr.to_string(),
        code_hash: registry_hash.to_string(),
        msg: to_binary(&query_msg)?,
    }))?;
    if response.contracts.len() != expected_count {
        return Err(cosmwasm_std::StdError::generic_err(
            format!("Registry returned {} contracts, expected {}", response.contracts.len(), expected_count)
        ));
    }
    Ok(response.contracts.into_iter().map(|c| c.info).collect())
}

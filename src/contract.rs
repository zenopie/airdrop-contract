use cosmwasm_std::{
    entry_point, from_binary, to_binary, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    QueryResponse, Response, StdError, StdResult, Uint128, WasmMsg,
};
use secret_toolkit::snip20;
use sha2::{Digest, Sha256};
use crate::msg::{
    CurrentRoundResponse, ExecuteMsg, HasClaimedResponse,
    InstantiateMsg, QueryMsg, ReceiveMsg, SendMsg,
};
use crate::state::{AirdropRound, Config, State, CLAIMS, CONFIG, STATE, CURRENT_ROUND};

/// Verify merkle proof using SHA256 and sorted pair hashing
fn verify_merkle_proof(
    proof: &[String],
    root: &str,
    leaf_hash: &str,
) -> Result<bool, StdError> {
    let mut computed_hash = hex_to_bytes(leaf_hash)?;

    for proof_element in proof {
        let proof_bytes = hex_to_bytes(proof_element)?;

        // Sorted pair hashing: sort before concatenating
        let combined = if computed_hash <= proof_bytes {
            [computed_hash, proof_bytes].concat()
        } else {
            [proof_bytes, computed_hash].concat()
        };

        // Hash the combined bytes
        let mut hasher = Sha256::new();
        hasher.update(&combined);
        computed_hash = hasher.finalize().to_vec();
    }

    let computed_root = format!("0x{}", hex::encode(computed_hash));
    Ok(computed_root == root)
}

/// Convert hex string (with or without 0x prefix) to bytes
fn hex_to_bytes(hex_str: &str) -> Result<Vec<u8>, StdError> {
    let hex_str = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    hex::decode(hex_str).map_err(|e| StdError::generic_err(format!("Invalid hex: {}", e)))
}

/// Compute leaf hash from address and amount (matching backend: "address:amount")
fn compute_leaf_hash(address: &str, amount: &str) -> String {
    let leaf_str = format!("{}:{}", address, amount);
    let mut hasher = Sha256::new();
    hasher.update(leaf_str.as_bytes());
    format!("0x{}", hex::encode(hasher.finalize()))
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let owner = deps.api.addr_validate(&msg.owner)?;
    let backend_wallet = deps.api.addr_validate(&msg.backend_wallet)?;
    let erth_token_contract = deps.api.addr_validate(&msg.erth_token_contract)?;
    let allocation_contract = deps.api.addr_validate(&msg.allocation_contract)?;

    let config = Config {
        owner,
        backend_wallet,
        erth_token_contract,
        erth_token_hash: msg.erth_token_hash.clone(),
        allocation_contract,
        allocation_hash: msg.allocation_hash.clone(),
    };
    CONFIG.save(deps.storage, &config)?;

    let state = State {
        pending_reward: Uint128::zero(),
        current_round_id: 0,
    };
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("owner", msg.owner))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Claim { amount, proof } => execute_claim(deps, env, info, amount, proof),
        ExecuteMsg::ResetAirdrop { merkle_root, total_stake } => {
            execute_reset_airdrop(deps, env, info, merkle_root, total_stake)
        }
        ExecuteMsg::UpdateConfig { config } => {
            execute_update_config(deps, env, info, config)
        }
        ExecuteMsg::Receive { sender, from, amount, msg, memo: _ } => {
            receive_dispatch(deps, env, info, sender, from, amount, msg)
        }
    }
}

fn receive_dispatch(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    _sender: String,
    _from: String,
    amount: Uint128,
    msg: Binary,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // Verify it's from ERTH token contract
    if info.sender != config.erth_token_contract {
        return Err(StdError::generic_err("Unauthorized: only ERTH token can send"));
    }

    let receive_msg: ReceiveMsg = from_binary(&msg)?;

    match receive_msg {
        ReceiveMsg::AllocationSend { allocation_id: _ } => {
            receive_allocation(deps, amount)
        }
    }
}

fn receive_allocation(
    deps: DepsMut,
    amount: Uint128,
) -> StdResult<Response> {
    let mut state = STATE.load(deps.storage)?;

    state.pending_reward += amount;
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("action", "receive_allocation")
        .add_attribute("amount", amount.to_string()))
}

fn execute_claim(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    stake_amount: Uint128,
    proof: Vec<String>,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;
    let mut round = CURRENT_ROUND.load(deps.storage)
        .map_err(|_| StdError::generic_err("No active airdrop round"))?;

    // Check if already claimed in this round
    if CLAIMS.get(deps.storage, &(round.round_id, info.sender.clone())).is_some() {
        return Err(StdError::generic_err("Already claimed for this round"));
    }

    // Compute leaf hash (backend stores stake amounts in merkle tree)
    let leaf_hash = compute_leaf_hash(&info.sender.to_string(), &stake_amount.to_string());

    // Verify merkle proof
    if !verify_merkle_proof(&proof, &round.merkle_root, &leaf_hash)? {
        return Err(StdError::generic_err("Invalid merkle proof"));
    }

    // Calculate proportional claim amount: (user_stake / total_stake) * total_amount
    let claim_amount = round.total_amount
        .multiply_ratio(stake_amount, round.total_stake);

    // Mark as claimed for this round
    CLAIMS.insert(deps.storage, &(round.round_id, info.sender.clone()), &stake_amount.to_string())?;

    // Update claimed amount
    round.claimed_amount += claim_amount;
    CURRENT_ROUND.save(deps.storage, &round)?;

    // Send SNIP-20 transfer
    let send_msg = snip20::transfer_msg(
        info.sender.to_string(),
        claim_amount,
        None,
        None,
        256,
        config.erth_token_hash.clone(),
        config.erth_token_contract.to_string(),
    )?;

    // Claim allocation from allocation contract
    let allocation_claim_msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.allocation_contract.to_string(),
        code_hash: config.allocation_hash.clone(),
        msg: to_binary(&SendMsg::ClaimAllocation {
            allocation_id: 4,
        })?,
        funds: vec![],
    });

    Ok(Response::new()
        .add_message(send_msg)
        .add_message(allocation_claim_msg)
        .add_attribute("action", "claim")
        .add_attribute("address", info.sender.to_string())
        .add_attribute("claim_amount", claim_amount.to_string())
        .add_attribute("round_id", round.round_id.to_string()))
}

fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    config: Config,
) -> StdResult<Response> {
    let old_config = CONFIG.load(deps.storage)?;

    if info.sender != old_config.owner {
        return Err(StdError::generic_err("Unauthorized"));
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "update_config"))
}

fn execute_reset_airdrop(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    merkle_root: String,
    total_stake: Uint128,
) -> StdResult<Response> {
    let config = CONFIG.load(deps.storage)?;

    // Only backend wallet can reset
    if info.sender != config.backend_wallet {
        return Err(StdError::generic_err("Unauthorized: only backend wallet can reset airdrop"));
    }

    let mut state = STATE.load(deps.storage)?;

    // Calculate unclaimed from previous round (if exists)
    let unclaimed = if state.current_round_id > 0 {
        let prev_round = CURRENT_ROUND.load(deps.storage)?;
        prev_round.total_amount - prev_round.claimed_amount
    } else {
        Uint128::zero()
    };

    // Increment round
    state.current_round_id += 1;

    // New airdrop total = pending_reward + unclaimed from previous
    let new_total = state.pending_reward + unclaimed;

    let new_round = AirdropRound {
        round_id: state.current_round_id,
        merkle_root: merkle_root.clone(),
        total_amount: new_total,
        total_stake,
        claimed_amount: Uint128::zero(),
        start_time: env.block.time.seconds(),
    };
    CURRENT_ROUND.save(deps.storage, &new_round)?;

    // Reset pending_reward
    state.pending_reward = Uint128::zero();
    STATE.save(deps.storage, &state)?;

    // Old claims remain with their round_id key
    // New round uses new round_id, so all addresses can claim again

    Ok(Response::new()
        .add_attribute("action", "reset_airdrop")
        .add_attribute("new_round_id", state.current_round_id.to_string())
        .add_attribute("merkle_root", merkle_root)
        .add_attribute("total_amount", new_total.to_string())
        .add_attribute("unclaimed_rollover", unclaimed.to_string()))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<QueryResponse> {
    match msg {
        QueryMsg::GetCurrentRound {} => to_binary(&query_current_round(deps)?),
        QueryMsg::HasClaimed { address } => to_binary(&query_has_claimed(deps, address)?),
        QueryMsg::GetConfig {} => to_binary(&query_config(deps)?),
    }
}

fn query_current_round(deps: Deps) -> StdResult<CurrentRoundResponse> {
    let round = CURRENT_ROUND.load(deps.storage)?;
    Ok(CurrentRoundResponse {
        round_id: round.round_id,
        merkle_root: round.merkle_root,
        total_amount: round.total_amount,
        total_stake: round.total_stake,
        claimed_amount: round.claimed_amount,
        start_time: round.start_time,
    })
}

fn query_has_claimed(deps: Deps, address: String) -> StdResult<HasClaimedResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let round = CURRENT_ROUND.load(deps.storage)?;
    let amount = CLAIMS.get(deps.storage, &(round.round_id, addr));

    Ok(HasClaimedResponse {
        has_claimed: amount.is_some(),
        amount,
    })
}

fn query_config(deps: Deps) -> StdResult<Config> {
    CONFIG.load(deps.storage)
}

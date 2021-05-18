use cosmwasm_std::{log, to_binary, Api, CosmosMsg, Env, Extern, HandleResponse, HandleResult, HumanAddr, Querier, StdResult, Storage, Uint128, WasmMsg, QueryRequest, WasmQuery, StdError};

use crate::state::{
    read_config, read_current_n, read_pool_info, rewards_read, rewards_store, store_current_n,
    store_pool_info, Config, PoolInfo, RewardInfo,
};
use nebula_protocol::factory::{ClusterExistsResponse, QueryMsg::ClusterExists};

use cw20::Cw20HandleMsg;

// deposit_reward must be from reward token contract
pub fn deposit_reward<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    rewards_amount: Uint128,
) -> HandleResult {
    let n = read_current_n(&deps.storage)?;
    let mut pool_info = read_pool_info(&deps.storage, n)?;
    pool_info.reward_sum += rewards_amount;
    store_pool_info(&mut deps.storage, n, &pool_info)?;
    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "deposit_reward"),
            log("rewards_amount", rewards_amount.to_string()),
        ],
        data: None,
    })
}

pub fn record_penalty<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    reward_owner: HumanAddr,
    penalty_amount: Uint128,
) -> HandleResult {
    let n = read_current_n(&deps.storage)?;

    let cluster = env.message.sender;
    let cfg = read_config(&deps.storage)?;

    let res: ClusterExistsResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: cfg.owner.clone(),
        msg: to_binary(&ClusterExists {
            contract_addr: cluster,
        })?,
    }))?;

    if !res.exists {
        return Err(StdError::unauthorized());
    }

    let reward_owner = deps.api.canonical_address(&reward_owner)?;
    let mut reward_info = rewards_read(&deps.storage, &reward_owner)?;
    before_share_change(&deps.storage, &mut reward_info)?;

    let mut pool_info = read_pool_info(&deps.storage, n)?;
    pool_info.penalty_sum += penalty_amount;
    reward_info.penalty += penalty_amount;

    rewards_store(&mut deps.storage, &reward_owner, &reward_info)?;
    store_pool_info(&mut deps.storage, n, &pool_info)?;

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "record_penalty"),
            log("penalty_amount", penalty_amount.to_string()),
        ],
        data: None,
    })
}

// withdraw all rewards or single reward depending on asset_token
pub fn withdraw_reward<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
) -> HandleResult {
    let reward_owner = deps.api.canonical_address(&env.message.sender)?;
    let mut reward_info = rewards_read(&deps.storage, &reward_owner)?;
    before_share_change(&deps.storage, &mut reward_info)?;

    let amount = reward_info.pending_reward;
    reward_info.pending_reward = Uint128::zero();
    rewards_store(&mut deps.storage, &reward_owner, &reward_info)?;

    let config: Config = read_config(&deps.storage)?;

    Ok(HandleResponse {
        messages: vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: deps.api.human_address(&config.nebula_token)?,
            msg: to_binary(&Cw20HandleMsg::Transfer {
                recipient: env.message.sender,
                amount,
            })?,
            send: vec![],
        })],
        log: vec![
            log("action", "withdraw"),
            log("amount", amount.to_string()),
        ],
        data: None,
    })
}

pub fn increment_n<S: Storage>(storage: &mut S) -> StdResult<()> {
    let current_n = read_current_n(storage)?;

    let new_pool = PoolInfo {
        n: current_n + 1,
        penalty_sum: Uint128::zero(),
        reward_sum: Uint128::zero(),
    };

    store_current_n(storage, current_n + 1)?;
    store_pool_info(storage, current_n + 1, &new_pool)?;

    Ok(())
}

// transform penalty into pending reward
// the penalty must be from before the current n
pub fn before_share_change<S: Storage>(storage: &S, reward_info: &mut RewardInfo) -> StdResult<()> {
    let n = read_current_n(storage)?;
    if reward_info.penalty != Uint128::zero() && reward_info.n != n {
        let pool_info = read_pool_info(storage, reward_info.n)?;

        // using integers here .. do we care if the remaining fractions of nebula stay in this contract?
        reward_info.pending_reward += Uint128(
            pool_info.reward_sum.u128() * reward_info.penalty.u128() / pool_info.penalty_sum.u128(),
        );
        reward_info.penalty = Uint128::zero();
    }
    reward_info.n = n;
    Ok(())
}

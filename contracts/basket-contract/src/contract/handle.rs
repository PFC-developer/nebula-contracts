use cosmwasm_std::{
    from_binary, log, Api, Env, Extern, HandleResponse, HandleResult, HumanAddr, Querier, StdError,
    StdResult, Storage, Uint128,
};

use cw20::Cw20ReceiveMsg;

use crate::msg::{Cw20HookMsg, HandleMsg};
use crate::penalty::{compute_penalty, compute_score};
use crate::state::{read_config, read_target, PenaltyParams};
use crate::util::to_fpdec_vec;
use crate::{
    ext_query::{query_cw20_balance, query_price},
    test_helper::query_cw20_token_supply,
};
use basket_math::{dot, sum, FPDecimal};

pub fn handle<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    msg: HandleMsg,
) -> StdResult<HandleResponse> {
    match msg {
        HandleMsg::Receive(msg) => receive_cw20(deps, env, msg),
        HandleMsg::Mint {
            asset_amounts,
            min_tokens,
        } => try_mint(deps, env, asset_amounts, min_tokens),
        HandleMsg::UnstageAsset { amount, asset } => try_unstage_asset(deps, env, asset, amount),
        HandleMsg::ResetTarget { target } => try_reset_target(deps, env, target),
    }
}

pub fn receive_cw20<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    cw20_msg: Cw20ReceiveMsg,
) -> HandleResult {
    let sender = cw20_msg.sender;
    let sent_amount = cw20_msg.amount;
    let sent_asset = env.message.sender.clone();

    if let Some(msg) = cw20_msg.msg {
        match from_binary(&msg)? {
            Cw20HookMsg::Burn {
                num_tokens,
                asset_weights,
            } => try_receive_burn(
                deps,
                env,
                &sender,
                &sent_asset,
                sent_amount,
                num_tokens,
                asset_weights,
            ),
            Cw20HookMsg::StageAsset { asset, amount } => {
                try_receive_stage_asset(deps, env, asset, amount)
            }
        }
    } else {
        Err(StdError::generic_err(
            "Receive Hook - missing expected .msg in body",
        ))
    }
}

pub fn try_receive_burn<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    sender: &HumanAddr,
    sent_asset: &HumanAddr,
    sent_amount: Uint128,
    num_tokens: Uint128,
    asset_weights: Option<Vec<u32>>,
) -> StdResult<HandleResponse> {
    let cfg = read_config(&deps.storage)?;

    // require that origin contract from Receive Hook is the associated Basket Token
    if *sent_asset != cfg.basket_token {
        return Err(StdError::unauthorized());
    }

    // check if (num_tokens) to be burnt <= sent_amount
    if num_tokens > sent_amount {
        return Err(StdError::generic_err(format!(
            "Num of tokens to burn ({}) exceeds amount sent ({})",
            num_tokens, sent_amount
        )));
    }

    let inv: Vec<Uint128> = cfg
        .assets
        .iter()
        .map(|asset| query_cw20_balance(&deps, &asset, &env.contract.address).unwrap())
        .collect();

    let inv = to_fpdec_vec(&inv);
    let basket_token_supply = query_cw20_token_supply(&deps, &cfg.basket_token)?;

    let m_div_n =
        FPDecimal::from(num_tokens.0 as i128) / FPDecimal::from(basket_token_supply.0 as i128);

    let redeem_subtotals: Vec<FPDecimal> = match &asset_weights {
        Some(weights) => {
            let weights_sum = weights
                .iter()
                .fold(FPDecimal::zero(), |acc, &el| acc + FPDecimal::from(el));
            let r = weights // normalize weights vector
                .iter()
                .map(|&x| FPDecimal::from(x) / weights_sum)
                .collect();
            let prices: Vec<FPDecimal> = cfg
                .assets
                .iter()
                .map(|asset| query_price(&deps, &cfg.oracle, &asset).unwrap())
                .collect();
            let prod = dot(&inv, &prices) / dot(&r, &prices);
            let b: Vec<FPDecimal> = r
                .iter()
                .map(|&x| FPDecimal::one().mul(-1) * m_div_n * prod * x)
                .collect();

            // compute penalty
            let score = compute_score(&inv, &b, &prices, &read_target(&deps.storage)?);
            let PenaltyParams {
                a_pos,
                s_pos,
                a_neg,
                s_neg,
            } = cfg.penalty_params;
            let penalty = compute_penalty(score, a_pos, s_pos, a_neg, s_neg);
            b.iter().map(|&x| penalty * x).collect()
        }
        None => inv.iter().map(|&x| m_div_n * x).collect(),
    };

    // convert reward into Uint128 -- truncate decimal as roundoff
    let redeem_totals: Vec<Uint128> = redeem_subtotals
        .iter()
        .map(|&x| Uint128((x.int().0 / FPDecimal::ONE) as u128))
        .collect();
    let redeem_roundoffs: Vec<FPDecimal> = redeem_subtotals.iter().map(|&x| x.fraction()).collect();

    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("action", "receive:burn"),
            log("sender", sender),
            log("sent_asset", sent_asset),
            log("sent_tokens", sent_amount),
            log("num_tokens", num_tokens),
            log(
                "asset_weights",
                match &asset_weights {
                    Some(v) => format!(
                        "[{}]",
                        v.iter()
                            .map(|&x| x.to_string())
                            .collect::<Vec<String>>()
                            .join(", ")
                    ),
                    None => "".to_string(),
                },
            ),
        ],
        data: None,
    })
}

pub fn try_receive_stage_asset<S: Storage, A: Api, Q: Querier>(
    deps: &mut Extern<S, A, Q>,
    env: Env,
    asset: HumanAddr,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

/// May be called by the Basket contract owner to reset the target
pub fn try_reset_target<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    env: Env,
    target: Vec<u32>,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

pub fn try_mint<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    env: Env,
    asset_amounts: Vec<Uint128>,
    min_tokens: Option<Uint128>,
) -> StdResult<HandleResponse> {
    let cfg = read_config(&deps.storage)?;
    let target = read_target(&deps.storage)?;

    // get current balances of each token (inventory)
    let inv: Vec<Uint128> = cfg
        .assets
        .iter()
        .map(|asset| query_cw20_balance(&deps, &asset, &env.contract.address).unwrap())
        .collect();
    let inv = to_fpdec_vec(&inv);
    let c = to_fpdec_vec(&asset_amounts);

    // get current prices of each token via oracle
    let prices: Vec<FPDecimal> = cfg
        .assets
        .iter()
        .map(|asset| query_price(&deps, &cfg.oracle, &asset).unwrap())
        .collect();

    // compute penalty
    let score = compute_score(&inv, &c, &prices, &target);
    let PenaltyParams {
        a_pos,
        s_pos,
        a_neg,
        s_neg,
    } = cfg.penalty_params;
    let penalty = compute_penalty(score, a_pos, s_pos, a_neg, s_neg);
    let basket_token_supply = query_cw20_token_supply(&deps, &cfg.basket_token)?;

    // compute number of new tokens
    let mint_subtotal = penalty * dot(&c, &prices) / dot(&inv, &prices)
        * FPDecimal::from(basket_token_supply.u128() as i128);

    let mint_roundoff = mint_subtotal.fraction(); // fraction part is truncated
    let mint_total = Uint128((mint_subtotal.int().0 / FPDecimal::ONE) as u128);

    if let Some(m) = min_tokens {
        if mint_total < m {
            return Err(StdError::generic_err(format!(
                "transaction aborted: transaction would mint {}, which is less than min_tokens specified: {}",
                mint_total.0, m
            )));
        }
    }

    // mint and send number of tokens to user
    Ok(HandleResponse {
        messages: vec![],
        log: vec![
            log("score", score),
            log("penalty", penalty),
            log("mint_total", mint_total),
            log("mint_roundoff", mint_roundoff),
        ],
        data: None,
    })
}

pub fn try_unstage_asset<S: Storage, A: Api, Q: Querier>(
    deps: &Extern<S, A, Q>,
    env: Env,
    asset: HumanAddr,
    amount: Uint128,
) -> StdResult<HandleResponse> {
    Ok(HandleResponse::default())
}

#[cfg(test)]
mod tests {

    use crate::test_helper::*;
    #[test]
    fn mint() {
        let (mut deps, _init_res) = mock_init();

        // Asset :: UST Price :: Balance (µ)     (+ proposed   ) :: %
        // ---
        // mAAPL ::  135.18   ::  7_290_053_159  (+ 125_000_000) :: 0.20367359382 -> 0.20391741720
        // mGOOG :: 1780.03   ::    319_710_128                  :: 0.11761841035 -> 0.11577407690
        // mMSFT ::  222.42   :: 14_219_281_228  (+ 149_000_000) :: 0.65364669475 -> 0.65013907200
        // mNFLX ::  540.82   ::    224_212_221  (+  50_090_272) :: 0.02506130106 -> 0.03016943389

        deps.querier.reset_oracle_querier();
        deps.querier.set_oracle_prices(vec![
            ("uusd", Decimal::one()),
            ("mAAPL", Decimal::from_str("135.18").unwrap()),
            ("mGOOG", Decimal::from_str("1780.03").unwrap()),
            ("mMSFT", Decimal::from_str("222.42").unwrap()),
            ("mNFLX", Decimal::from_str("540.82").unwrap()),
        ]);

        deps.querier.reset_token_querier();
        deps.querier.set_token(
            "basket",
            token_data::<Vec<(&str, u128)>, &str>(
                "Basket Protocol - 1",
                "BASKET",
                6,
                1_000_000_000,
                vec![],
            ),
        );
        deps.querier.set_token(
            "mAAPL",
            token_data(
                "Mirrored Apple",
                "mAAPL",
                6,
                1_000_000_000,
                vec![(MOCK_CONTRACT_ADDR, 7_290_053_159)],
            ),
        );
        deps.querier.set_token(
            "mGOOG",
            token_data(
                "Mirrored Google",
                "mGOOG",
                6,
                1_000_000_000,
                vec![(MOCK_CONTRACT_ADDR, 14_219_281_228)],
            ),
        );
        deps.querier.set_token(
            "mMSFT",
            token_data(
                "Mirrored Microsoft",
                "mMSFT",
                6,
                1_000_000_000,
                vec![(MOCK_CONTRACT_ADDR, 224_212_221)],
            ),
        );
        deps.querier.set_token(
            "mNFLX",
            token_data(
                "Mirrored Netflix",
                "mNFLX",
                6,
                1_000_000_000,
                vec![(MOCK_CONTRACT_ADDR, 224_212_221)],
            ),
        );

        let msg = HandleMsg::Mint {
            asset_amounts: vec![
                Uint128(125_000_000), // mAAPL
                Uint128::zero(),      // mGOOG
                Uint128(149_000_000), // mMSFT
                Uint128(50_090_272),  // mNFLX
            ],
            min_tokens: None,
        };

        let env = mock_env(consts::owner(), &[]);
        let res = handle(&mut deps, env, msg).unwrap();
        for log in res.log.iter() {
            println!("{}: {}", log.key, log.value);
        }
        assert_eq!(1, res.messages.len());
    }
}

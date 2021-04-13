use basket_math::FPDecimal;
use cosmwasm_std::{HumanAddr, Uint128};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{BasketConfig, PenaltyParams};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InitMsg {
    /// Basket name (title)
    pub name: String,

    /// Basket's permissioned owner
    pub owner: HumanAddr,

    /// Basket token CW20 address
    pub basket_token: Option<HumanAddr>,

    /// Asset addresses
    pub assets: Vec<HumanAddr>,

    /// Oracle address
    pub oracle: HumanAddr,

    /// Penalty function params
    pub penalty_params: PenaltyParams,

    /// Target weight vector (not normalized)
    pub target: Vec<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandleMsg {
    Receive(Cw20ReceiveMsg),

    /// Withdraws asset from staging
    UnstageAsset {
        asset: HumanAddr,
        amount: Option<Uint128>,
    },

    /// Called to set basket token after initialization
    _SetBasketToken {
        basket_token: HumanAddr,
    },

    /// Can be called by the owner to reset the basket weight target
    ResetTarget {
        assets: Vec<HumanAddr>,
        target: Vec<u32>,
    },

    /// Mints new assets
    Mint {
        /// Asset amounts deposited for minting (must be staged)
        asset_amounts: Vec<Uint128>,
        /// Minimum tokens to receive
        min_tokens: Option<Uint128>,
    },
    // AddAssetType {
    //     asset: HumanAddr,
    // },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cw20HookMsg {
    /// After received, registers the received amount and prepares it to be used for minting
    StageAsset {},

    /// Burns assets
    Burn {
        /// optional proposed set of weights to use
        asset_weights: Option<Vec<u32>>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Target {},
    StagedAmount {
        account: HumanAddr,
        asset: HumanAddr,
    },
    BasketState {
        basket_contract_address: HumanAddr,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    pub config: BasketConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct TargetResponse {
    pub target: Vec<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StagedAmountResponse {
    pub staged_amount: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BasketStateResponse {
    pub penalty_params: PenaltyParams,
    pub outstanding_balance_tokens: Uint128,
    pub prices: Vec<FPDecimal>,
    pub inv: Vec<Uint128>,
    pub assets: Vec<HumanAddr>,
    pub target: Vec<u32>,
}

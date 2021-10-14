use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Api, CanonicalAddr, Coin, ContractResult, Decimal, Empty,
    OwnedDeps, Querier, QuerierResult, QueryRequest, SystemError, SystemResult, WasmQuery,
};
use cosmwasm_storage::to_length_prefixed;

use crate::querier::MintAssetConfig;
use std::collections::HashMap;
use terraswap::asset::{AssetInfo, PairInfo};

/// mock_dependencies is a drop-in replacement for cosmwasm_std::testing::mock_dependencies
/// this uses our CustomQuerier.
pub fn mock_dependencies(
    contract_balance: &[Coin],
) -> OwnedDeps<MockStorage, MockApi, WasmMockQuerier> {
    let contract_addr = MOCK_CONTRACT_ADDR.to_string();
    let custom_querier: WasmMockQuerier =
        WasmMockQuerier::new(MockQuerier::new(&[(&contract_addr, contract_balance)]));

    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: custom_querier,
    }
}

pub struct WasmMockQuerier {
    base: MockQuerier<Empty>,
    terraswap_factory_querier: TerraswapFactoryQuerier,
    oracle_querier: OracleQuerier,
    mint_querier: MintQuerier,
}

#[derive(Clone, Default)]
pub struct TerraswapFactoryQuerier {
    pairs: HashMap<String, String>,
}

impl TerraswapFactoryQuerier {
    pub fn new(pairs: &[(&String, &String)]) -> Self {
        TerraswapFactoryQuerier {
            pairs: pairs_to_map(pairs),
        }
    }
}

pub(crate) fn pairs_to_map(pairs: &[(&String, &String)]) -> HashMap<String, String> {
    let mut pairs_map: HashMap<String, String> = HashMap::new();
    for (key, pair) in pairs.iter() {
        pairs_map.insert(key.to_string(), pair.to_string());
    }
    pairs_map
}

#[derive(Clone, Default)]
pub struct OracleQuerier {
    feeders: HashMap<String, String>,
}

#[derive(Clone, Default)]
pub struct MintQuerier {
    configs: HashMap<String, (Decimal, Decimal, Option<Decimal>)>,
}

impl Querier for WasmMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        // MockQuerier doesn't support Custom, so we ignore it completely here
        let request: QueryRequest<Empty> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return SystemResult::Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
            }
        };
        self.execute_query(&request)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Pair { asset_infos: [AssetInfo; 2] },
}

impl WasmMockQuerier {
    pub fn execute_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match &request {
            QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr: _,
                msg,
            }) => match from_binary(&msg).unwrap() {
                QueryMsg::Pair { asset_infos } => {
                    let key = asset_infos[0].to_string() + asset_infos[1].to_string().as_str();
                    match self.terraswap_factory_querier.pairs.get(&key) {
                        Some(v) => SystemResult::Ok(ContractResult::from(to_binary(&PairInfo {
                            contract_addr: "pair".to_string(),
                            liquidity_token: v.clone(),
                            asset_infos: [
                                AssetInfo::NativeToken {
                                    denom: "uusd".to_string(),
                                },
                                AssetInfo::NativeToken {
                                    denom: "uusd".to_string(),
                                },
                            ],
                        }))),
                        None => SystemResult::Err(SystemError::InvalidRequest {
                            error: "No pair info exists".to_string(),
                            request: msg.as_slice().into(),
                        }),
                    }
                }
            },
            QueryRequest::Wasm(WasmQuery::Raw { contract_addr, key }) => {
                let key: &[u8] = key.as_slice();
                let prefix_asset_config = to_length_prefixed(b"asset_config").to_vec();
                let prefix_feeder = to_length_prefixed(b"feeder").to_vec();

                let api: MockApi = MockApi::default();
                if key.len() > prefix_feeder.len()
                    && key[..prefix_feeder.len()].to_vec() == prefix_feeder
                {
                    let api: MockApi = MockApi::default();
                    let rest_key: &[u8] = &key[prefix_feeder.len()..];

                    if contract_addr == &("oracle0000") {
                        let asset_token: String = api
                            .addr_humanize(&(CanonicalAddr::from(rest_key.to_vec())))
                            .unwrap()
                            .to_string();

                        let feeder = match self.oracle_querier.feeders.get(&asset_token) {
                            Some(v) => v,
                            None => {
                                return SystemResult::Err(SystemError::InvalidRequest {
                                    error: format!(
                                        "Oracle Feeder is not found for {}",
                                        asset_token
                                    ),
                                    request: key.into(),
                                })
                            }
                        };

                        SystemResult::Ok(ContractResult::from(to_binary(
                            &api.addr_canonicalize(&feeder).unwrap(),
                        )))
                    } else {
                        panic!("DO NOT ENTER HERE")
                    }
                } else if key.len() > prefix_asset_config.len()
                    && key[..prefix_asset_config.len()].to_vec() == prefix_asset_config
                {
                    let rest_key: &[u8] = &key[prefix_asset_config.len()..];
                    let asset_token: String = api
                        .addr_humanize(&(CanonicalAddr::from(rest_key.to_vec())))
                        .unwrap()
                        .to_string();

                    let config = match self.mint_querier.configs.get(&asset_token) {
                        Some(v) => v,
                        None => {
                            return SystemResult::Err(SystemError::InvalidRequest {
                                error: format!("Mint Config is not found for {}", asset_token),
                                request: key.into(),
                            })
                        }
                    };

                    SystemResult::Ok(ContractResult::from(to_binary(
                        &to_binary(&MintAssetConfig {
                            token: api.addr_canonicalize(&asset_token).unwrap(),
                            auction_discount: config.0,
                            min_collateral_ratio: config.1,
                            min_collateral_ratio_after_migration: config.2,
                        })
                        .unwrap()
                        .to_string(),
                    )))
                } else {
                    panic!("DO NOT ENTER HERE")
                }
            }
            _ => self.base.handle_query(request),
        }
    }
}

impl WasmMockQuerier {
    pub fn new(base: MockQuerier<Empty>) -> Self {
        WasmMockQuerier {
            base,
            terraswap_factory_querier: TerraswapFactoryQuerier::default(),
            mint_querier: MintQuerier::default(),
            oracle_querier: OracleQuerier::default(),
        }
    }

    // configure the terraswap pair
    pub fn with_terraswap_pairs(&mut self, pairs: &[(&String, &String)]) {
        self.terraswap_factory_querier = TerraswapFactoryQuerier::new(pairs);
    }
}

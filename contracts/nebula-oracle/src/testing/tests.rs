use crate::contract::{execute, instantiate, query};
use crate::state::{read_config, Config};
use crate::testing::mock_querier::mock_dependencies;
use astroport::asset::AssetInfo;
use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{from_binary, Addr, Decimal, StdError};
use nebula_protocol::oracle::{ExecuteMsg, InstantiateMsg, PriceResponse, QueryMsg};
use std::str::FromStr;

fn init_msg() -> InstantiateMsg {
    InstantiateMsg {
        owner: "owner0000".to_string(),
        oracle_addr: "oracle0000".to_string(),
        base_denom: "uusd".to_string(),
    }
}

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("sender0000", &[]);
    let msg = init_msg();
    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    let config: Config = read_config(&deps.storage).unwrap();
    assert_eq!(
        config,
        Config {
            owner: Addr::unchecked("owner0000"),
            oracle_addr: Addr::unchecked("oracle0000"),
            base_denom: "uusd".to_string(),
        }
    );
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("sender0000", &[]);
    let msg = init_msg();
    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // unauthorized update
    let info = mock_info("imposter0000", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("imposter0000".to_string()),
        oracle_addr: Some("oracle0001".to_string()),
        base_denom: None,
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    assert_eq!(res, StdError::generic_err("unauthorized"));

    // successful update
    let info = mock_info("owner0000", &[]);
    let msg = ExecuteMsg::UpdateConfig {
        owner: Some("owner0001".to_string()),
        oracle_addr: Some("oracle0001".to_string()),
        base_denom: Some("uusd".to_string()),
    };
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    let config = read_config(&deps.storage).unwrap();
    assert_eq!(
        config,
        Config {
            owner: Addr::unchecked("owner0001"),
            oracle_addr: Addr::unchecked("oracle0001"),
            base_denom: "uusd".to_string()
        }
    )
}

#[test]
fn query_price() {
    let mut deps = mock_dependencies(&[]);
    let info = mock_info("sender0000", &[]);
    let msg = init_msg();
    let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(0, res.messages.len());

    deps.querier.set_tefi_oracle_prices(vec![
        ("token0001", Decimal::from_str("765.52").unwrap()),
        ("token0002", Decimal::from_str("1.9234").unwrap()),
    ]);

    deps.querier.set_terra_oracle_prices(vec![
        ("uluna", Decimal::from_str("66.435110305004678719").unwrap()),
        ("uusd", Decimal::from_str("1.00").unwrap()),
    ]);

    // no cw20 oracle price exists
    let msg = QueryMsg::Price {
        base_asset: AssetInfo::Token {
            contract_addr: Addr::unchecked("nebulatoken"),
        },
        quote_asset: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
    };
    let res = query(deps.as_ref(), mock_env(), msg).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err("Querier system error: Cannot parse request: No oracle price exists in: {\"price\":{\"asset_token\":\"nebulatoken\",\"timeframe\":null}}")
    );

    // no native oracle price exists
    let msg = QueryMsg::Price {
        base_asset: AssetInfo::NativeToken {
            denom: "ukrw".to_string(),
        },
        quote_asset: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
    };
    let res = query(deps.as_ref(), mock_env(), msg).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err(
            "Querier system error: Cannot parse request: No native denom exists in: "
        )
    );

    // successful queries
    let msg = QueryMsg::Price {
        base_asset: AssetInfo::Token {
            contract_addr: Addr::unchecked("token0001"),
        },
        quote_asset: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
    };
    let res = query(deps.as_ref(), mock_env(), msg).unwrap();
    let price: PriceResponse = from_binary(&res).unwrap();
    assert_eq!(price.rate, Decimal::from_str("765.52").unwrap());

    let msg = QueryMsg::Price {
        base_asset: AssetInfo::NativeToken {
            denom: "uusd".to_string(),
        },
        quote_asset: AssetInfo::NativeToken {
            denom: "uluna".to_string(),
        },
    };
    let res = query(deps.as_ref(), mock_env(), msg).unwrap();
    let price: PriceResponse = from_binary(&res).unwrap();
    assert_eq!(
        price.rate,
        Decimal::from_str("0.015052281774035657").unwrap()
    );
}

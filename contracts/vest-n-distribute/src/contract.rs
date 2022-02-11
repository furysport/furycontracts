use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128,
};

use cw2::set_contract_version;

use cw20::{Cw20ExecuteMsg};

use crate::error::ContractError;
use crate::msg::{InstantiateMsg, InstantiateVestingSchedulesInfo, MigrateMsg, QueryMsg};

use crate::state::{VestingDetails, VESTING_DETAILS};

const CONTRACT_NAME: &str = "crates.io:cw20-base";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    instantiate_category_vesting_schedules(deps, env, msg.vesting)?;

    Ok(Response::default())
}

fn instantiate_category_vesting_schedules(
    deps: DepsMut,
    env: Env,
    vesting: Option<InstantiateVestingSchedulesInfo>,
) -> Result<Response, ContractError> {
    match vesting {
        Some(vesting_info) => {
            for schedule in vesting_info.vesting_schedules {
                let mut parent_cat_addr = None;
                if !schedule.parent_category_address.is_empty() {
                    parent_cat_addr = Some(schedule.parent_category_address);
                }
                let vesting_start_timestamp = env.block.time;
                let address = deps.api.addr_validate(schedule.address.as_str())?;
                let vesting_details = VestingDetails {
                    vesting_start_timestamp: vesting_start_timestamp,
                    initial_vesting_count: schedule.initial_vesting_count,
                    initial_vesting_consumed: Uint128::zero(),
                    vesting_periodicity: schedule.vesting_periodicity,
                    vesting_count_per_period: schedule.vesting_count_per_period,
                    total_vesting_token_count: schedule.total_vesting_token_count,
                    total_claimed_tokens_till_now: Uint128::zero(),
                    last_claimed_timestamp: None,
                    tokens_available_to_claim: Uint128::zero(),
                    last_vesting_timestamp: None,
                    cliff_period: schedule.cliff_period,
                    parent_category_address: parent_cat_addr,
                    should_transfer: schedule.should_transfer,
                };
                VESTING_DETAILS.save(deps.storage, &address, &vesting_details)?;
            }
            Ok(Response::default())
        }
        None => Ok(Response::default()),
    }
}

// pub fn create_accounts(deps: &mut DepsMut, accounts: &[Cw20Coin]) -> StdResult<Uint128> {
//     let mut total_supply = Uint128::zero();
//     for row in accounts {
//         let address = deps.api.addr_validate(&row.address)?;
//         BALANCES.save(deps.storage, &address, &row.amount)?;
//         total_supply += row.amount;
//     }
//     Ok(total_supply)
// }

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    _deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: Cw20ExecuteMsg,
) -> Result<Response, ContractError> {
    Err(ContractError::Std(StdError::generic_err(String::from(
        "Not yet implemented",
    ))))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    to_binary(&some_query()?)
}

fn some_query() -> StdResult<String> {
    Err(StdError::not_found("Not yet implemented"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

#[cfg(test)]
mod tests {
    // use super::*;
    // use crate::msg::InstantiateMsg;
    // use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    #[test]
    fn vesting_test_cases() {
        assert_eq!(1, 1);
    }
}

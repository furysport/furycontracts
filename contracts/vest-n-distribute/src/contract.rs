use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
    Uint128,
};

use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{
    ExecuteMsg, InstantiateMsg, InstantiateVestingSchedulesInfo, MigrateMsg, QueryMsg,
};

use crate::state::{Config, VestingDetails, CONFIG, VESTING_DETAILS};

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
    //Save the main_wallet address into config
    let config: Config = Config{
        main_wallet : msg.main_wallet,
    };
    CONFIG.save(deps.storage, &config)?;
    instantiate_category_vesting_schedules(deps, env, msg.vesting)?;

    Ok(Response::default())
}

fn instantiate_category_vesting_schedules(
    deps: DepsMut,
    env: Env,
    vesting: InstantiateVestingSchedulesInfo,
) -> Result<Response, ContractError> {
    //Check if vesting is not provided and throw error here
    //Proceed now
    for schedule in vesting.vesting_schedules {
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
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::PeriodicallyTransferToCategories {} => {
            periodically_transfer_to_categories(deps, env, info)
        }
        ExecuteMsg::PeriodicallyCalculateVesting {} => {
            periodically_calculate_vesting(deps, env, info)
        }
        ExecuteMsg::ClaimVestedTokens { amount } => claim_vested_tokens(deps, env, info, amount),
    }
}

fn periodically_transfer_to_categories(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    //capture the current system time
    let now = env.block.time;
    let config = CONFIG.load(deps.storage)?;

    let address = config.main_wallet.as_ref().clone();

    // Fetch all tokens that can be distributed as per vesting logic
    let distribution_details = populate_transfer_details(&deps, now)?;

    // Calculate the total amount to be vested
    let total_transfer_amount = calculate_total_distribution(&distribution_details);
    //Get the balance available in main wallet
    let balance = BALANCES
        .may_load(deps.storage, &address)?
        .unwrap_or_default();

    //Check if there is sufficient balance with main wallet
    // return error otherwise
    if balance < total_transfer_amount {
        return Err(ContractError::Std(StdError::overflow(OverflowError::new(
            OverflowOperation::Sub,
            balance,
            total_transfer_amount,
        ))));
    }
    let distribute_from = address.into_string();
    let mut attribs: Vec<Attribute> = Vec::new();
    for elem in distribution_details {
        // Transfer the funds
        let res = distribute_vested(
            &mut deps,
            distribute_from.clone(),
            elem.spender_address.clone(),
            elem.amount,
        )?;
        for attrib in res.attributes {
            attribs.push(attrib);
        }
        // Save distribution information
        let res = update_vesting_details(
            &mut deps,
            elem.spender_address.clone(),
            env.block.time,
            Some(elem),
            None,
        )?;
        for attrib in res.attributes {
            attribs.push(attrib);
        }
        attribs.push(Attribute::new("kuchha hua", "Pata nahi"));
    }
    Ok(Response::new().add_attributes(attribs))
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

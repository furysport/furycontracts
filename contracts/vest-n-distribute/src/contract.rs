use cosmwasm_std::{Addr, Binary, CosmosMsg, Deps, DepsMut, entry_point, Env, MessageInfo, Reply, ReplyOn, Response, StdResult, SubMsg, to_binary, Uint128, WasmMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw2::set_contract_version;

use crate::error::ContractError;
use crate::execute::{add_vesting_schedules, claim_vested_tokens, periodically_calculate_vesting, periodically_transfer_to_categories};
use crate::msg::{CW20Custom, ExecuteMsg, InstantiateMsg, InstantiateVestingSchedulesInfo, MigrateMsg, QueryMsg};
use crate::query::query_vesting_details;
use crate::state::{Config, CONFIG, VESTING_DETAILS, VestingDetails};

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
    let config: Config = Config {
        admin_wallet: msg.admin_wallet,
        fury_token_address: msg.fury_token_contract,
    };
    CONFIG.save(deps.storage, &config)?;
    instantiate_category_vesting_schedules(deps, env, msg.vesting, None)?;
    Ok(Response::default())
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct MinterData {
    pub minter: Addr,
    /// cap is how many more tokens can be issued by the minter
    pub cap: Option<Uint128>,
}

#[derive(Clone, Default, Debug)]
pub struct VestingInfo {
    pub spender_address: String,
    pub parent_category_address: Option<String>,
    pub amount: Uint128,
}

pub fn instantiate_category_vesting_schedules(
    deps: DepsMut,
    env: Env,
    vesting_info: InstantiateVestingSchedulesInfo,
    add: Option<bool>,
) -> Result<Response, ContractError> {
    let mut check_duplicate_ = false;
    match add {
        None => {}
        Some(value) => {
            check_duplicate_ = value;
        }
    }
    for schedule in vesting_info.vesting_schedules {
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
            parent_category_address: schedule.parent_category_address,
            should_transfer: schedule.should_transfer,
        };

        match VESTING_DETAILS.load(deps.storage, &address) {
            Ok(some) => {
                if check_duplicate_ {

                    // set custom error saying accounts exists already in the schedule
                    return Err(ContractError::ErrorDupliacateEntry {});
                }
                VESTING_DETAILS.save(deps.storage, &address, &vesting_details)?;
            }
            Err(..) => {
                VESTING_DETAILS.save(deps.storage, &address, &vesting_details)?;
            }
        }

        VESTING_DETAILS.save(deps.storage, &address, &vesting_details)?;
    }
    Ok(Response::default())
}


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
        ExecuteMsg::AddVestingSchedules { schedules } => {
            add_vesting_schedules(deps, env, schedules)
        },
        ExecuteMsg::HelloSub {} => hello_sub(deps)
    }
}

fn hello_sub(
    deps: DepsMut,
) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;
    // TESTING AS EXECUTE
    let pl_msg = CW20Custom::HelloSub {};
    let exec = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: config.fury_token_address.to_string(),
        msg: to_binary(&pl_msg).unwrap(),
        funds: Vec::new(),
    });
    //TESTING AS SUB
    let mut send: SubMsg = SubMsg::new(exec.clone());
    let mut sub_req_id = 1;
    send.id = sub_req_id;
    send.reply_on = ReplyOn::Always;
    let mut resp = Response::new();
    resp = resp.add_submessage(send);
    resp = resp.add_message(exec);

    return Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::VestingDetails { address } => to_binary(&query_vesting_details(deps, address)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    return Ok(Response::new())
}
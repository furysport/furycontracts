use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{
    attr, entry_point, to_binary, Addr, Attribute, Binary, Deps, DepsMut, Env, MessageInfo,
    OverflowError, OverflowOperation, Response, StdError, StdResult, SubMsg, Timestamp, Uint128,
    WasmMsg,
};

use cw2::set_contract_version;
use cw20::{
    AllowanceResponse, BalanceResponse, Cw20Coin, Cw20ExecuteMsg, Cw20QueryMsg, Cw20ReceiveMsg,
    DownloadLogoResponse, EmbeddedLogo, Expiration, Logo, LogoInfo, MarketingInfoResponse,
    MinterResponse, TokenInfoResponse,
};

use crate::error::ContractError;
use crate::msg::{
    ExecuteMsg, InstantiateMsg, InstantiateVestingSchedulesInfo, MigrateMsg, QueryMsg,
};

use crate::state::{Config, VestingDetails, CONFIG, VESTING_DETAILS};

const CONTRACT_NAME: &str = "crates.io:cw20-base";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Cliff period unit (seconds in a week)
const CLIFF_PERIOD_UNIT: u64 = 7 * 24 * 60 * 60;
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
    instantiate_category_vesting_schedules(deps, env, msg.vesting)?;
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

fn instantiate_category_vesting_schedules(
    deps: DepsMut,
    env: Env,
    vesting_info: InstantiateVestingSchedulesInfo,
) -> Result<Response, ContractError> {
    // Some(vesting_info) => {
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
        VESTING_DETAILS.save(deps.storage, &address, &vesting_details)?;
    }
    Ok(Response::default())
}

fn periodically_calculate_vesting(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let now = env.block.time;
    let config = CONFIG.load(deps.storage)?;
    //Check if the sender (one who is executing this contract) is admin
    if config.admin_wallet != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // let address = env.contract.address;

    // Fetch all tokens that can be vested as per vesting logic
    let vested_details = populate_vesting_details(&deps, now)?;
    // Calculate the total amount to be vested
    let total_vested_amount = calculate_total_distribution(&vested_details);
    //Get the balance available in admin wallet
    let balance_msg = Cw20QueryMsg::Balance {
        address: info.sender.clone().into_string(),
    };
    let balance_response: cw20::BalanceResponse = deps
        .querier
        .query_wasm_smart(config.fury_token_address.clone(), &balance_msg)?;

    let balance = balance_response.balance;
    if balance < total_vested_amount {
        return Err(ContractError::Std(StdError::overflow(OverflowError::new(
            OverflowOperation::Sub,
            balance,
            total_vested_amount,
        ))));
    }
    let mut sub_msgs: Vec<SubMsg> = Vec::new();
    let mut attribs: Vec<Attribute> = Vec::new();
    for elem in vested_details {
        if elem.amount.u128() > 0 {
            let spender_addr = deps.api.addr_validate(&elem.spender_address)?;
            // let category_address = elem.clone().parent_category_address.unwrap_or_default();
            // let owner_addr = deps.api.addr_validate(&category_address)?;
            //Move the tokens from admin wallet to vesting contract
            let transfer_from_msg = Cw20ExecuteMsg::TransferFrom {
                owner: info.sender.clone().into_string(),
                recipient: env.contract.address.clone().into_string(),
                amount: elem.amount,
            };
            let exec_transfer_from_msg = WasmMsg::Execute {
                contract_addr: config.fury_token_address.to_string(),
                msg: to_binary(&transfer_from_msg).unwrap(),
                funds: vec![],
            };
            let send_transfer_from_msg: SubMsg = SubMsg::new(exec_transfer_from_msg);
            sub_msgs.push(send_transfer_from_msg);
            attribs.push(Attribute::new("action", "transfer_from"));
            attribs.push(Attribute::new("from", info.sender.clone().into_string()));
            attribs.push(Attribute::new("for", spender_addr.into_string()));
            attribs.push(Attribute::new("amount", elem.amount));
        }
        //Save the vesting details
        let res = update_vesting_details(
            &mut deps,
            elem.clone().spender_address,
            env.block.time,
            None,
            Some(elem),
        )?;
        for attrib in res.attributes {
            attribs.push(attrib);
        }
    }
    Ok(Response::new()
        .add_submessages(sub_msgs)
        .add_attributes(attribs))
}

fn calculate_total_distribution(distribution_details: &Vec<VestingInfo>) -> Uint128 {
    let mut total = Uint128::zero();
    for elem in distribution_details {
        total += elem.amount;
    }
    return total;
}

fn update_vesting_details(
    deps: &mut DepsMut,
    address: String,
    execution_timestamp: Timestamp,
    transferred: Option<VestingInfo>,
    vestable: Option<VestingInfo>,
) -> Result<Response, ContractError> {
    let addr = deps.api.addr_validate(&address)?;

    //replace the optional to required
    match transferred {
        Some(transferred) => {
            VESTING_DETAILS.update(deps.storage, &addr, |vd| -> StdResult<_> {
                //replace the optional to required
                match vd {
                    Some(mut v) => {
                        let new_count = v.total_claimed_tokens_till_now + transferred.amount;
                        if new_count <= v.total_vesting_token_count {
                            v.total_claimed_tokens_till_now = new_count;
                            v.last_vesting_timestamp = Some(execution_timestamp);
                            v.last_claimed_timestamp = Some(execution_timestamp);
                        }
                        v.initial_vesting_consumed = v.initial_vesting_count;
                        Ok(v)
                    }
                    None => Err(StdError::GenericErr {
                        msg: String::from("Vesting Details not found"),
                    }),
                }
            })?;
        }
        None => (),
    }
    match vestable {
        Some(vestable) => {
            VESTING_DETAILS.update(deps.storage, &addr, |vd| -> StdResult<_> {
                match vd {
                    Some(mut v) => {
                        let new_count = v.tokens_available_to_claim + vestable.amount;
                        let mut new_vestable_tokens = new_count;
                        if v.total_claimed_tokens_till_now + new_count > v.total_vesting_token_count
                        {
                            new_vestable_tokens =
                                v.total_vesting_token_count - v.total_claimed_tokens_till_now;
                        }
                        v.tokens_available_to_claim = new_vestable_tokens;
                        if v.last_vesting_timestamp.is_none() {
                            // v.tokens_available_to_claim += v.initial_vesting_count;
                            v.initial_vesting_consumed = v.initial_vesting_count;
                        }
                        v.last_vesting_timestamp = Some(execution_timestamp);
                        Ok(v)
                    }
                    None => Err(StdError::GenericErr {
                        msg: String::from("Vesting Details not found"),
                    }),
                }
            })?;
        }
        None => (),
    }
    Ok(Response::default())
}

fn populate_vesting_details(
    deps: &DepsMut,
    now: Timestamp,
) -> Result<Vec<VestingInfo>, ContractError> {
    let vester_addresses: Vec<String> = VESTING_DETAILS
        .keys(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();

    let mut distribution_details: Vec<VestingInfo> = Vec::new();

    for addr in vester_addresses {
        let wallet_address = deps.api.addr_validate(&addr)?;
        let vested_detais = VESTING_DETAILS.may_load(deps.storage, &wallet_address);
        match vested_detais {
            Ok(vested_detais) => {
                let vd = vested_detais.unwrap();
                if !vd.should_transfer {
                    let vesting_info = calculate_tokens_for_this_period(wallet_address, now, vd)?;
                    if vesting_info.amount.u128() > 0 {
                        distribution_details.push(vesting_info);
                    }
                }
            }
            Err(_) => {}
        }
    }

    // // For Nitin
    // let nitin_address = String::from(NITIN_WALLET);
    // let nitin_vesting_info = calculate_vesting_for_now(deps, nitin_address, now)?;
    // if nitin_vesting_info.amount.u128() > 0 {
    //     distribution_details.push(nitin_vesting_info);
    // }

    // // For Ajay
    // let ajay_address = String::from(AJAY_WALLET);
    // let ajay_vesting_info = calculate_vesting_for_now(deps, ajay_address, now)?;
    // if ajay_vesting_info.amount.u128() > 0 {
    //     distribution_details.push(ajay_vesting_info);
    // }

    // // For Sameer
    // let sameer_address = String::from(SAMEER_WALLET);
    // let sameer_vesting_info = calculate_vesting_for_now(deps, sameer_address, now)?;
    // if sameer_vesting_info.amount.u128() > 0 {
    //     distribution_details.push(sameer_vesting_info);
    // }
    Ok(distribution_details)
}

fn calculate_tokens_for_this_period(
    wallet_address: Addr,
    now: Timestamp,
    vd: VestingDetails,
) -> Result<VestingInfo, ContractError> {
    // println!("entered calculate_vesting_for_now: ");
    let mut seconds_lapsed = 0;
    let now_seconds: u64 = now.seconds();
    // println!("now_seconds = {}", now_seconds);
    let vesting_start_seconds = vd.vesting_start_timestamp.seconds();
    // println!("vesting_start_seconds = {:?}", vesting_start_seconds);
    // println!("vd.vesting_periodicity = {}", vd.vesting_periodicity);
    if vd.vesting_periodicity > 0 {
        let mut vesting_intervals = 0;
        if now_seconds >= (vesting_start_seconds + (vd.cliff_period * CLIFF_PERIOD_UNIT)) {
            // the now time is greater (ahead) of vesting start + cliff
            seconds_lapsed =
                now_seconds - (vesting_start_seconds + (vd.cliff_period * CLIFF_PERIOD_UNIT));
            // println!("seconds_lapsed_1 = {}", seconds_lapsed);
            let total_vesting_intervals = seconds_lapsed / vd.vesting_periodicity;
            // println!("total_vesting_intervals = {}", total_vesting_intervals);
            // println!(
            //     "vd.last_vesting_timestamp.seconds() = {:?}",
            //     vd.last_vesting_timestamp
            // );
            // println!("vesting_start_seconds = {}", vesting_start_seconds);
            // println!("vd.cliff_period = {}", vd.cliff_period);
            let mut seconds_till_last_vesting = 0;
            if vd.last_vesting_timestamp.is_some() {
                seconds_till_last_vesting = vd.last_vesting_timestamp.unwrap().seconds()
                    - (vesting_start_seconds + vd.cliff_period * CLIFF_PERIOD_UNIT);
            }
            // println!("seconds_till_last_vesting = {}", seconds_till_last_vesting);
            let total_vested_intervals = (seconds_till_last_vesting) / vd.vesting_periodicity;
            // println!("total_vested_intervals = {}", total_vested_intervals);

            vesting_intervals = total_vesting_intervals - total_vested_intervals;
            // println!("vesting_intervals = {}", vesting_intervals);
        }
        let tokens_for_this_period_result = vd
            .vesting_count_per_period
            .checked_mul(Uint128::from(vesting_intervals));
        let mut tokens_for_this_period: Uint128;
        match tokens_for_this_period_result {
            Ok(tokens) => {
                // println!("tokens = {}", tokens);
                //Add the initial vested tokens that are not yet claimed
                tokens_for_this_period = tokens;
            }
            Err(e) => {
                // println!("error = {:?}", e);
                let mut message = String::from("error = ");
                message.push_str(e.to_string().as_str());
                tokens_for_this_period = Uint128::zero();
            }
        }
        if vd.total_vesting_token_count
            < (tokens_for_this_period
                + vd.total_claimed_tokens_till_now
                + vd.tokens_available_to_claim)
        {
            tokens_for_this_period = vd.total_vesting_token_count
                - (vd.total_claimed_tokens_till_now + vd.tokens_available_to_claim);
        }
        // println!("tokens_for_this_period = {}", tokens_for_this_period);
        //add the initial seed if cliff period is over
        if now_seconds >= (vesting_start_seconds + (vd.cliff_period * CLIFF_PERIOD_UNIT)) {
            tokens_for_this_period += vd.initial_vesting_count - vd.initial_vesting_consumed;
            // println!(
            //     "tokens_for_this_period after adding= {}",
            //     tokens_for_this_period
            // );
        }
        Ok(VestingInfo {
            spender_address: wallet_address.to_string(),
            parent_category_address: vd.parent_category_address,
            amount: tokens_for_this_period,
        })
    } else {
        return Err(ContractError::Std(StdError::generic_err(format!(
            "Vesting periodicity for {:?} address is {:?}",
            wallet_address, vd.vesting_periodicity
        ))));
    }
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
    }
}

fn claim_vested_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    //Get vesting information for the sender of this message

    //in this scenerio do we need to have vesting details as optional?

    let vd = VESTING_DETAILS.may_load(deps.storage, &info.sender)?;
    match vd {
        Some(vd) => {
            let owner_addr_str = vd.parent_category_address;
            match owner_addr_str {
                Some(owner_addr_str) => {
                    let owner_addr = deps.api.addr_validate(&owner_addr_str)?;
                    // deduct allowance before doing anything else have enough allowance
                    //in our case do we have to deduct?
                    // deduct_allowance(deps.storage, &owner_addr, &info.sender, &env.block, amount)?;
                    let config = CONFIG.load(deps.storage)?;

                    let transfer_msg = Cw20ExecuteMsg::Transfer {
                        recipient: info.sender.clone().into_string(),
                        amount: amount,
                    };
                    let exec_transfer = WasmMsg::Execute {
                        contract_addr: config.fury_token_address.to_string(),
                        msg: to_binary(&transfer_msg).unwrap(),
                        funds: vec![],
                    };
                    let send_transfer: SubMsg = SubMsg::new(exec_transfer);
                    let res = Response::new().add_submessage(send_transfer);

                    //Update vesting info for sender
                    VESTING_DETAILS.update(deps.storage, &info.sender, |vd| -> StdResult<_> {
                        match vd {
                            Some(mut v) => {
                                v.total_claimed_tokens_till_now =
                                    v.total_claimed_tokens_till_now + amount;
                                v.tokens_available_to_claim = v.tokens_available_to_claim - amount;
                                v.last_claimed_timestamp = Some(env.block.time);
                                Ok(v)
                            }
                            None => Err(StdError::GenericErr {
                                msg: String::from("Vesting Details not found"),
                            }),
                        }
                    })?;
                    return Ok(res);
                }
                None => {
                    return Err(ContractError::Std(StdError::NotFound {
                        kind: String::from("No parent category found"),
                    }))
                }
            }
        }
        None => {
            return Err(ContractError::Std(StdError::NotFound {
                kind: String::from("No vesting details found"),
            }))
        }
    };
}

fn populate_transfer_details(
    deps: &DepsMut,
    now: Timestamp,
) -> Result<Vec<VestingInfo>, ContractError> {
    let vester_addresses: Vec<String> = VESTING_DETAILS
        .keys(deps.storage, None, None, cosmwasm_std::Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();

    let mut distribution_details: Vec<VestingInfo> = Vec::new();

    for addr in vester_addresses {
        let wallet_address = deps.api.addr_validate(&addr)?;
        let vested_detais = VESTING_DETAILS.may_load(deps.storage, &wallet_address);
        match vested_detais {
            Ok(vested_detais) => {
                let vd = vested_detais.unwrap();
                if vd.should_transfer {
                    let vesting_info = calculate_tokens_for_this_period(wallet_address, now, vd)?;
                    if vesting_info.amount.u128() > 0 {
                        distribution_details.push(vesting_info);
                    }
                }
            }
            Err(_) => {}
        }
    }

    // let ga_address = String::from(GAMIFIED_AIRDROP_WALLET);
    // let ga_vesting_info = calculate_vesting_for_now(deps, ga_address, now)?;
    // distribution_details.push(ga_vesting_info);

    //Tokens to be transferred to Private Sale wallet
    // let ps_address = String::from(PRIVATE_SALE_WALLET);
    // let ps_vesting_info = calculate_vesting_for_now(deps, ps_address, now)?;
    // distribution_details.push(ps_vesting_info);
    Ok(distribution_details)
}

fn distribute_vested(
    deps: &mut DepsMut,
    sender: String,
    recipient: String,
    amount: Uint128,
) -> Result<SubMsg, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }
    let config = CONFIG.load(deps.storage)?;

    let transfer_from_msg = Cw20ExecuteMsg::TransferFrom {
        owner: sender.clone(),
        recipient: recipient.clone(),
        amount: amount,
    };

    let exec_transfer_from = WasmMsg::Execute {
        contract_addr: config.fury_token_address.to_string(),
        msg: to_binary(&transfer_from_msg).unwrap(),
        funds: vec![],
    };

    let send_transfer_from: SubMsg = SubMsg::new(exec_transfer_from);
    Ok(send_transfer_from)
}

fn periodically_transfer_to_categories(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    //capture the current system time
    let now = env.block.time;
    let config = CONFIG.load(deps.storage)?;
    //Check if the sender (one who is executing this contract) is admin
    if config.admin_wallet != info.sender {
        return Err(ContractError::Unauthorized {});
    }
    // Fetch all tokens that can be distributed as per vesting logic
    let distribution_details = populate_transfer_details(&deps, now)?;

    // Calculate the total amount to be vested
    let total_transfer_amount = calculate_total_distribution(&distribution_details);
    //Get the balance available in main wallet
    //is it similar to querying on chain where we can query the contract BALANCE via address
    let balance_query_msg = Cw20QueryMsg::Balance {
        address: info.sender.clone().into_string(),
    };
    let balance_response: BalanceResponse = deps
        .querier
        .query_wasm_smart(config.fury_token_address, &balance_query_msg)?;
    //Check if there is sufficient balance with main wallet
    // return error otherwise
    if balance_response.balance < total_transfer_amount {
        return Err(ContractError::Std(StdError::overflow(OverflowError::new(
            OverflowOperation::Sub,
            balance_response.balance,
            total_transfer_amount,
        ))));
    }

    //this one is understandable, related to with the above one
    let distribute_from = info.sender.clone().into_string();
    let mut sub_msgs: Vec<SubMsg> = Vec::new();
    let mut attribs: Vec<Attribute> = Vec::new();
    for elem in distribution_details {
        // Transfer the funds
        let res = distribute_vested(
            &mut deps,
            distribute_from.clone(),
            elem.spender_address.clone(),
            elem.amount,
        )?;
        sub_msgs.push(res);
        attribs.push(Attribute {
            key: "action".to_string(),
            value: "transfer".to_string(),
        });
        attribs.push(Attribute {
            key: "from".to_string(),
            value: distribute_from.clone(),
        });
        attribs.push(Attribute {
            key: "to".to_string(),
            value: elem.spender_address.clone(),
        });
        attribs.push(Attribute {
            key: "amount".to_string(),
            value: elem.amount.to_string(),
        });
        // Save distribution information
        update_vesting_details(
            &mut deps,
            elem.spender_address.clone(),
            env.block.time,
            Some(elem),
            None,
        )?;
    }
    Ok(Response::new()
        .add_submessages(sub_msgs)
        .add_attributes(attribs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::VestingDetails { address } => to_binary(&query_vesting_details(deps, address)?),
    }
}

fn query_vesting_details(deps: Deps, address: String) -> StdResult<VestingDetails> {
    let address = deps.api.addr_validate(&address)?;
    let vd = VESTING_DETAILS.may_load(deps.storage, &address)?;
    match vd {
        Some(vd) => return Ok(vd),
        None => return Err(StdError::generic_err("No vesting details found")),
    };
}

pub fn query_balance(deps: Deps, address: String, info: MessageInfo) -> StdResult<BalanceResponse> {
    let address = deps.api.addr_validate(&address)?;
    let config = CONFIG.load(deps.storage)?;
    let balance_msg = Cw20QueryMsg::Balance {
        address: info.sender.clone().into_string(),
    };
    let balance_response: cw20::BalanceResponse = deps
        .querier
        .query_wasm_smart(config.fury_token_address.clone(), &balance_msg)?;

    let balance = balance_response.balance;
    Ok(BalanceResponse { balance })
}

pub fn query_minter(deps: Deps) -> StdResult<Option<MinterResponse>> {
    let meta = TOKEN_INFO.load(deps.storage)?;
    let minter = match meta.mint {
        Some(m) => Some(MinterResponse {
            minter: m.minter.into(),
            cap: m.cap,
        }),
        None => None,
    };
    Ok(minter)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

#[cfg(test)]
mod tests {
    // use crate::msg::InstantiateMsg;
    use cosmwasm_std::testing::{mock_dependencies, mock_env};
    use cosmwasm_std::Addr;

    use super::*;

    fn get_balance<T: Into<String>>(deps: Deps, address: T) -> Uint128 {
        query_balance(deps, address.into()).unwrap().balance
    }

    #[test]
    fn vesting_test_cases() {
        assert_eq!(1, 1);
    }

    // this will set up the instantiation for other tests
    fn do_instantiate(deps: DepsMut, addr: &str, amount: Uint128) -> TokenInfoResponse {
        _do_instantiate(deps, addr, amount, None)
    }

    // this will set up the instantiation for other tests
    fn _do_instantiate(
        mut deps: DepsMut,
        addr: &str,
        amount: Uint128,
        mint: Option<MinterResponse>,
    ) -> TokenInfoResponse {
        let instantiate_msg = InstantiateMsg {
//WIP
            admin_wallet: "Addr",
            fury_token_contract: "Addr",
            vesting: "InstantiateVestingSchedulesInfo",
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();
        let res = instantiate(deps.branch(), env, info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        let meta = query_token_info(deps.as_ref()).unwrap();
        assert_eq!(
            meta,
            TokenInfoResponse {
                name: "Auto Gen".to_string(),
                symbol: "AUTO".to_string(),
                decimals: 3,
                total_supply: amount,
            }
        );
        assert_eq!(get_balance(deps.as_ref(), addr), amount);
        assert_eq!(query_minter(deps.as_ref()).unwrap(), mint,);
        meta
    }

    #[test]
    fn transfer_to_categories() {
        let mut deps = mock_dependencies(&[]);
        let distribute_from = String::from("addr0001");
        let distribute_to = String::from("addr0002");
        let amount = Uint128::from(1000u128);

        do_instantiate(deps.as_mut(), &distribute_from, amount);

        let init_from_balance = get_balance(deps.as_ref(), distribute_from.clone());
        let init_to_balance = get_balance(deps.as_ref(), distribute_to.clone());

        // Transfer the funds
        let mut_deps = &mut deps.as_mut();
        let res = distribute_vested(
            mut_deps,
            distribute_from.clone(),
            distribute_to.clone(),
            amount,
        );

        let calc_new_from_balance = init_from_balance - amount;
        let calc_new_to_balance = init_to_balance + amount;

        let new_from_balance = get_balance(deps.as_ref(), distribute_from);
        let new_to_balance = get_balance(deps.as_ref(), distribute_to);
        // check that the transfer happened
        assert_eq!(calc_new_from_balance, new_from_balance);
        assert_eq!(calc_new_to_balance, new_to_balance);
    }

    #[test]
    fn fail_transfer_to_categories() {
        let mut deps = mock_dependencies(&[]);
        let distribute_from = String::from("addr0001");
        let distribute_to = String::from("addr0002");
        let amount1 = Uint128::from(1000u128);

        do_instantiate(deps.as_mut(), &distribute_from, amount1);

        let init_from_balance = get_balance(deps.as_ref(), distribute_from.clone());
        let init_to_balance = get_balance(deps.as_ref(), distribute_to.clone());

        let amount = init_from_balance + Uint128::from(1000u128);

        // Try to transfer more than the funds available - it should fail
        let mut_deps = &mut deps.as_mut();
        let res = distribute_vested(
            mut_deps,
            distribute_from.clone(),
            distribute_to.clone(),
            amount,
        );

        let new_from_balance = get_balance(deps.as_ref(), distribute_from);
        let new_to_balance = get_balance(deps.as_ref(), distribute_to);

        // check that the transfer did not happen
        assert_eq!(new_from_balance, init_from_balance);
        assert_eq!(new_to_balance, init_to_balance);
    }

    fn get_vesting_details() -> VestingDetails {
        let now = mock_env().block.time;
        let category_address = String::from("addr0002");
        return VestingDetails {
            vesting_start_timestamp: now,
            initial_vesting_count: Uint128::zero(),
            initial_vesting_consumed: Uint128::zero(),
            vesting_periodicity: 300, // In seconds
            vesting_count_per_period: Uint128::from(10u128),
            total_vesting_token_count: Uint128::from(2000u128),
            total_claimed_tokens_till_now: Uint128::zero(),
            last_claimed_timestamp: None,
            tokens_available_to_claim: Uint128::zero(),
            last_vesting_timestamp: None,
            cliff_period: 0, // in months
            parent_category_address: Some(category_address),
            should_transfer: true,
        };
    }

    #[test]
    fn test_vesting_at_tge() {
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today
        println!("now {:?}", now);

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.tokens_available_to_claim += vesting_details.vesting_count_per_period;
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }
    #[test]
    fn test_vesting_at_tge_with_initial_seed() {
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(1000u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }
    #[test]
    fn test_vesting_at_tge_no_initial_seed_first_interval() {
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity);
        let vcpp = vesting_details.vesting_count_per_period.u128();
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount.u128(), vcpp);
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_initial_seed_first_interval() {
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(1010u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }
    #[test]
    fn test_vesting_at_tge_no_initial_seed_2_uncalc_interval() {
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(20u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }
    #[test]
    fn test_vesting_at_tge_with_initial_seed_2_uncalc_interval() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(1020u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1vested_1uncalc_interval() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();

        vesting_details.tokens_available_to_claim = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp =
            now.minus_seconds(vesting_details.vesting_periodicity * 2);

        vesting_details.last_vesting_timestamp =
            Some(now.minus_seconds(vesting_details.vesting_periodicity));

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(10u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1claimed_1uncalc_interval() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();

        vesting_details.total_claimed_tokens_till_now = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp =
            now.minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);

        vesting_details.last_vesting_timestamp = Some(
            now.minus_seconds(vesting_details.vesting_periodicity)
                .minus_seconds(5),
        );

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(10u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1claimed_half_uncalc_interval() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();

        vesting_details.total_claimed_tokens_till_now = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp =
            now.minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);

        vesting_details.last_vesting_timestamp = Some(
            now.minus_seconds(vesting_details.vesting_periodicity)
                .minus_seconds(5),
        );

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(10u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today
        println!("now {:?}", now);

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.tokens_available_to_claim += vesting_details.vesting_count_per_period;
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_initial_seed_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::zero());
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_first_interval_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity);
        let vcpp = vesting_details.vesting_count_per_period.u128();
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount.u128(), 0u128);
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_initial_seed_first_interval_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_2_uncalc_interval_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_initial_seed_2_uncalc_intervalwith_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1vested_1uncalc_interval_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.tokens_available_to_claim = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp =
            now.minus_seconds(vesting_details.vesting_periodicity * 2);

        vesting_details.last_vesting_timestamp =
            Some(now.minus_seconds(vesting_details.vesting_periodicity));

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1claimed_1uncalc_interval_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.total_claimed_tokens_till_now = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp =
            now.minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);

        vesting_details.last_vesting_timestamp = Some(
            now.minus_seconds(vesting_details.vesting_periodicity)
                .minus_seconds(5),
        );

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1claimed_half_uncalc_interval_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.total_claimed_tokens_till_now = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp =
            now.minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);

        vesting_details.last_vesting_timestamp = Some(
            now.minus_seconds(vesting_details.vesting_periodicity)
                .minus_seconds(5),
        );

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today
        println!("now {:?}", now);

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;
        vesting_details.tokens_available_to_claim += vesting_details.vesting_count_per_period;
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_initial_seed_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today
                                         // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(1000u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_first_interval_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity);
        let vcpp = vesting_details.vesting_count_per_period.u128();
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount.u128(), vcpp);
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_initial_seed_first_interval_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(1010u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_2_uncalc_interval_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(20u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_initial_seed_2_uncalc_interval_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(1020u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1vested_1uncalc_interval_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;

        vesting_details.tokens_available_to_claim = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);

        vesting_details.last_vesting_timestamp =
            Some(now.minus_seconds(vesting_details.vesting_periodicity));

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(10u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1claimed_1uncalc_interval_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;

        vesting_details.total_claimed_tokens_till_now = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);

        vesting_details.last_vesting_timestamp = Some(
            now.minus_seconds(vesting_details.vesting_periodicity)
                .minus_seconds(5),
        );

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(10u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1claimed_half_uncalc_interval_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;

        vesting_details.total_claimed_tokens_till_now = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);

        vesting_details.last_vesting_timestamp = Some(
            now.minus_seconds(vesting_details.vesting_periodicity)
                .minus_seconds(5),
        );

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(10u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();

        // vesting_periodicity = 86400; // in seconds
        // vesting_started_before = 92; // in days
        // cliff_period = 3; // in months
        // vesting_start_timestamp = mock_env()
        //     .block
        //     .time
        //     .minus_seconds(vesting_started_before * 86400);
        // last_vesting_timestamp = mock_env().block.time;
        // total_vesting_token_count = Uint128::from(200u128);
        // total_claimed_tokens_till_now = Uint128::from(0u128);
        // tokens_available_to_claim = Uint128::from(10000u128);
        // let vested_amount = calculate_tokens_for_this_period(
        //     Addr::unchecked(category_address.clone()),
        //     now,
        //     VestingDetails {
        //         vesting_start_timestamp: vesting_start_timestamp,
        //         initial_vesting_count: initial_vesting_count,
        //         initial_vesting_consumed: initial_vesting_consumed,
        //         vesting_periodicity: vesting_periodicity,
        //         vesting_count_per_period: vesting_count_per_period,
        //         total_vesting_token_count: total_vesting_token_count,
        //         total_claimed_tokens_till_now: total_claimed_tokens_till_now,
        //         last_claimed_timestamp: last_claimed_timestamp,
        //         tokens_available_to_claim: tokens_available_to_claim,
        //         last_vesting_timestamp: last_vesting_timestamp,
        //         cliff_period: cliff_period,
        //         category_address: Some(category_address.clone()),
        //     },
        // );
        // match vested_amount {
        //     Ok(va) => {
        //         assert_eq!(va.amount, Uint128::from(200u128));
        //     }
        //     Err(e) => {
        //         assert_eq!(1, 0);
        //     }
        // }

        // vesting_periodicity = 86400; // in seconds
        // vesting_started_before = 90; // in days
        // cliff_period = 3; // in months
        // vesting_start_timestamp = mock_env()
        //     .block
        //     .time
        //     .minus_seconds(vesting_started_before * 86400);
        // last_vesting_timestamp = mock_env().block.time;
        // total_vesting_token_count = Uint128::from(200u128);
        // total_claimed_tokens_till_now = Uint128::from(0u128);
        // tokens_available_to_claim = Uint128::from(10000u128);
        // let vested_amount = calculate_tokens_for_this_period(
        //     Addr::unchecked(category_address.clone()),
        //     now,
        //     VestingDetails {
        //         vesting_start_timestamp: vesting_start_timestamp,
        //         initial_vesting_count: initial_vesting_count,
        //         initial_vesting_consumed: initial_vesting_consumed,
        //         vesting_periodicity: vesting_periodicity,
        //         vesting_count_per_period: vesting_count_per_period,
        //         total_vesting_token_count: total_vesting_token_count,
        //         total_claimed_tokens_till_now: total_claimed_tokens_till_now,
        //         last_claimed_timestamp: last_claimed_timestamp,
        //         tokens_available_to_claim: tokens_available_to_claim,
        //         last_vesting_timestamp: last_vesting_timestamp,
        //         cliff_period: cliff_period,
        //         category_address: Some(category_address.clone()),
        //     },
        // );
        // match vested_amount {
        //     Ok(va) => {
        //         assert_eq!(va.amount, Uint128::zero());
        //     }
        //     Err(e) => {
        //         assert_eq!(1, 0);
        //     }
        // }

        // vesting_periodicity = 86400; // in seconds
        // vesting_started_before = 89; // in days
        // cliff_period = 3; // in months
        // let mut vesting_start_timestamp = mock_env()
        //     .block
        //     .time
        //     .minus_seconds(vesting_started_before * 86400);
        // last_vesting_timestamp = mock_env().block.time;
        // total_vesting_token_count = Uint128::from(200u128);
        // total_claimed_tokens_till_now = Uint128::from(0u128);
        // tokens_available_to_claim = Uint128::from(10000u128);
        // let vested_amount = calculate_tokens_for_this_period(
        //     Addr::unchecked(category_address.clone()),
        //     now,
        //     VestingDetails {
        //         vesting_start_timestamp: vesting_start_timestamp,
        //         initial_vesting_count: initial_vesting_count,
        //         initial_vesting_consumed: initial_vesting_consumed,
        //         vesting_periodicity: vesting_periodicity,
        //         vesting_count_per_period: vesting_count_per_period,
        //         total_vesting_token_count: total_vesting_token_count,
        //         total_claimed_tokens_till_now: total_claimed_tokens_till_now,
        //         last_claimed_timestamp: last_claimed_timestamp,
        //         tokens_available_to_claim: tokens_available_to_claim,
        //         last_vesting_timestamp: last_vesting_timestamp,
        //         cliff_period: cliff_period,
        //         category_address: Some(category_address.clone()),
        //     },
        // );
        // match vested_amount {
        //     Ok(va) => {
        //         assert_eq!(va.amount, Uint128::zero());
        //     }
        //     Err(e) => {
        //         assert_eq!(1, 0);
        //     }
        // }

        // vesting_periodicity = 86400; // in seconds
        // vesting_started_before = 89; // in days
        // cliff_period = 0; // in months
        // let mut vesting_start_seconds =
        //     mock_env().block.time.seconds() - vesting_started_before * 86400;
        // last_vesting_timestamp = mock_env().block.time;
        // total_vesting_token_count = Uint128::from(200u128);
        // total_claimed_tokens_till_now = Uint128::from(0u128);
        // tokens_available_to_claim = Uint128::from(10000u128);
        // let vested_amount = calculate_tokens_for_this_period(
        //     Addr::unchecked(category_address.clone()),
        //     now,
        //     VestingDetails {
        //         vesting_start_timestamp: vesting_start_timestamp,
        //         initial_vesting_count: initial_vesting_count,
        //         initial_vesting_consumed: initial_vesting_consumed,
        //         vesting_periodicity: vesting_periodicity,
        //         vesting_count_per_period: vesting_count_per_period,
        //         total_vesting_token_count: total_vesting_token_count,
        //         total_claimed_tokens_till_now: total_claimed_tokens_till_now,
        //         last_claimed_timestamp: last_claimed_timestamp,
        //         tokens_available_to_claim: tokens_available_to_claim,
        //         last_vesting_timestamp: last_vesting_timestamp,
        //         cliff_period: cliff_period,
        //         category_address: Some(category_address.clone()),
        //     },
        // );
        // match vested_amount {
        //     Ok(va) => {
        //         assert_eq!(va.amount, Uint128::from(8900u128));
        //     }
        //     Err(e) => {
        //         assert_eq!(1, 0);
        //     }
        // }

        // vesting_periodicity = 0; // in seconds - immediately vest
        // vesting_started_before = 89; // in days
        // cliff_period = 0; // in months
        // vesting_start_seconds = mock_env().block.time.seconds() - vesting_started_before * 86400;
        // last_vesting_timestamp = mock_env().block.time;
        // total_vesting_token_count = Uint128::from(200u128);
        // total_claimed_tokens_till_now = Uint128::from(0u128);
        // tokens_available_to_claim = Uint128::from(10000u128);
        // let vested_amount = calculate_tokens_for_this_period(
        //     Addr::unchecked(category_address.clone()),
        //     now,
        //     VestingDetails {
        //         vesting_start_timestamp: vesting_start_timestamp,
        //         initial_vesting_count: initial_vesting_count,
        //         initial_vesting_consumed: initial_vesting_consumed,
        //         vesting_periodicity: vesting_periodicity,
        //         vesting_count_per_period: vesting_count_per_period,
        //         total_vesting_token_count: total_vesting_token_count,
        //         total_claimed_tokens_till_now: total_claimed_tokens_till_now,
        //         last_claimed_timestamp: last_claimed_timestamp,
        //         tokens_available_to_claim: tokens_available_to_claim,
        //         last_vesting_timestamp: last_vesting_timestamp,
        //         cliff_period: cliff_period,
        //         category_address: Some(category_address.clone()),
        //     },
        // );
        // match vested_amount {
        //     Ok(va) => {
        //         assert_eq!(va.amount, Uint128::zero());
        //     }
        //     Err(e) => {
        //         assert_eq!(1, 0);
        //     }
        // }
    }
}

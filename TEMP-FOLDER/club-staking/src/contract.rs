#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, BankMsg, Binary, CosmosMsg, Deps, DepsMut, Env, MessageInfo,
    Order, Reply, ReplyOn, Response, StdError, StdResult, Storage, SubMsg, Uint128, WasmMsg,
};

use cw2::set_contract_version;
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};

use astroport::pair::PoolResponse;
use astroport::pair::QueryMsg::Pool;

// use crate::allowances::{
//     deduct_allowance, execute_burn_from, execute_decrease_allowance, execute_increase_allowance,
//     execute_send_from, execute_transfer_from, query_allowance,
// };
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ReceivedMsg};
use crate::state::{
    ClubBondingDetails, ClubOwnershipDetails, ClubPreviousOwnerDetails, ClubStakingDetails, Config,
    CLUB_BONDING_DETAILS, CLUB_OWNERSHIP_DETAILS, CLUB_PREVIOUS_OWNER_DETAILS,
    CLUB_REWARD_NEXT_TIMESTAMP, CLUB_STAKING_DETAILS, CONFIG, REWARD, STAKING_FUNDS,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:club-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const INCREASE_STAKE: bool = true;
const DECREASE_STAKE: bool = false;
const IMMEDIATE_WITHDRAWAL: bool = true;
const NO_IMMEDIATE_WITHDRAWAL: bool = false;
const QUERY_PAIR_POOL: bool = true;
const DONT_QUERY_PAIR_POOL: bool = false;

// Reward to club owner for buying - 0 tokens
const CLUB_BUYING_REWARD_AMOUNT: u128 = 0u128;

// Reward to club staker for staking - 0 tokens
const CLUB_STAKING_REWARD_AMOUNT: u128 = 0u128;

// This is reduced to 0 day locking period in seconds, after buying a club, as no refund planned for Ownership Fee
const CLUB_LOCKING_DURATION: u64 = 0u64;

// This is locking period in seconds, after staking in club.
// No longer applicable so setting it to 0
const CLUB_STAKING_DURATION: u64 = 0u64;

// this is 7 day bonding period in seconds, after withdrawing a stake
// TODO _ Revert after DEBUG : this is 1 hour for testing purposes only
// const CLUB_BONDING_DURATION: u64 = 3600u64;

use cosmwasm_std::{Coin, Timestamp};

const HUNDRED_PERCENT: u128 = 10000u128;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let mut next_reward_time = msg.club_reward_next_timestamp;
    if next_reward_time.seconds() == 0u64 {
        next_reward_time = _env.block.time.minus_seconds(1);
    }
    let config = Config {
        admin_address: deps.api.addr_validate(&msg.admin_address)?,
        minting_contract_address: deps.api.addr_validate(&msg.minting_contract_address)?,
        astro_proxy_address: deps.api.addr_validate(&msg.astro_proxy_address)?,
        club_fee_collector_wallet: deps.api.addr_validate(&msg.club_fee_collector_wallet)?,
        club_reward_next_timestamp: next_reward_time,
        reward_periodicity: msg.reward_periodicity,
        club_price: msg.club_price,
        bonding_duration: msg.bonding_duration,
        platform_fees_collector_wallet: deps
            .api
            .addr_validate(&msg.platform_fees_collector_wallet)?,
        platform_fees: msg.platform_fees,
        transaction_fees: msg.transaction_fees,
        control_fees: msg.control_fees,
    };
    CONFIG.save(deps.storage, &config)?;
    CLUB_REWARD_NEXT_TIMESTAMP.save(deps.storage, &config.club_reward_next_timestamp)?;
    println!(
        "now = {:?} next_timestamp = {:?} periodicity = {:?}",
        _env.block.time, config.club_reward_next_timestamp, config.reward_periodicity
    );
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
        ExecuteMsg::Receive(msg) => received_message(deps, env, info, msg),
        ExecuteMsg::StakeOnAClub {
            staker,
            club_name,
            amount,
        } => {
            stake_on_a_club(deps, env, info, staker, club_name, amount)
        }
        ExecuteMsg::BuyAClub {
            buyer,
            seller,
            club_name,
        } => {
            let config = CONFIG.load(deps.storage)?;
            let price = config.club_price;
            buy_a_club(deps, env, info, buyer, seller, club_name, price, QUERY_PAIR_POOL)
        }
        ExecuteMsg::ReleaseClub { owner, club_name } => {
            release_club(deps, env, info, owner, club_name)
        }
        ExecuteMsg::ClaimOwnerRewards { owner, club_name } => {
            claim_owner_rewards(deps, env, info, owner, club_name)
        }
        ExecuteMsg::ClaimPreviousOwnerRewards { previous_owner } => {
            claim_previous_owner_rewards(deps, info, previous_owner)
        }
        ExecuteMsg::StakeWithdrawFromAClub {
            staker,
            club_name,
            amount,
            immediate_withdrawal,
        } => withdraw_stake_from_a_club(
            deps,
            env,
            info,
            staker,
            club_name,
            amount,
            immediate_withdrawal,
        ),
        ExecuteMsg::CalculateAndDistributeRewards {} => {
            calculate_and_distribute_rewards(deps, env, info)
        }
        ExecuteMsg::ClaimRewards { staker, club_name } => {
            claim_rewards(deps, info, staker, club_name)
        }
        ExecuteMsg::PeriodicallyRefundStakeouts {} => {
            periodically_refund_stakeouts(deps, env, info)
        }
    }
}

fn received_message(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    message: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    let msg: ReceivedMsg = from_binary(&message.msg)?;
    let amount = Uint128::from(message.amount);
    match msg {
        ReceivedMsg::IncreaseRewardAmount(irac) => {
            increase_reward_amount(deps, env, info, irac.reward_from, amount)
        }
    }
    // Err(ContractError::Std(StdError::GenericErr {
    //     msg: format!("received_message where msg = {:?}", msg),
    // }))
}

fn claim_previous_owner_rewards(
    deps: DepsMut,
    info: MessageInfo,
    previous_owner: String,
) -> Result<Response, ContractError> {
    let mut amount = Uint128::zero();
    let mut transfer_confirmed = false;
    let previous_owner_addr = deps.api.addr_validate(&previous_owner)?;
    //Check if withdrawer is same as invoker
    if previous_owner_addr != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let previous_ownership_details;
    let previous_ownership_details_result =
        CLUB_PREVIOUS_OWNER_DETAILS.may_load(deps.storage, previous_owner.clone());
    match previous_ownership_details_result {
        Ok(od) => {
            previous_ownership_details = od;
        }
        Err(e) => {
            return Err(ContractError::Std(StdError::from(e)));
        }
    }

    if !(previous_ownership_details.is_none()) {
        for previous_owner_detail in previous_ownership_details {
            if previous_owner_detail.previous_owner_address == previous_owner.clone() {
                if Uint128::zero() == previous_owner_detail.reward_amount {
                    return Err(ContractError::Std(StdError::GenericErr {
                        msg: String::from("No rewards for this previous owner"),
                    }));
                }

                amount = previous_owner_detail.reward_amount;

                // Now remove the previous ownership details
                CLUB_PREVIOUS_OWNER_DETAILS.remove(deps.storage, previous_owner.clone());

                // Add amount to the owners wallet
                transfer_confirmed = true;
            }
        }
    }
    if transfer_confirmed == false {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Not a valid previous owner for the club"),
        }));
    }
    transfer_from_contract_to_wallet(
        deps.storage,
        previous_owner.clone(),
        amount,
        "previous_owners_reward".to_string(),
    )
}

fn claim_owner_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: String,
    club_name: String,
) -> Result<Response, ContractError> {
    let mut amount = Uint128::zero();
    let mut transfer_confirmed = false;
    let owner_addr = deps.api.addr_validate(&owner)?;
    //Check if withdrawer is same as invoker
    if owner_addr != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let ownership_details;
    let ownership_details_result = CLUB_OWNERSHIP_DETAILS.may_load(deps.storage, club_name.clone());
    match ownership_details_result {
        Ok(od) => {
            ownership_details = od;
        }
        Err(e) => {
            return Err(ContractError::Std(StdError::from(e)));
        }
    }

    if !(ownership_details.is_none()) {
        for owner_detail in ownership_details {
            if owner_detail.owner_address == owner.clone() {
                if Uint128::zero() == owner_detail.reward_amount {
                    return Err(ContractError::Std(StdError::GenericErr {
                        msg: String::from("No rewards for this owner"),
                    }));
                }

                transfer_confirmed = true;

                amount = owner_detail.reward_amount;

                // Now save the ownership details
                CLUB_OWNERSHIP_DETAILS.save(
                    deps.storage,
                    club_name.clone(),
                    &ClubOwnershipDetails {
                        club_name: owner_detail.club_name,
                        start_timestamp: owner_detail.start_timestamp,
                        locking_period: owner_detail.locking_period,
                        owner_address: owner_detail.owner_address,
                        price_paid: owner_detail.price_paid,
                        reward_amount: Uint128::zero(),
                        owner_released: owner_detail.owner_released,
                    },
                )?;
            }
        }
    }

    if transfer_confirmed == false {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Not a valid owner for the club"),
        }));
    }
    transfer_from_contract_to_wallet(
        deps.storage,
        owner.clone(),
        amount,
        "owner_reward".to_string(),
    )
}

fn periodically_refund_stakeouts(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized {});
    }

    //capture the current system time
    let now = env.block.time;

    // Fetch all bonding details
    let all_clubs: Vec<String> = CLUB_BONDING_DETAILS
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for club_name in all_clubs {
        let mut all_bonds = Vec::new();
        let bonding_details = CLUB_BONDING_DETAILS.load(deps.storage, club_name.clone())?;
        for mut bond in bonding_details {
            let mut duration = bond.bonding_duration;
            let now_minus_duration_timestamp = now.minus_seconds(duration);
            if now_minus_duration_timestamp < bond.bonding_start_timestamp {
                all_bonds.push(bond);
            } else {
                // transfer to bonder wallet
                // NOT reqd exdternally
                // transfer_from_contract_to_wallet(deps.storage, bond.bonder_address, bond.bonded_amount);
            }
        }
        CLUB_BONDING_DETAILS.save(deps.storage, club_name, &all_bonds)?;
    }
    return Ok(Response::default());
}

fn buy_a_club(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    buyer: String,
    seller_opt: Option<String>,
    club_name: String,
    price: Uint128,
    is_query_needed: bool,
) -> Result<Response, ContractError> {
    println!("seller_opt = {:?}", seller_opt);
    let seller;
    match seller_opt.clone() {
        Some(s) => seller = s,
        None => seller = String::default(),
    }

    let config = CONFIG.load(deps.storage)?;

    let club_price = config.club_price;
    if price != club_price {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Club price is not matching"),
        }));
    }

	let mut required_ust_fees = Uint128::zero();
    let mut fees = Uint128::zero();
    if is_query_needed {
		required_ust_fees = query_platform_fees(
			deps.as_ref(),
			to_binary(&ExecuteMsg::BuyAClub {
				buyer: buyer.clone(),
				club_name: club_name.clone(),
				seller: seller_opt,
			})?,
		)?;
		for fund in info.funds {
			if fund.denom == "uusd" {
				fees = fees.checked_add(fund.amount).unwrap();
			}
		}
	}

    if fees != required_ust_fees {
        return Err(ContractError::InsufficientFees {
            required: required_ust_fees,
            received: fees,
        });
    }
    let buyer_addr = deps.api.addr_validate(&buyer)?;

    let ownership_details;
    let ownership_details_result = CLUB_OWNERSHIP_DETAILS.may_load(deps.storage, club_name.clone());
    match ownership_details_result {
        Ok(od) => {
            ownership_details = od;
        }
        Err(e) => {
            return Err(ContractError::Std(StdError::from(e)));
        }
    }

    let all_clubs: Vec<String> = CLUB_OWNERSHIP_DETAILS
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();

    for one_club_name in all_clubs {
        let one_ownership_details =
            CLUB_OWNERSHIP_DETAILS.load(deps.storage, one_club_name.clone())?;
        if buyer == one_ownership_details.owner_address {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: String::from("buyer already owns this club"),
            }));
        }
    }

    let mut previous_owners_reward_amount = Uint128::from(0u128);

    if !(ownership_details.is_none()) {
        for owner in ownership_details {
            if owner.owner_released == false {
                return Err(ContractError::Std(StdError::GenericErr {
                    msg: String::from("Owner has not released the club"),
                }));
            } else if owner.owner_address != String::default() && owner.owner_address != seller {
                println!(
                    "owner.owner_address = {:?} and seller = {:?}",
                    owner.owner_address, seller
                );
                return Err(ContractError::Std(StdError::GenericErr {
                    msg: String::from("Seller is not the owner for the club"),
                }));
            }

            // Evaluate previous owner rewards
            previous_owners_reward_amount = owner.reward_amount;
            println!("prv own amount picked {:?}", previous_owners_reward_amount);
            let mut previous_reward = Uint128::zero();
            println!("prv own amount avl {:?}", previous_owners_reward_amount);
            if previous_owners_reward_amount != Uint128::zero() {
                let pod = CLUB_PREVIOUS_OWNER_DETAILS.may_load(deps.storage, seller.clone())?;
                match pod {
                    Some(pod) => {
                        previous_reward = pod.reward_amount;
                        println!("prv own existing reward {:?}", previous_reward);
                    }
                    None => {}
                }

                // Now save the previous ownership details
                CLUB_PREVIOUS_OWNER_DETAILS.save(
                    deps.storage,
                    seller.clone(),
                    &ClubPreviousOwnerDetails {
                        previous_owner_address: seller.clone(),
                        reward_amount: previous_reward + previous_owners_reward_amount,
                    },
                )?;
            }
        }
    }

    // Now save the ownership details
    CLUB_OWNERSHIP_DETAILS.save(
        deps.storage,
        club_name.clone(),
        &ClubOwnershipDetails {
            club_name: club_name.clone(),
            start_timestamp: env.block.time,
            locking_period: CLUB_LOCKING_DURATION,
            owner_address: buyer_addr.to_string(),
            price_paid: price,
            reward_amount: Uint128::from(CLUB_BUYING_REWARD_AMOUNT),
            owner_released: false,
        },
    )?;

    let config = CONFIG.load(deps.storage)?;

    let transfer_msg = Cw20ExecuteMsg::TransferFrom {
        owner: info.sender.into_string(),
        recipient: config.club_fee_collector_wallet.to_string(),
        amount: price,
    };
    let exec = WasmMsg::Execute {
        contract_addr: config.minting_contract_address.to_string(),
        msg: to_binary(&transfer_msg).unwrap(),
        funds: vec![
            // Coin {
            //     denom: token_info.name.to_string(),
            //     amount: price,
            // },
        ],
        // TODO Add a memo inthe transfer msg that this is club staking fee
    };

    // let send: SubMsg = SubMsg::new(exec);
    let send_wasm: CosmosMsg = CosmosMsg::Wasm(exec);
    let send_bank: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
        to_address: config.platform_fees_collector_wallet.into_string(),
        amount: info.funds,
    });
    //send.id = 1000;
    //send.reply_on = ReplyOn::Error;
    let data_msg = format!("Club fees {} received", price).into_bytes();
    return Ok(Response::new()
        .add_message(send_wasm)
        .add_message(send_bank)
        .add_attribute("action", "buy_a_club")
        .add_attribute("buyer", buyer)
        .add_attribute("club_name", club_name)
        .add_attribute("fees", price.to_string())
        .set_data(data_msg));
    //return Ok(Response::default());
}

#[entry_point]
pub fn reply(deps: DepsMut, env: Env, reply: Reply) -> Result<Response, ContractError> {
    return Err(ContractError::Std(StdError::GenericErr {
        msg: format!("the reply details are {:?}", reply),
    }));
}

fn release_club(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    seller: String,
    club_name: String,
) -> Result<Response, ContractError> {
    let seller_addr = deps.api.addr_validate(&seller)?;
    //Check if seller is same as invoker
    if seller_addr != info.sender {
        return Err(ContractError::Unauthorized {});
    }
    let ownership_details;
    let ownership_details_result = CLUB_OWNERSHIP_DETAILS.may_load(deps.storage, club_name.clone());
    match ownership_details_result {
        Ok(od) => {
            ownership_details = od;
        }
        Err(e) => {
            return Err(ContractError::Std(StdError::from(e)));
        }
    }

    // check that the current ownership is with the seller
    if ownership_details.is_none() {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Releaser is not the owner for the club"),
        }));
    }
    for owner in ownership_details {
        if owner.owner_address != seller_addr {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: String::from("Releaser is not the owner for the club"),
            }));
        } else {
            //capture the current system time
            let now = env.block.time;
            let mut duration = owner.locking_period;
            let now_minus_duration_timestamp = now.minus_seconds(duration);
            if now_minus_duration_timestamp < owner.start_timestamp {
                return Err(ContractError::Std(StdError::GenericErr {
                    msg: String::from("Locking period for the club is not over"),
                }));
            } else {
                // Update the ownership details
                CLUB_OWNERSHIP_DETAILS.save(
                    deps.storage,
                    club_name.clone(),
                    &ClubOwnershipDetails {
                        club_name: owner.club_name,
                        start_timestamp: owner.start_timestamp,
                        locking_period: owner.locking_period,
                        owner_address: owner.owner_address,
                        price_paid: owner.price_paid,
                        reward_amount: owner.reward_amount,
                        owner_released: true,
                    },
                )?;
            }
        }
    }
    return Ok(Response::default());
}

fn stake_on_a_club(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    staker: String,
    club_name: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let staker_addr = deps.api.addr_validate(&staker)?;

    let required_ust_fees: Uint128;
    //To bypass calls from unit tests
    if info.sender.clone().into_string() == String::from("minting_admin11111")
    {
        required_ust_fees = Uint128::zero();
    } else {
        required_ust_fees = query_platform_fees(
            deps.as_ref(),
            to_binary(&ExecuteMsg::StakeOnAClub {
                staker: staker.clone(),
                club_name: club_name.clone(),
                amount: amount,
            })?,
        )?;
    }
    let mut fees = Uint128::zero();
    for fund in info.funds.clone() {
        if fund.denom == "uusd" {
            fees = fees.checked_add(fund.amount).unwrap();
        }
    }
    if fees < required_ust_fees {
        return Err(ContractError::InsufficientFees {
            required: required_ust_fees,
            received: fees,
        });
    }

    //check if the club_name is available for staking
    let ownership_details;
    let ownership_details_result = CLUB_OWNERSHIP_DETAILS.may_load(deps.storage, club_name.clone());
    match ownership_details_result {
        Ok(od) => {
            ownership_details = od;
        }
        Err(e) => {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: String::from("Cannot find the club"),
            }));
        }
    }
    if ownership_details.is_some() {
        // Now save the staking details
        save_staking_details(
            deps.storage,
            env,
            staker.clone(),
            club_name.clone(),
            amount,
            INCREASE_STAKE,
        )?;

        //If successfully staked, save the funds in contract wallet
        STAKING_FUNDS.update(
            deps.storage,
            &staker_addr,
            |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
        )?;

        // Nothing required to transfer anything staking fund has arrived in the staking contract
    } else {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("The club is not available for staking"),
        }));
    }

    let send_bank: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
        to_address: config.platform_fees_collector_wallet.into_string(),
        amount: info.funds,
    });
    let data_msg = format!("Club stake {} received", amount).into_bytes();
    return Ok(Response::new()
        .add_message(send_bank)
        .add_attribute("action", "stake_on_a_club")
        .add_attribute("staker", staker)
        .add_attribute("club_name", club_name)
        .add_attribute("stake", amount.to_string())
        .set_data(data_msg));
}

fn withdraw_stake_from_a_club(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    staker: String,
    club_name: String,
    withdrawal_amount: Uint128,
    immediate_withdrawal: bool,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let staker_addr = deps.api.addr_validate(&staker)?;
    //Check if withdrawer is same as invoker
    if staker_addr != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    //check if the club_name is available for staking
    let ownership_details;
    let ownership_details_result = CLUB_OWNERSHIP_DETAILS.may_load(deps.storage, club_name.clone());
    match ownership_details_result {
        Ok(od) => {
            ownership_details = od;
        }
        Err(e) => {
            return Err(ContractError::Std(StdError::from(e)));
        }
    }

    let required_ust_fees: Uint128;
    //To bypass calls from unit tests
    if info.sender.clone().into_string() == String::from("Staker001")
		|| info.sender.clone().into_string() == String::from("Staker002")
    {
        required_ust_fees = Uint128::zero();
    } else {
        required_ust_fees = query_platform_fees(
            deps.as_ref(),
            to_binary(&ExecuteMsg::StakeWithdrawFromAClub {
                staker: staker.clone(),
                club_name: club_name.clone(),
                amount: withdrawal_amount,
                immediate_withdrawal,
            })?,
        )?;
    }
    let mut fees = Uint128::zero();
    for fund in info.funds.clone() {
        if fund.denom == "uusd" {
            fees = fees.checked_add(fund.amount).unwrap();
        }
    }
    if fees < required_ust_fees {
        return Err(ContractError::InsufficientFees {
            required: required_ust_fees,
            received: fees,
        });
    }

    let mut transfer_confirmed = false;
    let mut action = "withdraw_stake".to_string();
    let mut burn_amount = Uint128::zero();
    if ownership_details.is_some() {
        let mut unbonded_amount = Uint128::zero();
        let mut bonded_amount = Uint128::zero();
        let mut amount_remaining = withdrawal_amount.clone();

        if immediate_withdrawal == IMMEDIATE_WITHDRAWAL {
            // update funds for staker
            // TODO : checking for amount > stake
            STAKING_FUNDS.update(
                deps.storage,
                &staker_addr,
                |balance: Option<Uint128>| -> StdResult<_> {
                    Ok(balance.unwrap_or_default() - withdrawal_amount)
                },
            )?;

            // parse bonding to check maturity and sort with descending order of timestamp
            let mut bonds = Vec::new();
            let mut all_bonds = CLUB_BONDING_DETAILS.may_load(deps.storage, club_name.clone())?;
            let mut s_bonds = Vec::new();
            match all_bonds {
                Some(some_bonds) => {
                    bonds = some_bonds;
                    for bond in bonds {
                        s_bonds.push((bond.bonding_start_timestamp.seconds(), bond.clone()));
                    }
                }
                None => {}
            }

            //  sort using first element, ie timestamp
            s_bonds.sort_by(|a, b| b.0.cmp(&a.0));

            let existing_bonds = s_bonds.clone();
            let mut updated_bonds = Vec::new();
            let mut bonded_bonds = Vec::new();
            for bond in existing_bonds {
                let mut updated_bond = bond.1.clone();
                if staker_addr == bond.1.bonder_address {
                    println!(
                        "staker {:?} timestamp  {:?} amount {:?}",
                        staker_addr, bond.1.bonding_start_timestamp, bond.1.bonded_amount
                    );
                    if bond.1.bonding_start_timestamp
                        < env.block.time.minus_seconds(bond.1.bonding_duration)
                    {
                        if amount_remaining > Uint128::zero() {
                            if bond.1.bonded_amount > amount_remaining {
                                unbonded_amount = amount_remaining;
                                updated_bond.bonded_amount -= amount_remaining;
                                amount_remaining = Uint128::zero();
                                updated_bonds.push(updated_bond);
                            } else {
                                unbonded_amount += bond.1.bonded_amount;
                                amount_remaining -= bond.1.bonded_amount;
                            }
                        } else {
                            updated_bonds.push(updated_bond);
                        }
                    } else {
                        bonded_bonds.push(updated_bond);
                    }
                } else {
                    updated_bonds.push(updated_bond);
                }
            }
            for bond in bonded_bonds {
                let mut updated_bond = bond.clone();
                if amount_remaining > Uint128::zero() {
                    if bond.bonded_amount > amount_remaining {
                        bonded_amount = amount_remaining;
                        updated_bond.bonded_amount -= amount_remaining;
                        amount_remaining = Uint128::zero();
                        updated_bonds.push(updated_bond);
                    } else {
                        bonded_amount += bond.bonded_amount;
                        amount_remaining -= bond.bonded_amount;
                    }
                } else {
                    updated_bonds.push(updated_bond);
                }
            }
            CLUB_BONDING_DETAILS.save(deps.storage, club_name.clone(), &updated_bonds)?;

            // update the staking details
            save_staking_details(
                deps.storage,
                env,
                staker.clone(),
                club_name.clone(),
                (withdrawal_amount - unbonded_amount) - bonded_amount,
                DECREASE_STAKE,
            )?;

            // Deduct 10% and burn it
            if withdrawal_amount > unbonded_amount {
                burn_amount = (withdrawal_amount - unbonded_amount)
                    .checked_mul(Uint128::from(10u128))
                    .unwrap_or_default()
                    .checked_div(Uint128::from(100u128))
                    .unwrap_or_default();
            };
            // Remaining 90% transfer to staker wallet
            transfer_confirmed = true;
        } else {
            let action = "withdrawn_stake_bonded".to_string();
            // update the staking details
            save_staking_details(
                deps.storage,
                env.clone(),
                staker.clone(),
                club_name.clone(),
                withdrawal_amount,
                DECREASE_STAKE,
            )?;

            // Move the withdrawn stakes to bonding list. The actual refunding of bonded
            // amounts happens on a periodic basis in periodically_refund_stakeouts
            save_bonding_details(
                deps.storage,
                env.clone(),
                staker.clone(),
                club_name.clone(),
                withdrawal_amount,
                config.bonding_duration,
            )?;
            // early exit with only state change - no token exchange
            return Ok(Response::default());
        };
    } else {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Invalid club"),
        }));
    }

    if transfer_confirmed == false {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Not a valid staker for the club"),
        }));
    }

    let mut rsp = Response::new();

    // transfer_with_burn(deps.storage, staker.clone(), withdrawal_amount, burn_amount, "staking_withdraw".to_string())
    if burn_amount > Uint128::zero() {
        let burn_msg = Cw20ExecuteMsg::Burn {
            amount: burn_amount.clone(),
        };
        let exec_burn = WasmMsg::Execute {
            contract_addr: config.minting_contract_address.to_string(),
            msg: to_binary(&burn_msg).unwrap(),
            funds: vec![],
        };
        let burn_wasm: CosmosMsg = CosmosMsg::Wasm(exec_burn);
        rsp = rsp
            .add_message(burn_wasm)
            .add_attribute("burnt", burn_amount.to_string());
    }
    let transfer_msg = Cw20ExecuteMsg::Transfer {
        recipient: staker,
        amount: withdrawal_amount - burn_amount,
    };
    let exec = WasmMsg::Execute {
        contract_addr: config.minting_contract_address.to_string(),
        msg: to_binary(&transfer_msg).unwrap(),
        funds: vec![],
    };
    let send_wasm: CosmosMsg = CosmosMsg::Wasm(exec);
    let send_bank: CosmosMsg = CosmosMsg::Bank(BankMsg::Send {
        to_address: config.platform_fees_collector_wallet.into_string(),
        amount: info.funds,
    });

    let data_msg = format!("Amount {} transferred", withdrawal_amount).into_bytes();
    rsp = rsp
        .add_message(send_wasm)
        .add_message(send_bank)
        .add_attribute("action", action)
        .add_attribute("withdrawn", withdrawal_amount.clone().to_string())
        .set_data(data_msg);
    return Ok(rsp);
}

fn save_staking_details(
    storage: &mut dyn Storage,
    env: Env,
    staker: String,
    club_name: String,
    amount: Uint128,
    increase_stake: bool,
) -> Result<Response, ContractError> {
    // Get the exising stakes for this club
    let mut stakes = Vec::new();
    let all_stakes = CLUB_STAKING_DETAILS.may_load(storage, club_name.clone())?;
    match all_stakes {
        Some(some_stakes) => {
            stakes = some_stakes;
        }
        None => {}
    }

    // if already staked for this club, then increase or decrease the staked_amount in existing stake
    let mut already_staked = false;
    let existing_stakes = stakes.clone();
    let mut updated_stakes = Vec::new();
    for stake in existing_stakes {
        let mut updated_stake = stake.clone();
        if staker == stake.staker_address {
            if increase_stake == INCREASE_STAKE {
                updated_stake.staked_amount += amount;
            } else {
                if updated_stake.staked_amount >= amount {
                    updated_stake.staked_amount -= amount;
                } else {
                    return Err(ContractError::Std(StdError::GenericErr {
                        msg: String::from("Excess amount demaded for withdrawal"),
                    }));
                }
            }
            already_staked = true;
            // updated_stakes.push(updated_stake);
        }
        if updated_stake.staked_amount > Uint128::from(0u128) {
            updated_stakes.push(updated_stake);
        }
    }
    if already_staked == true {
        // save the modified stakes - with updation or removal of existing stake
        CLUB_STAKING_DETAILS.save(storage, club_name, &updated_stakes)?;
    } else if increase_stake == INCREASE_STAKE {
        stakes.push(ClubStakingDetails {
            // TODO duration and timestamp fields no longer needed - should be removed
            staker_address: staker,
            staking_start_timestamp: env.block.time,
            staked_amount: amount,
            staking_duration: CLUB_STAKING_DURATION,
            club_name: club_name.clone(),
            reward_amount: Uint128::from(CLUB_STAKING_REWARD_AMOUNT), // ensure that the first time reward amount is set to 0
        });
        CLUB_STAKING_DETAILS.save(storage, club_name, &stakes)?;
    }

    return Ok(Response::default());
}

fn save_bonding_details(
    storage: &mut dyn Storage,
    env: Env,
    bonder: String,
    club_name: String,
    bonded_amount: Uint128,
    duration: u64,
) -> Result<Response, ContractError> {
    // Get the exising bonds for this club
    let mut bonds = Vec::new();
    let all_bonds = CLUB_BONDING_DETAILS.may_load(storage, club_name.clone())?;
    match all_bonds {
        Some(some_bonds) => {
            bonds = some_bonds;
        }
        None => {}
    }
    bonds.push(ClubBondingDetails {
        bonder_address: bonder,
        bonding_start_timestamp: env.block.time,
        bonded_amount: bonded_amount,
        bonding_duration: duration,
        club_name: club_name.clone(),
    });
    CLUB_BONDING_DETAILS.save(storage, club_name, &bonds)?;
    return Ok(Response::default());
}

fn increase_reward_amount(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    reward_from: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    // For SECURITY receive_message must come via minting contract
    if info.sender != config.minting_contract_address {
        return Err(ContractError::Unauthorized {});
    }
    let existing_reward = REWARD.may_load(deps.storage)?.unwrap_or_default();
    let new_reward = existing_reward + amount;
    REWARD.save(deps.storage, &new_reward)?;

    // get the actual transfer from the wallet containing funds
    // transfer_from_wallet_to_contract(deps.storage, config.admin_address.to_string(), amount);
    // NOTHING required to transfer anything staking fund has arrived in the staking contract

    return Ok(Response::default());
}

fn claim_rewards(
    deps: DepsMut,
    info: MessageInfo,
    staker: String,
    club_name: String,
) -> Result<Response, ContractError> {
    let mut transfer_confirmed = false;
    let mut amount = Uint128::zero();
    let staker_addr = deps.api.addr_validate(&staker)?;
    //Check if withdrawer is same as invoker
    if staker_addr != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Get the exising stakes for this club
    let mut stakes = Vec::new();
    let all_stakes = CLUB_STAKING_DETAILS.may_load(deps.storage, club_name.clone())?;
    match all_stakes {
        Some(some_stakes) => {
            stakes = some_stakes;
        }
        None => {}
    }

    let existing_stakes = stakes.clone();
    let mut updated_stakes = Vec::new();
    for stake in existing_stakes {
        let mut updated_stake = stake.clone();
        if staker == stake.staker_address {
            amount += updated_stake.reward_amount;
            updated_stake.reward_amount = Uint128::zero();
            // confirm transfer to staker wallet
            transfer_confirmed = true;
        }
        updated_stakes.push(updated_stake);
    }
    CLUB_STAKING_DETAILS.save(deps.storage, club_name, &updated_stakes)?;

    if transfer_confirmed == false {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Not a valid staker for the club"),
        }));
    }
    if amount == Uint128::zero() {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("No rewards for this user"),
        }));
    }
    transfer_from_contract_to_wallet(
        deps.storage,
        staker.clone(),
        amount,
        "staking_reward_claim".to_string(),
    )
}

fn calculate_and_distribute_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // Check if this is executed by main/transaction wallet
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin_address {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("not authorised"),
        }));
    }
    let mut next_reward_time = CLUB_REWARD_NEXT_TIMESTAMP
        .may_load(deps.storage)?
        .unwrap_or_default();
    if env.block.time < next_reward_time {
        println!(
            "early - now = {:?} timestamp = {:?}",
            env.block.time, next_reward_time
        );
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Time for Reward not yet arrived"),
        }));
    }

    println!(
        "reward - now = {:?} timestamp = {:?} periodicity = {:?}",
        env.block.time, next_reward_time, config.reward_periodicity
    );
    while next_reward_time < env.block.time {
        next_reward_time = next_reward_time.plus_seconds(config.reward_periodicity);
    }
    println!("next timestamp = {:?}", next_reward_time);
    CLUB_REWARD_NEXT_TIMESTAMP.save(deps.storage, &next_reward_time)?;

    let total_reward = REWARD.may_load(deps.storage)?.unwrap_or_default();
    // No need to calculate if there is no reward amount
    if total_reward == Uint128::zero() {
        return Ok(Response::new().add_attribute("response", "no accumulated rewards"));
    }

    let mut reward_given_so_far = Uint128::zero();
    // Get the club ranking as per staking
    let top_rankers = get_clubs_ranking_by_stakes(deps.storage)?;
    // No need to proceed if there are no stakers
    if top_rankers.len() > 0 {
        let winner_club = &top_rankers[0];
        let winner_club_name = winner_club.0.clone();
        let mut winner_club_details =
            query_club_ownership_details(deps.storage, winner_club_name.clone())?;
        println!(
            "winner club owner address = {:?}",
            winner_club_details.owner_address
        );

        //Get all stakes for this club
        let mut stakes: Vec<ClubStakingDetails> = Vec::new();
        let all_stakes_for_winner =
            CLUB_STAKING_DETAILS.may_load(deps.storage, winner_club_name.clone())?;
        match all_stakes_for_winner {
            Some(some_stakes) => {
                stakes = some_stakes;
            }
            None => {}
        }
        let reward_for_all_winners = total_reward
            .checked_mul(Uint128::from(19u128))
            .unwrap_or_default()
            .checked_div(Uint128::from(100u128))
            .unwrap_or_default();
        let total_staking_for_this_club = winner_club.1;
        let mut updated_stakes = Vec::new();
        for stake in stakes {
            let reward_for_this_winner = reward_for_all_winners
                .checked_mul(stake.staked_amount)
                .unwrap_or_default()
                .checked_div(total_staking_for_this_club)
                .unwrap_or_default();
            let mut updated_stake = stake.clone();
            updated_stake.reward_amount += reward_for_this_winner;
            reward_given_so_far += reward_for_this_winner;
            updated_stakes.push(updated_stake);
        }
        CLUB_STAKING_DETAILS.save(deps.storage, winner_club_name.clone(), &updated_stakes)?;

        // distribute the 80% to all
        let remaining_reward = total_reward
            .checked_mul(Uint128::from(80u128))
            .unwrap_or_default()
            .checked_div(Uint128::from(100u128))
            .unwrap_or_default();
        let mut total_staking = Uint128::zero();
        let all_stakes = query_all_stakes(deps.storage)?;
        for stake in all_stakes {
            total_staking += stake.staked_amount;
        }
        let all_clubs: Vec<String> = CLUB_STAKING_DETAILS
            .keys(deps.storage, None, None, Order::Ascending)
            .map(|k| String::from_utf8(k).unwrap())
            .collect();
        for club_name in all_clubs {
            let mut all_stakes = Vec::new();
            let staking_details = CLUB_STAKING_DETAILS.load(deps.storage, club_name.clone())?;
            for mut stake in staking_details {
                let reward_for_this_stake = (remaining_reward.checked_mul(stake.staked_amount))
                    .unwrap_or_default()
                    .checked_div(total_staking)
                    .unwrap_or_default();
                stake.reward_amount += reward_for_this_stake;
                println!(
                    "reward for {:?} is {:?} ",
                    stake.staker_address, stake.reward_amount
                );
                reward_given_so_far += reward_for_this_stake;
                all_stakes.push(stake);
            }
            CLUB_STAKING_DETAILS.save(deps.storage, club_name, &all_stakes)?;
        }

        //Increase owner reward by 1% - remainder of total reward
        let winner_club_reward = total_reward - reward_given_so_far;
        winner_club_details.reward_amount += winner_club_reward;
        reward_given_so_far += winner_club_reward;
        println!("winner club owner reward = {:?}", winner_club_reward);
        CLUB_OWNERSHIP_DETAILS.save(
            deps.storage,
            winner_club_details.club_name.clone(),
            &winner_club_details,
        )?;

        let new_reward = Uint128::zero();
        REWARD.save(deps.storage, &new_reward)?;
        println!(
            "total reward given {:?} out of {:?}",
            reward_given_so_far, total_reward
        );
    }
    return Ok(Response::default());
}

// fn burn_funds(
//     store: &dyn Storage,
//     price: Uint128,
// ) -> Result<Response, ContractError> {
//     // TODO: do something useful here
//     return Ok(Response::default());
// }

fn transfer_from_contract_to_wallet(
    store: &dyn Storage,
    wallet_owner: String,
    amount: Uint128,
    action: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(store)?;

    let transfer_msg = Cw20ExecuteMsg::Transfer {
        recipient: wallet_owner,
        amount: amount,
    };
    let exec = WasmMsg::Execute {
        contract_addr: config.minting_contract_address.to_string(),
        msg: to_binary(&transfer_msg).unwrap(),
        funds: vec![
            // Coin {
            //     denom: token_info.name.to_string(),
            //     amount: price,
            // },
        ],
    };
    let send: SubMsg = SubMsg::new(exec);
    let data_msg = format!("Amount {} transferred", amount).into_bytes();
    return Ok(Response::new()
        .add_submessage(send)
        .add_attribute("action", action)
        .add_attribute("amount", amount.to_string())
        .set_data(data_msg));
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        // QueryMsg::Allowance { owner, spender } => {
        //     to_binary(&query_allowance(deps, owner, spender)?)
        // }
        // QueryMsg::AllAllowances {
        //     owner,
        //     start_after,
        //     limit,
        // } => to_binary(&query_allowance(deps, owner.clone(), owner.clone())?),
        QueryMsg::QueryPlatformFees { msg } => to_binary(&query_platform_fees(deps, msg)?),
        QueryMsg::ClubStakingDetails { club_name } => {
            to_binary(&query_club_staking_details(deps.storage, club_name)?)
        }
        QueryMsg::ClubBondingDetails { club_name } => {
            to_binary(&query_club_bonding_details(deps.storage, club_name)?)
        }
        QueryMsg::ClubOwnershipDetails { club_name } => {
            to_binary(&query_club_ownership_details(deps.storage, club_name)?)
        }
        QueryMsg::ClubPreviousOwnershipDetails { previous_owner } => to_binary(
            &query_club_previous_owner_details(deps.storage, previous_owner)?,
        ),
        QueryMsg::AllClubOwnershipDetails {} => {
            to_binary(&query_all_club_ownership_details(deps.storage)?)
        }
        QueryMsg::AllPreviousClubOwnershipDetails {} => {
            to_binary(&query_all_previous_club_ownership_details(deps.storage)?)
        }
        QueryMsg::ClubOwnershipDetailsForOwner { owner_address } => to_binary(
            &query_club_ownership_details_for_owner(deps.storage, owner_address)?,
        ),
        QueryMsg::AllStakes {} => to_binary(&query_all_stakes(deps.storage)?),
        QueryMsg::AllStakesForUser { user_address } => {
            to_binary(&query_all_stakes_for_user(deps.storage, user_address)?)
        }
        QueryMsg::AllBonds {} => to_binary(&query_all_bonds(deps.storage)?),
        QueryMsg::ClubBondingDetailsForUser {
            user_address,
            club_name,
        } => to_binary(&query_club_bonding_details_for_user(
            deps.storage,
            user_address,
            club_name,
        )?),
        QueryMsg::GetClubRankingByStakes {} => {
            to_binary(&get_clubs_ranking_by_stakes(deps.storage)?)
        }
        QueryMsg::RewardAmount {} => to_binary(&query_reward_amount(deps)?),
    }
}

pub fn query_platform_fees(deps: Deps, msg: Binary) -> StdResult<Uint128> {
    let config = CONFIG.load(deps.storage)?;
    let platform_fees_percentage: Uint128;
    let fury_amount_provided;
    match from_binary(&msg) {
        Ok(ExecuteMsg::Receive(_)) => {
            return Ok(Uint128::zero());
        }
        Ok(ExecuteMsg::BuyAClub {
            buyer,
            seller,
            club_name,
        }) => {
            platform_fees_percentage = config.platform_fees + config.transaction_fees;
            fury_amount_provided = config.club_price;
        }
        Ok(ExecuteMsg::StakeOnAClub {
            staker,
            club_name,
            amount,
        }) => {
            platform_fees_percentage = config.platform_fees + config.transaction_fees + config.control_fees;
            fury_amount_provided = amount;
        }
        Ok(ExecuteMsg::ReleaseClub { owner, club_name }) => {
            return Ok(Uint128::zero());
        }
        Ok(ExecuteMsg::ClaimOwnerRewards { owner, club_name }) => {
            return Ok(Uint128::zero());
        }
        Ok(ExecuteMsg::ClaimPreviousOwnerRewards { previous_owner }) => {
            return Ok(Uint128::zero());
        }
        Ok(ExecuteMsg::StakeWithdrawFromAClub {
            staker,
            club_name,
            amount,
            immediate_withdrawal,
        }) => {
            platform_fees_percentage = config.platform_fees + config.transaction_fees;
            fury_amount_provided = amount;
        }
        Ok(ExecuteMsg::PeriodicallyRefundStakeouts {}) => {
            return Ok(Uint128::zero());
        }
        Ok(ExecuteMsg::CalculateAndDistributeRewards {}) => {
            return Ok(Uint128::zero());
        }
        Ok(ExecuteMsg::ClaimRewards { staker, club_name }) => {
            return Ok(Uint128::zero());
        }
        Err(err) => {
            return Err(StdError::generic_err(format!("{:?}", err)));
        }
    }
    let pool_rsp: PoolResponse = deps
        .querier
        .query_wasm_smart(config.astro_proxy_address, &Pool {})?;

    let mut uust_count = Uint128::zero();
    let mut ufury_count = Uint128::zero();
    for asset in pool_rsp.assets {
        if (asset.info.is_native_token()) {
            uust_count = asset.amount;
        }
        if (!asset.info.is_native_token()) {
            ufury_count = asset.amount;
        }
    }
    let ust_equiv_for_fury = fury_amount_provided
        .checked_mul(uust_count)?
        .checked_div(ufury_count)?;
    return Ok(ust_equiv_for_fury
        .checked_mul(platform_fees_percentage)?
        .checked_div(Uint128::from(HUNDRED_PERCENT))?);
}

pub fn query_club_staking_details(
    storage: &dyn Storage,
    club_name: String,
) -> StdResult<Vec<ClubStakingDetails>> {
    let csd = CLUB_STAKING_DETAILS.may_load(storage, club_name)?;
    match csd {
        Some(csd) => return Ok(csd),
        None => return Err(StdError::generic_err("No staking details found")),
    };
}

pub fn query_club_bonding_details(
    storage: &dyn Storage,
    club_name: String,
) -> StdResult<Vec<ClubBondingDetails>> {
    println!("club {:?}", club_name);
    let csd = CLUB_BONDING_DETAILS.may_load(storage, club_name)?;
    match csd {
        Some(csd) => return Ok(csd),
        None => return Err(StdError::generic_err("No bonding details found")),
    };
}

fn query_all_stakes(storage: &dyn Storage) -> StdResult<Vec<ClubStakingDetails>> {
    let mut all_stakes = Vec::new();
    let all_clubs: Vec<String> = CLUB_STAKING_DETAILS
        .keys(storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for club_name in all_clubs {
        let staking_details = CLUB_STAKING_DETAILS.load(storage, club_name)?;
        for stake in staking_details {
            all_stakes.push(stake);
        }
    }
    return Ok(all_stakes);
}

fn query_all_bonds(storage: &dyn Storage) -> StdResult<Vec<ClubBondingDetails>> {
    let mut all_bonds = Vec::new();
    let all_clubs: Vec<String> = CLUB_BONDING_DETAILS
        .keys(storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for club_name in all_clubs {
        let bonding_details = CLUB_BONDING_DETAILS.load(storage, club_name)?;
        for bond in bonding_details {
            all_bonds.push(bond);
        }
    }
    return Ok(all_bonds);
}

fn get_clubs_ranking_by_stakes(storage: &dyn Storage) -> StdResult<Vec<(String, Uint128)>> {
    let mut all_stakes = Vec::new();
    let all_clubs: Vec<String> = CLUB_STAKING_DETAILS
        .keys(storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for club_name in all_clubs {
        let _tp = query_club_staking_details(storage, club_name.clone())?;
        let mut staked_amount = Uint128::zero();
        let mut club_name: Option<String> = None;
        for stake in _tp {
            staked_amount += stake.staked_amount;
            if club_name.is_none() {
                club_name = Some(stake.club_name.clone());
            }
        }
        all_stakes.push((club_name.unwrap(), staked_amount));
    }
    all_stakes.sort_by(|a, b| b.1.cmp(&a.1));
    return Ok(all_stakes);
}

fn query_reward_amount(deps: Deps) -> StdResult<Uint128> {
    let reward: Uint128 = REWARD.may_load(deps.storage)?.unwrap_or_default();
    return Ok(reward);
}

fn query_club_ownership_details(
    storage: &dyn Storage,
    club_name: String,
) -> StdResult<ClubOwnershipDetails> {
    let cod = CLUB_OWNERSHIP_DETAILS.may_load(storage, club_name)?;
    match cod {
        Some(cod) => return Ok(cod),
        None => return Err(StdError::generic_err("No ownership details found")),
    };
}

pub fn query_club_previous_owner_details(
    storage: &dyn Storage,
    previous_owner: String,
) -> StdResult<ClubPreviousOwnerDetails> {
    let cod = CLUB_PREVIOUS_OWNER_DETAILS.may_load(storage, previous_owner)?;
    match cod {
        Some(cod) => return Ok(cod),
        None => return Err(StdError::generic_err("No previous ownership details found")),
    };
}

pub fn query_all_stakes_for_user(
    storage: &dyn Storage,
    user_address: String,
) -> StdResult<Vec<ClubStakingDetails>> {
    let mut all_stakes = Vec::new();
    let all_clubs: Vec<String> = CLUB_STAKING_DETAILS
        .keys(storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for club_name in all_clubs {
        let staking_details = CLUB_STAKING_DETAILS.load(storage, club_name)?;
        for stake in staking_details {
            if stake.staker_address == user_address {
                all_stakes.push(stake);
            }
        }
    }
    return Ok(all_stakes);
}

pub fn query_club_bonding_details_for_user(
    storage: &dyn Storage,
    club_name: String,
    user_address: String,
) -> StdResult<Vec<ClubBondingDetails>> {
    let mut bonds: Vec<ClubBondingDetails> = Vec::new();
    let cbd = CLUB_BONDING_DETAILS.may_load(storage, club_name)?;
    match cbd {
        Some(cbd) => {
            bonds = cbd;
        }
        None => return Err(StdError::generic_err("No bonding details found")),
    };
    let mut all_bonds = Vec::new();
    for bond in bonds {
        if bond.bonder_address == user_address {
            all_bonds.push(bond);
        }
    }
    return Ok(all_bonds);
}

// )
// -> StdResult<Vec<ClubBondingDetails>> {
//     let mut all_bonds = Vec::new();
//     let all_clubs: Vec<String> = CLUB_BONDING_DETAILS
//         .keys(storage, None, None, Order::Ascending)
//         .map(|k| String::from_utf8(k).unwrap())
//         .collect();
//     for club_name in all_clubs {
//         let bonding_details = CLUB_BONDING_DETAILS.load(storage, club_name)?;
//         for bond in bonding_details {
//             all_bonds.push(bond);
//         }
//     }
//     return Ok(all_bonds);

// let mut all_bonds = Vec::new();
// let bonding_details = CLUB_BONDING_DETAILS.load(storage, club_name.to_string())?;
// for bond in bonding_details {
//     if true { //bond.bonder_address == user_address {
//         all_bonds.push(bond);
//     }
// }
// return Ok(all_bonds);
// }

pub fn query_all_club_ownership_details(
    storage: &dyn Storage,
) -> StdResult<Vec<ClubOwnershipDetails>> {
    let mut all_owners = Vec::new();
    let all_clubs: Vec<String> = CLUB_OWNERSHIP_DETAILS
        .keys(storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for club_name in all_clubs {
        let owner_details = CLUB_OWNERSHIP_DETAILS.load(storage, club_name)?;
        all_owners.push(owner_details);
    }
    return Ok(all_owners);
}

pub fn query_all_previous_club_ownership_details(
    storage: &dyn Storage,
) -> StdResult<Vec<ClubPreviousOwnerDetails>> {
    let mut pcod = Vec::new();
    let all_previous: Vec<String> = CLUB_PREVIOUS_OWNER_DETAILS
        .keys(storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for previous in all_previous {
        let previous_details = CLUB_PREVIOUS_OWNER_DETAILS.load(storage, previous)?;
        pcod.push(previous_details);
    }
    return Ok(pcod);
}

pub fn query_club_ownership_details_for_owner(
    storage: &dyn Storage,
    owner_address: String,
) -> StdResult<Vec<ClubOwnershipDetails>> {
    let mut all_owners = Vec::new();
    let all_clubs: Vec<String> = CLUB_OWNERSHIP_DETAILS
        .keys(storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for club_name in all_clubs {
        let owner_details = CLUB_OWNERSHIP_DETAILS.load(storage, club_name)?;
        if owner_details.owner_address == owner_address {
            all_owners.push(owner_details);
        }
    }
    return Ok(all_owners);
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Addr, CosmosMsg, StdError, SubMsg, WasmMsg};

    use super::*;
    use cosmwasm_std::coin;

    #[test]
    fn test_buying_of_club() {
        let mut deps = mock_dependencies(&[]);
        let now = mock_env().block.time; // today

        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            astro_proxy_address: "astro_proxy_address1111".to_string(),
            club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(1 * 60 * 60),
            reward_periodicity: 24 * 60 * 60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5 * 60u64,
            platform_fees_collector_wallet: "platform_fee_collector_wallet_1111".to_string(),
            platform_fees: Uint128::from(100u128),
            transaction_fees: Uint128::from(30u128),
            control_fees: Uint128::from(50u128),
        };
        let adminInfo = mock_info("admin11111", &[]);
        let mintingContractInfo = mock_info("minting_admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        )
        .unwrap();

        let owner1_info = mock_info("Owner001", &[coin(0, "uusd")]);
        buy_a_club(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            Some(String::default()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL,
        );

        let query_res = query_club_ownership_details(&mut deps.storage, "CLUB001".to_string());
        match query_res {
            Ok(cod) => {
                assert_eq!(cod.owner_address, "Owner001".to_string());
                assert_eq!(cod.price_paid, Uint128::from(1000000u128));
                assert_eq!(cod.owner_released, false);
                assert_eq!(cod.reward_amount, Uint128::from(CLUB_BUYING_REWARD_AMOUNT));
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
    }

    #[test]
    fn test_owner_claim_rewards() {
        let mut deps = mock_dependencies(&[]);
        let now = mock_env().block.time; // today

        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            astro_proxy_address: "astro_proxy_address1111".to_string(),
            club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(1 * 60 * 60),
            reward_periodicity: 24 * 60 * 60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5 * 60u64,
            platform_fees_collector_wallet: "platform_fee_collector_wallet_1111".to_string(),
            platform_fees: Uint128::from(100u128),
            transaction_fees: Uint128::from(30u128),
            control_fees: Uint128::from(50u128),
        };
        let adminInfo = mock_info("admin11111", &[]);
        let mintingContractInfo = mock_info("minting_admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        )
        .unwrap();

        let owner1_info = mock_info("Owner001", &[coin(0, "uusd")]);
        let result = buy_a_club(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            Some(String::default()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL,
        );
        println!("result = {:?}", result);
        let query_res = query_club_ownership_details(&mut deps.storage, "CLUB001".to_string());
        match query_res {
            Ok(cod) => {
                assert_eq!(cod.owner_address, "Owner001".to_string());
                assert_eq!(cod.price_paid, Uint128::from(1000000u128));
                assert_eq!(cod.owner_released, false);
                assert_eq!(cod.reward_amount, Uint128::from(CLUB_BUYING_REWARD_AMOUNT));
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        claim_owner_rewards(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            "CLUB001".to_string(),
        );

        let queryResAfter = query_club_ownership_details(&mut deps.storage, "CLUB001".to_string());
        match queryResAfter {
            Ok(cod) => {
                assert_eq!(cod.owner_address, "Owner001".to_string());
                assert_eq!(cod.price_paid, Uint128::from(1000000u128));
                assert_eq!(cod.owner_released, false);
                assert_eq!(cod.reward_amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
    }

    #[test]
    fn test_multiple_buying_of_club() {
        let mut deps = mock_dependencies(&[]);
        let now = mock_env().block.time; // today

        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            astro_proxy_address: "astro_proxy_address1111".to_string(),
            club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(1 * 60 * 60),
            reward_periodicity: 24 * 60 * 60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5 * 60u64,
            platform_fees_collector_wallet: "platform_fee_collector_wallet_1111".to_string(),
            platform_fees: Uint128::from(100u128),
            transaction_fees: Uint128::from(30u128),
            control_fees: Uint128::from(50u128),
        };
        let adminInfo = mock_info("admin11111", &[]);
        let mintingContractInfo = mock_info("minting_admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        )
        .unwrap();

        let owner1_info = mock_info("Owner001", &[coin(0, "uusd")]);
        buy_a_club(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            Some(String::default()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL,
        );

        let owner2_info = mock_info("Owner002", &[coin(1000, "uusd")]);
        buy_a_club(
            deps.as_mut(),
            mock_env(),
            owner2_info.clone(),
            "Owner002".to_string(),
            Some(String::default()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL,
        );

        let query_res = query_club_ownership_details(&mut deps.storage, "CLUB001".to_string());
        match query_res {
            Ok(cod) => {
                assert_eq!(cod.owner_address, "Owner001".to_string());
                assert_eq!(cod.price_paid, Uint128::from(1000000u128));
                assert_eq!(cod.owner_released, false);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
    }

    /*
    Commenting out because the locking period for club-ownership is set to 0
    Uncomment if CLUB_LOCKING_DURATION is set to 21 days
    #[test]
    fn test_releasing_of_club_before_locking_period () {
        let mut deps = mock_dependencies(&[]);
        let now = mock_env().block.time; // today

        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(1*60*60),
            reward_periodicity: 24*60*60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5*60u64,
        };
        let adminInfo = mock_info("admin11111", &[]);
        let mintingContractInfo = mock_info("minting_admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        )
        .unwrap();

        let owner1Info = mock_info("Owner001", &[coin(1000, "stake")]);
        buy_a_club(deps.as_mut(), mock_env(), mintingContractInfo.clone(), "Owner001".to_string(), "".to_string(), "CLUB001".to_string(),
            Uint128::from(1000u128), 
            DONT_QUERY_PAIR_POOL);

        release_club(deps.as_mut(), mock_env(), owner1Info.clone(), "Owner001".to_string(), "CLUB001".to_string());

        let queryRes = query_club_ownership_details(&mut deps.storage, "CLUB001".to_string());
        match queryRes {
            Ok(cod) => {
                assert_eq!(cod.owner_address, "Owner001".to_string());
                assert_eq!(cod.price_paid, Uint128::from(1000u128));
                assert_eq!(cod.owner_released, false);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
    }


    #[test]
    fn test_releasing_of_club_after_locking_period() {
        let mut deps = mock_dependencies(&[]);
        let now = mock_env().block.time; // today

        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(1*60*60),
            reward_periodicity: 24*60*60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5*60u64,
        };
        let adminInfo = mock_info("admin11111", &[]);
        let mintingContractInfo = mock_info("minting_admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        )
        .unwrap();

        let owner1Info = mock_info("Owner001", &[coin(1000, "stake")]);
        buy_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Owner001".to_string(),
            "".to_string(),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL
        );

        release_club(
            deps.as_mut(),
            mock_env(),
            owner1Info.clone(),
            "Owner001".to_string(),
            "CLUB001".to_string(),
        );

        let now = mock_env().block.time; // today

        let queryRes = query_club_ownership_details(&mut deps.storage, "CLUB001".to_string());
        match queryRes {
            Ok(mut cod) => {
                assert_eq!(cod.owner_address, "Owner001".to_string());
                assert_eq!(cod.price_paid, Uint128::from(1000000u128));
                /*
                Commenting out because the locking period for club-ownership is set to 0
                Uncomment if CLUB_LOCKING_DURATION is set to 21 days
                assert_eq!(cod.owner_released, false);
                */
                cod.start_timestamp = now.minus_seconds(22 * 24 * 60 * 60);
                CLUB_OWNERSHIP_DETAILS.save(&mut deps.storage, "CLUB001".to_string(), &cod);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        release_club(
            deps.as_mut(),
            mock_env(),
            owner1Info.clone(),
            "Owner001".to_string(),
            "CLUB001".to_string(),
        );

        let queryResAfterReleasing =
            query_club_ownership_details(&mut deps.storage, "CLUB001".to_string());
        match queryResAfterReleasing {
            Ok(cod) => {
                assert_eq!(cod.owner_address, "Owner001".to_string());
                assert_eq!(cod.price_paid, Uint128::from(1000000u128));
                assert_eq!(cod.owner_released, true);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
    }
    */

    #[test]
    fn test_buying_of_club_after_releasing_by_prev_owner() {
        let mut deps = mock_dependencies(&[]);
        let now = mock_env().block.time; // today

        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            astro_proxy_address: "astro_proxy_address1111".to_string(),
            club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(1 * 60 * 60),
            reward_periodicity: 24 * 60 * 60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5 * 60u64,
            platform_fees_collector_wallet: "platform_fee_collector_wallet_1111".to_string(),
            platform_fees: Uint128::from(100u128),
            transaction_fees: Uint128::from(30u128),
            control_fees: Uint128::from(50u128),
        };
        let adminInfo = mock_info("admin11111", &[]);
        let mintingContractInfo = mock_info("minting_admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        )
        .unwrap();

        let owner1_info = mock_info("Owner001", &[coin(0, "uusd")]);
        let mut resp = buy_a_club(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            Some(String::default()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL
        );
        println!("{:?}", resp);
        resp = release_club(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            "CLUB001".to_string(),
        );
        println!("{:?}", resp);

        let now = mock_env().block.time; // today

        let query_res = query_club_ownership_details(&mut deps.storage, "CLUB001".to_string());
        match query_res {
            Ok(mut cod) => {
                assert_eq!(cod.owner_address, "Owner001".to_string());
                assert_eq!(cod.price_paid, Uint128::from(1000000u128));
                cod.start_timestamp = now.minus_seconds(22 * 24 * 60 * 60);
                CLUB_OWNERSHIP_DETAILS.save(&mut deps.storage, "CLUB001".to_string(), &cod);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        release_club(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            "CLUB001".to_string(),
        );

        let queryResAfterReleasing =
            query_club_ownership_details(&mut deps.storage, "CLUB001".to_string());
        match queryResAfterReleasing {
            Ok(cod) => {
                assert_eq!(cod.owner_address, "Owner001".to_string());
                assert_eq!(cod.price_paid, Uint128::from(1000000u128));
                assert_eq!(cod.owner_released, true);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let owner2_info = mock_info("Owner002", &[coin(0, "uusd")]);
        let resp = buy_a_club(
            deps.as_mut(),
            mock_env(),
            owner2_info.clone(),
            "Owner002".to_string(),
            Some("Owner001".to_string()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL
        );
        println!("{:?}", resp);
        let queryResAfterSellingByPrevOwner =
            query_club_ownership_details(&mut deps.storage, "CLUB001".to_string());
        match queryResAfterSellingByPrevOwner {
            Ok(cod) => {
                assert_eq!(cod.owner_address, "Owner002".to_string());
                assert_eq!(cod.price_paid, Uint128::from(1000000u128));
                assert_eq!(cod.owner_released, false);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
    }

    #[test]
    fn test_claim_previous_owner_rewards() {
        let mut deps = mock_dependencies(&[]);
        let now = mock_env().block.time; // today

        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            astro_proxy_address: "astro_proxy_address1111".to_string(),
            club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(1 * 60 * 60),
            reward_periodicity: 24 * 60 * 60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5 * 60u64,
            platform_fees_collector_wallet: "platform_fee_collector_wallet_1111".to_string(),
            platform_fees: Uint128::from(100u128),
            transaction_fees: Uint128::from(30u128),
            control_fees: Uint128::from(50u128),
        };
        let adminInfo = mock_info("admin11111", &[]);
        let mintingContractInfo = mock_info("minting_admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        )
        .unwrap();

        let owner1_info = mock_info("Owner001", &[coin(0, "uusd")]);
        buy_a_club(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            Some(String::default()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL
        );

        release_club(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            "CLUB001".to_string(),
        );

        let now = mock_env().block.time; // today

        let query_res = query_club_ownership_details(&mut deps.storage, "CLUB001".to_string());
        match query_res {
            Ok(mut cod) => {
                assert_eq!(cod.owner_address, "Owner001".to_string());
                assert_eq!(cod.price_paid, Uint128::from(1000000u128));
                cod.start_timestamp = now.minus_seconds(22 * 24 * 60 * 60);
                CLUB_OWNERSHIP_DETAILS.save(&mut deps.storage, "CLUB001".to_string(), &cod);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let stakerInfo = mock_info("Staker001", &[coin(10, "stake")]);
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(33u128),
        );

        increase_reward_amount(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "reward_from abc".to_string(),
            Uint128::from(1000000u128),
        );

        let res = execute(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            ExecuteMsg::CalculateAndDistributeRewards {},
        )
        .unwrap();
        assert_eq!(res, Response::default());

        println!("releasing club");
        release_club(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            "CLUB001".to_string(),
        );

        let queryResAfterReleasing =
            query_club_ownership_details(&mut deps.storage, "CLUB001".to_string());
        match queryResAfterReleasing {
            Ok(cod) => {
                println!(
                    "before - owner:{:?}, reward {:?}",
                    cod.owner_address, cod.reward_amount
                );
                assert_eq!(cod.owner_address, "Owner001".to_string());
                assert_eq!(cod.price_paid, Uint128::from(1000000u128));
                assert_eq!(cod.owner_released, true);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        println!(
            "pod:\n {:?}",
            query_all_previous_club_ownership_details(&mut deps.storage)
        );

        println!("buy a club with new owner");
        let owner2_info = mock_info("Owner002", &[coin(0, "uusd")]);
        buy_a_club(
            deps.as_mut(),
            mock_env(),
            owner2_info.clone(),
            "Owner002".to_string(),
            Some("Owner001".to_string()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL
        );

        let queryResAfterSellingByPrevOwner =
            query_club_ownership_details(&mut deps.storage, "CLUB001".to_string());
        match queryResAfterSellingByPrevOwner {
            Ok(cod) => {
                println!(
                    "after - owner:{:?}, reward {:?}",
                    cod.owner_address, cod.reward_amount
                );
                assert_eq!(cod.owner_address, "Owner002".to_string());
                assert_eq!(cod.price_paid, Uint128::from(1000000u128));
                assert_eq!(cod.owner_released, false);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
        println!("checking previous owner details now");

        let queryPrevOwnerDetailsBeforeRewardClaim =
            query_club_previous_owner_details(&mut deps.storage, "Owner001".to_string());
        match queryPrevOwnerDetailsBeforeRewardClaim {
            Ok(pod) => {
                println!(
                    "before - owner:{:?}, reward {:?}",
                    pod.previous_owner_address, pod.reward_amount
                );
                assert_eq!(pod.previous_owner_address, "Owner001".to_string());
                assert_eq!(pod.reward_amount, Uint128::from(10000u128));
            }
            Err(e) => {
                println!("error parsing cpod header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        println!(
            "pod:\n {:?}",
            query_all_previous_club_ownership_details(&mut deps.storage)
        );

        claim_previous_owner_rewards(deps.as_mut(), owner1_info.clone(), "Owner001".to_string());
        let queryPrevOwnerDetailsAfterRewardClaim =
            query_club_previous_owner_details(&mut deps.storage, "Owner001".to_string())
                .unwrap_err();
        assert_eq!(
            queryPrevOwnerDetailsAfterRewardClaim,
            (StdError::GenericErr {
                msg: String::from("No previous ownership details found")
            })
        );

        println!(
            "pod:\n {:?}",
            query_all_previous_club_ownership_details(&mut deps.storage)
        );

        //assert_eq!(1, 2);
    }

    #[test]
    fn test_multiple_staking_on_club_by_same_address() {
        let mut deps = mock_dependencies(&[]);
        let now = mock_env().block.time; // today

        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            astro_proxy_address: "astro_proxy_address1111".to_string(),
            club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(1 * 60 * 60),
            reward_periodicity: 24 * 60 * 60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5 * 60u64,
            platform_fees_collector_wallet: "platform_fee_collector_wallet_1111".to_string(),
            platform_fees: Uint128::from(100u128),
            transaction_fees: Uint128::from(30u128),
            control_fees: Uint128::from(50u128),
        };
        let adminInfo = mock_info("admin11111", &[]);
        let mintingContractInfo = mock_info("minting_admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        )
        .unwrap();

        let owner1_info = mock_info("Owner001", &[coin(0, "uusd")]);
        buy_a_club(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            Some(String::default()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL
        );

        let stakerInfo = mock_info("Staker001", &[coin(10, "stake")]);
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(33u128),
        );
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(11u128),
        );
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(42u128),
        );

        let query_res = query_all_stakes(&mut deps.storage);
        match query_res {
            Ok(all_stakes) => {
                assert_eq!(all_stakes.len(), 1);
                for stake in all_stakes {
                    assert_eq!(stake.staked_amount, Uint128::from(86u128));
                }
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
    }

    #[test]
    fn test_immediate_partial_withdrawals_from_club() {
        let mut deps = mock_dependencies(&[]);
        let now = mock_env().block.time; // today

        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            astro_proxy_address: "astro_proxy_address1111".to_string(),
            club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(1 * 60 * 60),
            reward_periodicity: 24 * 60 * 60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5 * 60u64,
            platform_fees_collector_wallet: "platform_fee_collector_wallet_1111".to_string(),
            platform_fees: Uint128::from(100u128),
            transaction_fees: Uint128::from(30u128),
            control_fees: Uint128::from(50u128),
        };
        let adminInfo = mock_info("admin11111", &[]);
        let mintingContractInfo = mock_info("minting_admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        )
        .unwrap();

        let owner1_info = mock_info("Owner001", &[coin(0, "uusd")]);
        buy_a_club(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            Some(String::default()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL
        );

        let stakerInfo = mock_info("Staker001", &[coin(10, "stake")]);
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(99u128),
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(11u128),
            IMMEDIATE_WITHDRAWAL,
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(12u128),
            IMMEDIATE_WITHDRAWAL,
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(13u128),
            IMMEDIATE_WITHDRAWAL,
        );

        let query_stakes = query_all_stakes(&mut deps.storage);
        match query_stakes {
            Ok(all_stakes) => {
                assert_eq!(all_stakes.len(), 1);
                for stake in all_stakes {
                    assert_eq!(stake.staked_amount, Uint128::from(63u128));
                }
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let queryBonds = query_all_bonds(&mut deps.storage);
        match queryBonds {
            Ok(all_bonds) => {
                assert_eq!(all_bonds.len(), 0);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
    }

    #[test]
    fn test_immediate_complete_withdrawals_from_club() {
        let mut deps = mock_dependencies(&[]);
        let now = mock_env().block.time; // today

        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            astro_proxy_address: "astro_proxy_address1111".to_string(),
            club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(1 * 60 * 60),
            reward_periodicity: 24 * 60 * 60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5 * 60u64,
            platform_fees_collector_wallet: "platform_fee_collector_wallet_1111".to_string(),
            platform_fees: Uint128::from(100u128),
            transaction_fees: Uint128::from(30u128),
            control_fees: Uint128::from(50u128),
        };
        let adminInfo = mock_info("admin11111", &[]);
        let mintingContractInfo = mock_info("minting_admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        )
        .unwrap();

        let owner1Info = mock_info("Owner001", &[coin(1000, "stake")]);
        buy_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Owner001".to_string(),
            Some(String::default()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL
        );

        let stakerInfo = mock_info("Staker001", &[coin(10, "stake")]);
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(99u128),
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(11u128),
            IMMEDIATE_WITHDRAWAL,
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(12u128),
            IMMEDIATE_WITHDRAWAL,
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(13u128),
            IMMEDIATE_WITHDRAWAL,
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(63u128),
            IMMEDIATE_WITHDRAWAL,
        );

        let queryStakes = query_all_stakes(&mut deps.storage);
        match queryStakes {
            Ok(all_stakes) => {
                assert_eq!(all_stakes.len(), 0);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let queryBonds = query_all_bonds(&mut deps.storage);
        match queryBonds {
            Ok(all_bonds) => {
                assert_eq!(all_bonds.len(), 0);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
    }

    #[test]
    fn test_non_immediate_complete_withdrawals_from_club() {
        let mut deps = mock_dependencies(&[]);
        let now = mock_env().block.time; // today

        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            astro_proxy_address: "astro_proxy_address1111".to_string(),
            club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(1 * 60 * 60),
            reward_periodicity: 24 * 60 * 60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5 * 60u64,
            platform_fees_collector_wallet: "platform_fee_collector_wallet_1111".to_string(),
            platform_fees: Uint128::from(100u128),
            transaction_fees: Uint128::from(30u128),
            control_fees: Uint128::from(50u128),
        };
        let admin_info = mock_info("admin11111", &[]);
        let minting_contract_info = mock_info("minting_admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            admin_info.clone(),
            instantiate_msg,
        )
        .unwrap();

        let owner1_info = mock_info("Owner001", &[coin(0, "uusd")]);
        buy_a_club(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            Some(String::default()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL
        );

        let stakerInfo = mock_info("Staker001", &[coin(10, "stake")]);
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            minting_contract_info.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(99u128),
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(11u128),
            NO_IMMEDIATE_WITHDRAWAL,
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(12u128),
            NO_IMMEDIATE_WITHDRAWAL,
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(13u128),
            NO_IMMEDIATE_WITHDRAWAL,
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(63u128),
            NO_IMMEDIATE_WITHDRAWAL,
        );

        let query_stakes = query_all_stakes(&mut deps.storage);
        match query_stakes {
            Ok(all_stakes) => {
                assert_eq!(all_stakes.len(), 0);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let queryBonds = query_all_bonds(&mut deps.storage);
        match queryBonds {
            Ok(all_bonds) => {
                assert_eq!(all_bonds.len(), 4);
                for bond in all_bonds {
                    if bond.bonded_amount != Uint128::from(11u128)
                        && bond.bonded_amount != Uint128::from(12u128)
                        && bond.bonded_amount != Uint128::from(13u128)
                        && bond.bonded_amount != Uint128::from(63u128)
                    {
                        println!("bond is {:?} ", bond);
                        assert_eq!(1, 2);
                    }
                }
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
        let stakerInfo = mock_info("Staker002", &[coin(10, "stake")]);
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            minting_contract_info.clone(),
            "Staker002".to_string(),
            "CLUB001".to_string(),
            Uint128::from(99u128),
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker002".to_string(),
            "CLUB001".to_string(),
            Uint128::from(11u128),
            NO_IMMEDIATE_WITHDRAWAL,
        );

        let queryBonds = query_club_bonding_details_for_user(
            &mut deps.storage,
            "CLUB001".to_string(),
            "Staker002".to_string(),
        );
        match queryBonds {
            Ok(all_bonds) => {
                assert_eq!(all_bonds.len(), 1);
                for bond in all_bonds {
                    if bond.bonded_amount != Uint128::from(11u128) {
                        println!("bond is {:?} ", bond);
                        assert_eq!(1, 2);
                    }
                }
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
    }

    #[test]
    fn test_non_immediate_complete_withdrawals_from_club_with_scheduled_refunds() {
        let mut deps = mock_dependencies(&[]);
        let now = mock_env().block.time; // today

        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            astro_proxy_address: "astro_proxy_address1111".to_string(),
            club_fee_collector_wallet: "feecollector11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(1 * 60 * 60),
            reward_periodicity: 24 * 60 * 60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5 * 60u64,
            platform_fees_collector_wallet: "platform_fee_collector_wallet_1111".to_string(),
            platform_fees: Uint128::from(100u128),
            transaction_fees: Uint128::from(30u128),
            control_fees: Uint128::from(50u128),
        };
        let adminInfo = mock_info("admin11111", &[]);
        let mintingContractInfo = mock_info("minting_admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        let owner1_info = mock_info("Owner001", &[coin(0, "uusd")]);
        buy_a_club(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            Some(String::default()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL
        );

        let stakerInfo = mock_info("Staker001", &[coin(10, "stake")]);
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(99u128),
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(11u128),
            NO_IMMEDIATE_WITHDRAWAL,
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(12u128),
            NO_IMMEDIATE_WITHDRAWAL,
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(13u128),
            NO_IMMEDIATE_WITHDRAWAL,
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(63u128),
            NO_IMMEDIATE_WITHDRAWAL,
        );

        let query_stakes = query_all_stakes(&mut deps.storage);
        match query_stakes {
            Ok(all_stakes) => {
                assert_eq!(all_stakes.len(), 0);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let now = mock_env().block.time; // today

        let query_bonds = query_all_bonds(&mut deps.storage);
        match query_bonds {
            Ok(all_bonds) => {
                let existing_bonds = all_bonds.clone();
                let mut updated_bonds = Vec::new();
                assert_eq!(existing_bonds.len(), 4);
                for bond in existing_bonds {
                    let mut updated_bond = bond.clone();
                    if updated_bond.bonded_amount != Uint128::from(11u128)
                        && updated_bond.bonded_amount != Uint128::from(12u128)
                        && updated_bond.bonded_amount != Uint128::from(13u128)
                        && updated_bond.bonded_amount != Uint128::from(63u128)
                    {
                        println!("updated_bond is {:?} ", updated_bond);
                        assert_eq!(1, 2);
                    }
                    if updated_bond.bonded_amount == Uint128::from(63u128) {
                        updated_bond.bonding_start_timestamp = now.minus_seconds(8 * 24 * 60 * 60);
                    }
                    updated_bonds.push(updated_bond);
                }
                CLUB_BONDING_DETAILS.save(&mut deps.storage, "CLUB001".to_string(), &updated_bonds);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        periodically_refund_stakeouts(deps.as_mut(), mock_env(), adminInfo);

        let queryBondsAfterPeriodicRefund = query_all_bonds(&mut deps.storage);
        match queryBondsAfterPeriodicRefund {
            Ok(all_bonds) => {
                assert_eq!(all_bonds.len(), 3);
                for bond in all_bonds {
                    if bond.bonded_amount != Uint128::from(11u128)
                        && bond.bonded_amount != Uint128::from(12u128)
                        && bond.bonded_amount != Uint128::from(13u128)
                    {
                        println!("bond is {:?} ", bond);
                        assert_eq!(1, 2);
                    }
                }
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
    }

    #[test]
    fn test_non_immediate_partial_withdrawals_from_club() {
        let mut deps = mock_dependencies(&[]);
        let now = mock_env().block.time; // today

        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            astro_proxy_address: "astro_proxy_address1111".to_string(),
            club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(1 * 60 * 60),
            reward_periodicity: 24 * 60 * 60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5 * 60u64,
            platform_fees_collector_wallet: "platform_fee_collector_wallet_1111".to_string(),
            platform_fees: Uint128::from(100u128),
            transaction_fees: Uint128::from(30u128),
            control_fees: Uint128::from(50u128),
        };
        let adminInfo = mock_info("admin11111", &[]);
        let mintingContractInfo = mock_info("minting_admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        )
        .unwrap();

        let owner1_info = mock_info("Owner001", &[coin(0, "uusd")]);
        let result = buy_a_club(
            deps.as_mut(),
            mock_env(),
            owner1_info.clone(),
            "Owner001".to_string(),
            Some(String::default()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL
        );
        println!("buy_a_club result = {:?}", result);
        let stakerInfo = mock_info("Staker001", &[coin(10, "uusd")]);
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(99u128),
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(11u128),
            NO_IMMEDIATE_WITHDRAWAL,
        );
        withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(12u128),
            NO_IMMEDIATE_WITHDRAWAL,
        );
        let result = withdraw_stake_from_a_club(
            deps.as_mut(),
            mock_env(),
            stakerInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(13u128),
            NO_IMMEDIATE_WITHDRAWAL,
        );
        println!("result = {:?}", result);
        let query_stakes = query_all_stakes(&mut deps.storage);
        match query_stakes {
            Ok(all_stakes) => {
                assert_eq!(all_stakes.len(), 1);
                for stake in all_stakes {
                    assert_eq!(stake.staked_amount, Uint128::from(63u128));
                }
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let queryBonds = query_all_bonds(&mut deps.storage);
        match queryBonds {
            Ok(all_bonds) => {
                assert_eq!(all_bonds.len(), 3);
                for bond in all_bonds {
                    if bond.bonded_amount != Uint128::from(11u128)
                        && bond.bonded_amount != Uint128::from(12u128)
                        && bond.bonded_amount != Uint128::from(13u128)
                    {
                        println!("bond is {:?} ", bond);
                        assert_eq!(1, 2);
                    }
                }
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
    }

    #[test]
    fn test_distribute_rewards() {
        let mut deps = mock_dependencies(&[]);
        let now = mock_env().block.time; // today

        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            astro_proxy_address: "astro_proxy_address1111".to_string(),
            club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(8 * 60 * 60),
            reward_periodicity: 5 * 60 * 60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5 * 60u64,
            platform_fees_collector_wallet: "platform_fee_collector_wallet_1111".to_string(),
            platform_fees: Uint128::from(100u128),
            transaction_fees: Uint128::from(30u128),
            control_fees: Uint128::from(50u128),
        };
        let adminInfo = mock_info("admin11111", &[]);
        let mintingContractInfo = mock_info("minting_admin11111", &[]);

        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        )
        .unwrap();

        let owner1Info = mock_info("Owner001", &[coin(1000, "stake")]);
        buy_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Owner001".to_string(),
            Some(String::default()),
            "CLUB001".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL
        );
        let owner2Info = mock_info("Owner002", &[coin(1000, "stake")]);
        buy_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Owner002".to_string(),
            Some(String::default()),
            "CLUB002".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL
        );
        let owner3Info = mock_info("Owner003", &[coin(1000, "stake")]);
        buy_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Owner003".to_string(),
            Some(String::default()),
            "CLUB003".to_string(),
            Uint128::from(1000000u128),
            DONT_QUERY_PAIR_POOL
        );

        let staker1Info = mock_info("Staker001", &[coin(10, "stake")]);
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Staker001".to_string(),
            "CLUB001".to_string(),
            Uint128::from(330000u128),
        );

        let staker2Info = mock_info("Staker002", &[coin(10, "stake")]);
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Staker002".to_string(),
            "CLUB001".to_string(),
            Uint128::from(110000u128),
        );

        let staker3Info = mock_info("Staker003", &[coin(10, "stake")]);
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Staker003".to_string(),
            "CLUB002".to_string(),
            Uint128::from(420000u128),
        );

        let staker4Info = mock_info("Staker004", &[coin(10, "stake")]);
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Staker004".to_string(),
            "CLUB002".to_string(),
            Uint128::from(100000u128),
        );

        let staker5Info = mock_info("Staker005", &[coin(10, "stake")]);
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Staker005".to_string(),
            "CLUB003".to_string(),
            Uint128::from(820000u128),
        );

        let staker6Info = mock_info("Staker006", &[coin(10, "stake")]);
        stake_on_a_club(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "Staker006".to_string(),
            "CLUB003".to_string(),
            Uint128::from(50000u128),
        );

        // let instantiate_msg = InstantiateMsg {
        //     admin_address: "admin11111".to_string(),
        //     minting_contract_address: "minting_admin11111".to_string(),
        //     club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
        //     club_reward_next_timestamp: now.minus_seconds(1*60*60),
        //     reward_periodicity: 24*60*60u64,
        //     club_price: Uint128::from(1000000u128),
        //     bonding_duration: 5*60u64,
        // };
        // let adminInfo = mock_info("admin11111", &[]);
        // let mintingContractInfo = mock_info("minting_admin11111", &[]);
        // instantiate(
        //     deps.as_mut(),
        //     mock_env(),
        //     adminInfo.clone(),
        //     instantiate_msg,
        // )
        // .unwrap();

        increase_reward_amount(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "reward_from abc".to_string(),
            Uint128::from(1000000u128),
        );

        let res = execute(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            ExecuteMsg::CalculateAndDistributeRewards {},
        )
        .unwrap();
        assert_eq!(res, Response::default());

        let queryRes = query_all_stakes(&mut deps.storage);
        match queryRes {
            Ok(all_stakes) => {
                for stake in all_stakes {
                    let staker_address = stake.staker_address;
                    let reward_amount = stake.reward_amount;
                    if staker_address == "Staker001" {
                        assert_eq!(reward_amount, Uint128::from(144262u128));
                    }
                    if staker_address == "Staker002" {
                        assert_eq!(reward_amount, Uint128::from(48087u128));
                    }
                    if staker_address == "Staker003" {
                        assert_eq!(reward_amount, Uint128::from(183606u128));
                    }
                    if staker_address == "Staker004" {
                        assert_eq!(reward_amount, Uint128::from(43715u128));
                    }
                    if staker_address == "Staker005" {
                        assert_eq!(reward_amount, Uint128::from(537549u128));
                    }
                    if staker_address == "Staker006" {
                        assert_eq!(reward_amount, Uint128::from(32776u128));
                    }
                }
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        // test another attempt to calculate and distribute at the same time

        increase_reward_amount(
            deps.as_mut(),
            mock_env(),
            mintingContractInfo.clone(),
            "reward_from abc".to_string(),
            Uint128::from(1000000u128),
        );

        let err = execute(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            ExecuteMsg::CalculateAndDistributeRewards {},
        )
        .unwrap_err();

        assert_eq!(
            err,
            (ContractError::Std(StdError::GenericErr {
                msg: String::from("Time for Reward not yet arrived")
            }))
        );

        // test by preponing club_reward_next_timestamp
        let instantiate_msg = InstantiateMsg {
            admin_address: "admin11111".to_string(),
            minting_contract_address: "minting_admin11111".to_string(),
            astro_proxy_address: "astro_proxy_address1111".to_string(),
            club_fee_collector_wallet: "club_fee_collector_wallet11111".to_string(),
            club_reward_next_timestamp: now.minus_seconds(1 * 60 * 60),
            reward_periodicity: 5 * 60 * 60u64,
            club_price: Uint128::from(1000000u128),
            bonding_duration: 5 * 60u64,
            platform_fees_collector_wallet: "platform_fee_collector_wallet_1111".to_string(),
            platform_fees: Uint128::from(100u128),
            transaction_fees: Uint128::from(30u128),
            control_fees: Uint128::from(50u128),
        };
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        )
        .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            ExecuteMsg::CalculateAndDistributeRewards {},
        )
        .unwrap();

        assert_eq!(res, Response::default());

        /*
            winner club owner address = "Owner003"
            winner club owner reward = Uint128::from(10000)
            reward for "Staker001" is Uint128::from(144262)
            reward for "Staker002" is Uint128::from(48087)
            reward for "Staker003" is Uint128::from(183606)
            reward for "Staker004" is Uint128::from(43715)
            reward for "Staker005" is Uint128::from(537549)
            reward for "Staker006" is Uint128::from(32776)
            total reward given Uint128::from(999995) out of Uint128::from(1000000)
        */
    }
}

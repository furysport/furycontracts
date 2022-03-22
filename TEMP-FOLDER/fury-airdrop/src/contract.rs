#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
  attr, from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response, StdError, StdResult,
  Storage, Uint128, WasmMsg, SubMsg,
};

use cw2::set_contract_version;
use cw20::{
    Cw20ExecuteMsg, Cw20ReceiveMsg,
};

use crate::allowances::{query_allowance};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ReceivedMsg};
use crate::state::{
  Config, LunaUserDetails, UserActivityDetails, UserRewardInfo, CONFIG, LUNA_USER_DETAILS,
  USER_ACTIVITY_DETAILS,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:crll-airdrop";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// Note that Luna activity is just a placeholder - so 3 activities
const NUM_OF_USER_ACTIVITIES: usize = 4;
const LUNA_ACTIVITY: &str = "LUNA_ACTIVITY";
const GAMING_ACTIVITY: &str = "GAMING_ACTIVITY";
const STAKING_ACTIVITY: &str = "STAKING_ACTIVITY";
const LIQUIDITY_ACTIVITY: &str = "LIQUIDITY_ACTIVITY";

const QUALIFIED_FOR_REWARD: bool = true;
const NOT_QUALIFIED_FOR_REWARD: bool = false;

const LOCKED: u128 = 1u128;
const UNLOCKED: u128 = 0u128;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
  deps: DepsMut,
  _env: Env,
  _info: MessageInfo,
  msg: InstantiateMsg,
) -> Result<Response, ContractError> {
  set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

  let config = Config {
    admin_address: deps.api.addr_validate(&msg.admin_address)?,
    minting_contract_address: deps.api.addr_validate(&msg.minting_contract_address)?,
  };
  CONFIG.save(deps.storage, &config)?;

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
    ExecuteMsg::ClaimUserRewards { 
        user_name, 
    } => {
        claim_user_rewards (deps, env, info, user_name)
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
        ReceivedMsg::UpdateActivityRewardForUsers(uarfuc) => update_activity_reward_for_users(
            deps,
            env,
            info,
            uarfuc.activity_name,
            uarfuc.user_reward_list,
            amount,
        ),
    }
}


fn update_activity_reward_for_users(
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  activity_name: String,
  user_reward_list: Vec<UserRewardInfo>,
  total_reward_amount: Uint128,
) -> Result<Response, ContractError> {
  let config = CONFIG.load(deps.storage)?;
  if info.sender != config.minting_contract_address {
    return Err(ContractError::Unauthorized {});
  }

  let mut computed_total_reward = Uint128::zero();
  for user_reward in user_reward_list {
    let user_name = user_reward.user_name;
    let reward_amount = user_reward.reward_amount;
    computed_total_reward += reward_amount;
    println!(
      "username {:?} activity {:?} reward {:?}",
      user_name.clone(),
      activity_name,
      reward_amount
    );

    let res = query_luna_user_details(deps.storage, user_name.clone());
    match res {
      Ok(user) => {
        // User is already created - do nothing
      }
      Err(e) => {
        // Create the user
        LUNA_USER_DETAILS.save(
          deps.storage,
          user_name.clone(),
          &LunaUserDetails {
            user_name: user_name.clone(),
            luna_airdrop_qualified: QUALIFIED_FOR_REWARD,
            luna_airdrop_reward_amount: Uint128::zero(),
          },
        )?;
        // also create activities : luna, gaming, staking, liquidity
        let mut activities = Vec::new();
        activities.push(UserActivityDetails {
          user_name: user_name.clone(),
          activity_name: LUNA_ACTIVITY.to_string(),
          activity_qualified: NOT_QUALIFIED_FOR_REWARD,
          activity_reward_amount_accrued: Uint128::zero(),
        });
        activities.push(UserActivityDetails {
          user_name: user_name.clone(),
          activity_name: GAMING_ACTIVITY.to_string(),
          activity_qualified: NOT_QUALIFIED_FOR_REWARD,
          activity_reward_amount_accrued: Uint128::zero(),
        });
        activities.push(UserActivityDetails {
          user_name: user_name.clone(),
          activity_name: STAKING_ACTIVITY.to_string(),
          activity_qualified: NOT_QUALIFIED_FOR_REWARD,
          activity_reward_amount_accrued: Uint128::zero(),
        });
        activities.push(UserActivityDetails {
          user_name: user_name.clone(),
          activity_name: LIQUIDITY_ACTIVITY.to_string(),
          activity_qualified: NOT_QUALIFIED_FOR_REWARD,
          activity_reward_amount_accrued: Uint128::zero(),
        });
        USER_ACTIVITY_DETAILS.save(deps.storage, user_name.clone(), &activities)?;
      }
    }

    let mut user_activities = Vec::new();
    let all_user_activities = USER_ACTIVITY_DETAILS.may_load(deps.storage, user_name.clone())?;
    match all_user_activities {
      Some(some_user_activities) => {
        user_activities = some_user_activities;
      }
      None => {}
    }
    let existing_user_activities = user_activities.clone();
    let mut updated_user_activities = Vec::new();
    for user_activity in existing_user_activities {
      let mut updated_user_activity = user_activity.clone();
      if user_activity.activity_name == activity_name {
        updated_user_activity.activity_reward_amount_accrued += reward_amount;
      }
      updated_user_activities.push(updated_user_activity);
    }
    USER_ACTIVITY_DETAILS.save(deps.storage, user_name, &updated_user_activities)?;
  }

  if computed_total_reward != total_reward_amount {
    return Err(ContractError::Std(StdError::generic_err("Total reward amount does not match individual reward amounts")));
  }

  return Ok(Response::default());
}

fn claim_user_rewards (
  deps: DepsMut,
  env: Env,
  info: MessageInfo,
  user_name: String,
) -> Result<Response, ContractError> {
  let user_addr = deps.api.addr_validate(&user_name)?;
  if user_addr != info.sender {
      return Err(ContractError::Unauthorized {});
  }

  let mut total_amount = Uint128::zero();

  // Get the existing rewards for this user activities
  let mut activities = Vec::new();
  let all_activities = USER_ACTIVITY_DETAILS.may_load(deps.storage, user_name.clone())?;
  match all_activities {
      Some(some_activities) => {
          activities = some_activities;
      }
      None => {}
  }

  let existing_activities = activities.clone();
  let mut updated_activities = Vec::new();
  for activity in existing_activities {
      let mut updated_activity = activity.clone();
      if activity.activity_reward_amount_accrued > Uint128::zero() {
          total_amount += activity.activity_reward_amount_accrued;
          updated_activity.activity_reward_amount_accrued = Uint128::zero();
      }
      updated_activities.push(updated_activity);
  }
  USER_ACTIVITY_DETAILS.save(deps.storage, user_name.clone(), &updated_activities)?;


  println!("reward amount is {:?}", total_amount);

  if total_amount == Uint128::zero() {
    return Err(ContractError::Std(StdError::GenericErr {
        msg: String::from("No reward for this user"),
      }));
  }
  

  // transfer total amount to user wallet
  transfer_from_contract_to_wallet(
    deps.storage, 
    user_name.clone(), 
    total_amount,
    "reward".to_string()
  ) 
}

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
        .add_attribute("amount", amount.to_string())
        .add_attribute("action", action)
        .set_data(data_msg));
}



#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
  match msg {
    QueryMsg::UserActivityDetails { user_name } => {
        to_binary(&query_airdrop_activity_details(deps.storage, user_name)?)
    }
    QueryMsg::QueryUserRewards { user_name } => {
        to_binary(&query_user_rewards(deps.storage, user_name)?)
    }
  }
}


fn query_user_rewards(storage: &dyn Storage, user_name: String) -> StdResult<Uint128> {
  let mut total_amount = Uint128::zero();
  let mut activities = Vec::new();
  let all_activities = USER_ACTIVITY_DETAILS.may_load(storage, user_name.clone())?;
  match all_activities {
      Some(some_activities) => {
          activities = some_activities;
      }
      None => {}
  }
  for activity in activities {
      if activity.activity_reward_amount_accrued > Uint128::zero() {
          total_amount += activity.activity_reward_amount_accrued;
      }
  }
  return Ok(total_amount);
}


fn query_luna_user_details(storage: &dyn Storage, user_name: String) -> StdResult<LunaUserDetails> {
  let lud = LUNA_USER_DETAILS.may_load(storage, user_name)?;
  match lud {
    Some(lud) => return Ok(lud),
    None => return Err(StdError::generic_err("No luna user details found")),
  };
}

pub fn query_airdrop_activity_details(
  storage: &dyn Storage,
  user_name: String,
) -> StdResult<Vec<UserActivityDetails>> {
  let ad = USER_ACTIVITY_DETAILS.may_load(storage, user_name)?;
  match ad {
      Some(ad) => return Ok(ad),
      None => return Err(StdError::generic_err("No airdrop activity details found")),
  };
}

fn query_all_user_activities(storage: &dyn Storage) -> StdResult<Vec<UserActivityDetails>> {
  let mut all_activities = Vec::new();
  let all_users: Vec<String> = USER_ACTIVITY_DETAILS
      .keys(storage, None, None, Order::Ascending)
      .map(|k| String::from_utf8(k).unwrap())
      .collect();
  for user_name in all_users {
      let activity_details = USER_ACTIVITY_DETAILS.load(storage, user_name)?;
      for activity in activity_details {
          all_activities.push(activity);
      }
  }
  return Ok(all_activities);
}


#[cfg(test)]
mod tests {
  use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
  
  use super::*;
  use cosmwasm_std::coin;

    #[test]
    fn test_userlist_update_activity() {
          let mut deps = mock_dependencies(&[]);

          let instantiate_msg = InstantiateMsg {
              minting_contract_address: "cwtoken11111".to_string(),
              admin_address: "admin11111".to_string(),
          };
          let rewardInfo = mock_info("rewardInfo", &[]);
          let tokenInfo = mock_info("cwtoken11111", &[coin(1000, "stake")]);


          let mut user_name_list_for_final_processing = Vec::new();
          let total_count = 1000;
          // Worked up to 1 million. Reducing it to 100 
          let user_reward_amount = Uint128::from(100u128);
          for count in 1..total_count+1 {
              let count_str : String = count.to_string();
              let mut username = String::new();
              username += "LunaUser_";
              username += &count_str;

              let mut user_reward = UserRewardInfo { 
                user_name: username.clone(),
                reward_amount: user_reward_amount,
              };
              user_name_list_for_final_processing.push (user_reward);
          }
      
          instantiate(deps.as_mut(), mock_env(), rewardInfo.clone(), instantiate_msg).unwrap();

          let num = total_count as u128;
          let total_reward = user_reward_amount.checked_mul(Uint128::from(num)).unwrap_or_default();
          update_activity_reward_for_users (deps.as_mut(), mock_env(), tokenInfo.clone(), 
            "STAKING_ACTIVITY".to_string(), user_name_list_for_final_processing.clone(), total_reward);
          
          let all_luna_users: Vec<String> = LUNA_USER_DETAILS
              .keys(&deps.storage, None, None, Order::Ascending)
              .map(|k| String::from_utf8(k).unwrap())
              .collect();
          for user in all_luna_users {
              // check that these many can be loaded in memory
              // it maxes out at 2 million for my machine
              // i7 processor, 32GB RAM, 1 TB SSD

              let queryRes = query_luna_user_details (&deps.storage, user);
              match queryRes {
                  Ok(lud) => {
                      assert_eq!(lud.luna_airdrop_qualified, QUALIFIED_FOR_REWARD);
                      assert_eq!(lud.luna_airdrop_reward_amount, Uint128::zero());
                  }
                  Err(e) => {
                      println!("error parsing header: {:?}", e);
                      assert_eq!(1, 2);
                  }
              }
          }
          let queryAllUserActRes = query_all_user_activities(&mut deps.storage);
          match queryAllUserActRes {
              Ok(all_acts) => {
                  assert_eq!(all_acts.len(), total_count*NUM_OF_USER_ACTIVITIES);
                  for act in all_acts {
                      if act.activity_name != STAKING_ACTIVITY.to_string() {
                          assert_eq!(act.activity_reward_amount_accrued, Uint128::zero());
                      } else {
                          assert_eq!(act.activity_reward_amount_accrued, Uint128::from(100u128));
                      }
                  }
              }
              Err(e) => {
                  println!("error parsing header: {:?}", e);
                  assert_eq!(1, 2);
              }
          }

          update_activity_reward_for_users (deps.as_mut(), mock_env(), tokenInfo.clone(), 
            "LUNA_ACTIVITY".to_string(), user_name_list_for_final_processing.clone(), total_reward);

          let all_luna_users_2: Vec<String> = LUNA_USER_DETAILS
              .keys(&deps.storage, None, None, Order::Ascending)
              .map(|k| String::from_utf8(k).unwrap())
              .collect();
          for user in all_luna_users_2 {
              // check that these many can be loaded in memory
              // it maxes out at 2 million for my machine
              // i7 processor, 32GB RAM, 1 TB SSD

              let queryRes = query_luna_user_details (&deps.storage, user);
              match queryRes {
                  Ok(lud) => {
                      assert_eq!(lud.luna_airdrop_qualified, QUALIFIED_FOR_REWARD);
                      assert_eq!(lud.luna_airdrop_reward_amount, Uint128::zero());
                  }
                  Err(e) => {
                      println!("error parsing header: {:?}", e);
                      assert_eq!(1, 2);
                  }
              }
          }
          let queryAllUserActRes_2 = query_all_user_activities(&mut deps.storage);
          match queryAllUserActRes_2 {
              Ok(all_acts) => {
                  assert_eq!(all_acts.len(), total_count*NUM_OF_USER_ACTIVITIES);
                  for act in all_acts {
                      if act.activity_name == STAKING_ACTIVITY.to_string() 
                         || act.activity_name == LUNA_ACTIVITY.to_string() {
                          assert_eq!(act.activity_reward_amount_accrued, Uint128::from(100u128));
                      } else {
                          assert_eq!(act.activity_reward_amount_accrued, Uint128::zero());
                      }
                  }
              }
              Err(e) => {
                  println!("error parsing header: {:?}", e);
                  assert_eq!(1, 2);
              }
          }

          let user1Info = mock_info("LunaUser_1", &[coin(1000, "stake")]);
          let rsp1 = claim_user_rewards(deps.as_mut(), mock_env(), user1Info.clone(), "LunaUser_1".to_string());
          match rsp1 {
              Ok(rsp1) => {
                  assert_eq!(rsp1.attributes[0].value.clone(), "200");
              }
              Err(e) => {
                  println!("error parsing header: {:?}", e);
                  assert_eq!(1, 2);
              }
          }
    }
}

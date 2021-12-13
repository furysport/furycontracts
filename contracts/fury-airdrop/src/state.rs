use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw20::AllowanceResponse;

pub const CONFIG_KEY: &str = "config";
pub const CONFIG: Item<Config> = Item::new(CONFIG_KEY);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub admin_address: Addr,
    pub minting_contract_address: Addr,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct UserRewardInfo {
    /// Address of the user wallet
    pub user_name: String,

    /// airdrop reward amount in quantity of tokens
    pub reward_amount: Uint128,
}

/// This is used for saving various activity details
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct ActivityDetails {
    /// Name of the activity
    pub activity_name: String,

    /// The activity which is a prereq for this activity
    /// If the activity_name is same prereq_activity_name, it means no prereq
    pub prereq_activity_name: String,

    /// total airdrop reward amount if the activity is completed in quantity of tokens
    pub eligible_activity_reward_amount: Uint128,
}

pub const ALLOWANCES: Map<(&Addr, &Addr), AllowanceResponse> = Map::new("allowance");

/// Map of users and its Luna Airdrop information.
/// the key is user name and the
/// LunaUserDetails will contain information about the Luna staking
pub const LUNA_USER_DETAILS: Map<String, LunaUserDetails> = Map::new("luna_user_details");

/// Map of users and list of their Activities. the key is activity name and the
/// ActivityDetails will contain information about the activity
pub const ACTIVITY_DETAILS: Map<String, Vec<ActivityDetails>> =
    Map::new("activity_details");

/// Map of users and list of their Activities. the key is user name and the
/// UserActivityDetails will contain information about the users and activities completed
pub const USER_ACTIVITY_DETAILS: Map<String, Vec<UserActivityDetails>> =
    Map::new("user_activity_details");

/// This is used for saving various activity details
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct LunaUserDetails {
    /// Address of the user wallet
    pub user_name: String,

    /// Not used anymore
    /// is the user qualified for luna airdrop
    pub luna_airdrop_qualified: bool,

    /// Not used anymore
    /// luna airdrop reward amount calculated outside of the contract
    pub luna_airdrop_reward_amount: Uint128,
}

/// This is used for saving various activity details
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct UserActivityDetails {
    /// Address of the user wallet
    pub user_name: String,

    /// Name of the activity
    pub activity_name: String,

    /// is the user qualified for activity. Determines the eligibility for airdrop 
    pub activity_qualified: bool,

    /// airdrop reward amount acrrued for this activity in quantity of tokens
    pub activity_reward_amount_accrued: Uint128,
}


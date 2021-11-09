use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Timestamp, Uint128};
use cw_storage_plus::{Item, Map};

use cw20::AllowanceResponse;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    pub cw20_token_address: Addr,
    pub admin_address: Addr,
}

pub const CONFIG_KEY: &str = "config";
pub const CONFIG: Item<Config> = Item::new(CONFIG_KEY);

/// This is used for saving various vesting details
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct ClubOwnershipDetails {
    /// The club name
    pub club_name: String,
    /// The system timestamp to be used as starting point when ownership
    /// of a club was taken. the 21 days restrictions start from this time
    pub start_timestamp: Timestamp,

    /// The locking period(days) expressed in seconds
    pub locking_period: u64,

    pub owner_address: String,

    pub price_paid: Uint128,
}

/// This is used for saving various vesting details
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct ClubStakingDetails {
    pub club_name: String,

    pub staker_address: String,

    /// The system timestamp to be used as starting point of staking
    pub staking_start_timestamp: Timestamp,

    /// staked amount in quantity of tokens
    pub staked_amount: Uint128,
    /// Duration of staking expressed in seconds
    pub staking_duration: u64,
}

pub const ALLOWANCES: Map<(&Addr, &Addr), AllowanceResponse> = Map::new("allowance");
/// Map of clubs and its owners. the key is club name and the
/// ClubOwnershipDetails will contain information about the owner
pub const CLUB_OWNERSHIP_DETAILS: Map<String, ClubOwnershipDetails> =
    Map::new("club_ownership_details");
/// Map of clubs and its stakers. the key is club name and the
/// ClubStakingDetails will contain information about the stakers and amount staked
pub const CLUB_STAKING_DETAILS: Map<String, Vec<ClubStakingDetails>> =
    Map::new("club_staking_details");

pub const CONTRACT_WALLET: Map<&Addr, Uint128> = Map::new("contract_wallet");

pub const REWARD: Item<Uint128> = Item::new("staking_reward");

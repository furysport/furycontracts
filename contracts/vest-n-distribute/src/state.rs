use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128, Timestamp};
use cw_storage_plus::{Map};

/// This is used for saving various vesting details
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub struct VestingDetails {
    /// The system timestamp to be used as starting point of vesting schedule
    pub vesting_start_timestamp: Timestamp,

    /// Initial seed amount. The seed quantity of tokens
    pub initial_vesting_count: Uint128,
    
    /// amount claimed fom initial seed
    pub initial_vesting_consumed: Uint128,
    
    /// The vesting periodicity(hourly/daily/weekly/monthly) expressed in seconds
    pub vesting_periodicity: u64,
    
    /// vesting count for each period
    pub vesting_count_per_period: Uint128,
    
    /// Total amount to be vested over period of time 
    /// This also includes the seed tokens
    pub total_vesting_token_count: Uint128,
    
    /// Total number of tokens transferred till date
    /// This also includes the seed tokens 
    pub total_claimed_tokens_till_now: Uint128,
    
    /// Timestamp for the latest claimed/transferred transaction.
    /// This will be used to calculate the next vesting count
    pub last_claimed_timestamp: Option<Timestamp>,
    
    /// Number of tokens available for claiming
    pub tokens_available_to_claim: Uint128,

    /// Timestamp for the latest claimed/transferred transaction.
    /// This will be used to calculate the next vesting count
    pub last_vesting_timestamp: Option<Timestamp>,
    
    /// Cliff time period expressed in months -- change it into 4 weeks 
    pub cliff_period: u64,

    /// This contains addredd of parent category. It will be none for all main categories. 
    pub parent_category_address: Option<String>,

    /// Flag to let system know if the vested amount has to be transferred immediately
    /// Or should be kept in allowances for the vester to claim
    pub should_transfer: bool,
}


pub const VESTING_DETAILS: Map<&Addr, VestingDetails> = Map::new("vesting_details");

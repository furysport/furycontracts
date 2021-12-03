use cosmwasm_std::{Binary, Uint128};
use cw0::Expiration;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::state::{ UserRewardInfo };

// #[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
// pub struct InstantiateMarketingInfo {
//     pub project: Option<String>,
//     pub description: Option<String>,
//     pub marketing: Option<String>,
//     pub logo: Option<Logo>,
// }

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMsg {
    pub cw20_token_address: String,
    pub admin_address: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateUserRewardAmount {
        activity_name: String,
        user_reward_list: Vec<UserRewardInfo>,
    },
    ClaimUserRewards {
        user_name: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Only with "allowance" extension.
    /// Returns how much spender can use from owner account, 0 if unset.
    /// Return type: AllowanceResponse.
    Allowance {
        owner: String,
        spender: String,
    },
    /// Only with "enumerable" extension (and "allowances")
    /// Returns all allowances this owner has approved. Supports pagination.
    /// Return type: AllAllowancesResponse.
    AllAllowances {
        owner: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Returns the current state of activity information for the given address.
    /// Return type: UserActivityDetails.
    UserActivityDetails {
        user_name: String,
    },
}

use cosmwasm_std::{Binary, Uint128};
use cw0::Expiration;
use cw20::{Cw20ReceiveMsg, Logo};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Coin, Timestamp};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMarketingInfo {
    pub project: Option<String>,
    pub description: Option<String>,
    pub marketing: Option<String>,
    pub logo: Option<Logo>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMsg {
    pub admin_address: String,
    pub minting_contract_address: String,
    pub club_fee_collector_wallet: String,
    pub club_reward_next_timestamp: Timestamp,
    pub reward_periodicity: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    ReleaseClub {
        owner: String,
        club_name: String,
    },
    ClaimOwnerRewards {
        owner: String,
        club_name: String,
        amount: Uint128,
    },
    ClaimPreviousOwnerRewards {
        previous_owner: String,
        club_name: String,
        amount: Uint128,
    },
    StakeWithdrawFromAClub {
        staker: String,
        club_name: String,
        amount: Uint128,
        immediate_withdrawal: bool,
    },
    PeriodicallyRefundStakeouts {},
    CalculateAndDistributeRewards {},
    ClaimRewards {
        staker: String,
        club_name: String,
        amount: Uint128,
    },
    IncreaseAllowance {
        spender: String,
        amount: Uint128,
        expires: Option<Expiration>,
    },
    DecreaseAllowance {
        spender: String,
        amount: Uint128,
        expires: Option<Expiration>,
    },
    TransferFrom {
        owner: String,
        recipient: String,
        amount: Uint128,
    },
    BurnFrom {
        owner: String,
        amount: Uint128,
    },
    SendFrom {
        owner: String,
        contract: String,
        amount: Uint128,
        msg: Binary,
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
    /// Returns the current state of vesting information for the given address.
    /// Return type: StakingDetails.
    ClubStakingDetails {
        club_name: String,
    },
    /// Returns the current state of withdrawn tokens that are locked for
    /// BONDING_DURATION = 7 days (before being credited back) for the given address.
    /// Return type: BondingDetails.
    ClubBondingDetails {
        club_name: String,
    },
    ClubOwnershipDetails {
        club_name: String,
    },
    AllStakes {},
    GetClubRankingByStakes {},
    RewardAmount {},
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceivedMsg {
    BuyAClub(BuyClubCommand),
    StakeOnAClub(StakeOnAClubCommand),
    IncreaseRewardAmount(IncreaseRewardAmountCommand),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct BuyClubCommand {
    pub buyer: String,
    pub seller: String,
    pub club_name: String,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakeOnAClubCommand {
    pub staker: String,
    pub club_name: String,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct IncreaseRewardAmountCommand {
    pub reward_from: String,
}

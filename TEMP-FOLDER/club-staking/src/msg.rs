use cosmwasm_std::{Binary, Uint128};
use cw0::Expiration;
use cw20::{Cw20ReceiveMsg, Logo};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cosmwasm_std::{Coin, Timestamp};

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMsg {
    pub admin_address: String,
    pub minting_contract_address: String,
    pub astro_proxy_address: String,
    pub club_fee_collector_wallet: String,
    pub club_reward_next_timestamp: Timestamp,
    pub reward_periodicity: u64,
    pub club_price: Uint128,
    pub bonding_duration: u64,
    pub platform_fees_collector_wallet: String,
    ///Specified in percentage multiplied by 100, i.e. 100% = 10000 and 0.01% = 1
    pub platform_fees: Uint128,
    ///Specified in percentage multiplied by 100, i.e. 100% = 10000 and 0.01% = 1
    pub transaction_fees: Uint128,
    ///Specified in percentage multiplied by 100, i.e. 100% = 10000 and 0.01% = 1
    pub control_fees: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    Receive(Cw20ReceiveMsg),
    BuyAClub {
        buyer: String,
        seller: Option<String>,
        club_name: String,
    },
    StakeOnAClub {
        staker: String,
        club_name: String,
        amount: Uint128,
    },
    ReleaseClub {
        owner: String,
        club_name: String,
    },
    ClaimOwnerRewards {
        owner: String,
        club_name: String,
    },
    ClaimPreviousOwnerRewards {
        previous_owner: String,
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
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
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
    ClubPreviousOwnershipDetails {
        previous_owner: String,
    },
    ClubOwnershipDetailsForOwner {
        owner_address: String,
    },
    AllClubOwnershipDetails {
    },
    AllPreviousClubOwnershipDetails {
    },
    AllStakes {},
    AllStakesForUser { 
		user_address: String,
	},
    AllBonds {},
    ClubBondingDetailsForUser { 
        club_name: String,
        user_address: String,
    },
    GetClubRankingByStakes {},
    RewardAmount {},
    QueryPlatformFees { 
        msg: Binary,
    },
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceivedMsg {
    IncreaseRewardAmount(IncreaseRewardAmountCommand),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct IncreaseRewardAmountCommand {
    pub reward_from: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum ProxyQueryMsgs {
    GetFuryEquivalentToUst {
        ust_count: Uint128,
    },
    GetUstEquivalentToFury {
        fury_count: Uint128,
    },
}


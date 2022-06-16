use cosmwasm_std::{Binary, Decimal, Timestamp, Uint128, Uint64};
use cw20::Cw20ReceiveMsg;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use terraswap::asset::Asset;

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
pub struct InstantiateMsg {
    /// admin address for configuration activities
    pub admin_address: String,

    /// contract address of Fury token
    pub custom_token_address: String,

    /// discount_rate when fury and UST are both provided
    pub pair_discount_rate: u16,
    /// bonding period when fury and UST are both provided
    pub pair_bonding_period_in_sec: u64,
    /// Fury tokens for balanced investment will be fetched from this wallet
    pub pair_fury_reward_wallet: String,
    /// The LP tokens generated at time of Pair investment for discounted Fury Rewards will be assigned to this wallet
    pub pair_lp_tokens_holder: String,

    /// discount_rate when only UST is provided
    pub native_discount_rate: u16,
    /// bonding period when only UST is provided
    pub native_bonding_period_in_sec: u64,
    /// Fury tokens for native(UST only) investment will be fetched from this wallet
    pub native_investment_reward_wallet: String,
    /// The native(UST only) investment will be stored into this wallet
    pub native_investment_receive_wallet: String,

    /// This address has the authority to provide liquidity (balanced UST + Fury) and in return shall get the LP tokens
    pub authorized_liquidity_provider: String,
    /// Time in nano seconds since EPOC when the swapping will be enabled
    pub swap_opening_date: Uint64,
    /// The underlying Astroport-Core Pair Contract address
    pub pool_pair_address: Option<String>,
    /// The wallet to which various fees that is collected shall be transferred
    pub platform_fees_collector_wallet: String,
    /// Platform Fee Specified in percentage multiplied by 100, i.e. 100% = 10000 and 0.01% = 1
    pub platform_fees: Uint128,
    /// Transaction Fee Specified in percentage multiplied by 100, i.e. 100% = 10000 and 0.01% = 1
    pub transaction_fees: Uint128,
    /// Swap Fees pecified in percentage multiplied by 100, i.e. 100% = 10000 and 0.01% = 1
    pub swap_fees: Uint128,
    /// Maximum number of simultaneous outstanding Bonds of discounted Reward Fury Tokens permitted per user
    pub max_bonding_limit_per_user: u64,
    pub usdc_ibc_symbol: String,

}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CW20Custom {
    HelloSub {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Configure the Proxy Parameters after Astroport Core contract initialization
    Configure {
        /// Liquidity Pool Pair contract address of Astroport-Core
        pool_pair_address: Option<String>,
        /// Liquidity LP token contract address of Astroport-Core
        liquidity_token: Option<String>,
        ///Time in nano seconds since EPOC when the swapping will be enabled
        swap_opening_date: Uint64,
    },
    /// ## Description
    /// Receives a message of type [`Cw20ReceiveMsg`]
    Receive(Cw20ReceiveMsg),
    /// ProvidePairForReward a user provides pair liquidity (UST + Fury balanced) and gets Fury rewards
    ProvidePairForReward {
        /// the type of asset available in [`Asset`]
        assets: [Asset; 2],
        /// the slippage tolerance for sets the maximum percent of price movement
        slippage_tolerance: Option<Decimal>,
        /// Determines whether an autostake will be performed on the generator
        auto_stake: Option<bool>,
    },
    /// ProvideNativeForReward a user provides native liquidity (UST only) and gets Fury rewards
    ProvideNativeForReward {
        /// the type of asset available in [`Asset`]
        asset: Asset,
        /// the slippage tolerance for sets the maximum percent of price movement
        slippage_tolerance: Option<Decimal>,
        /// Determines whether an autostake will be performed on the generator
        auto_stake: Option<bool>,
    },
    /// ProvideLiquidity an Authorized user provides pair liquidity and gets lp_tokens
    ProvideLiquidity {
        /// the type of asset available in [`Asset`]
        assets: [Asset; 2],
        /// the slippage tolerance for sets the maximum percent of price movement
        slippage_tolerance: Option<Decimal>,
        /// Determines whether an autostake will be performed on the generator
        auto_stake: Option<bool>,
    },
    /// Swap an offer asset to the other
    Swap {
        offer_asset: Asset,
        belief_price: Option<Decimal>,
        max_spread: Option<Decimal>,
        to: Option<String>,
    },
    /// Claim the Discounted Reward Fury after bond maturity
    RewardClaim {
        receiver: String,
        withdrawal_amount: Uint128,
    },
    HelloSub {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Returns information about Proxy Configuration
    Configuration {},
    /// Returns counts of Tokens related to the Liquidity Pool
    Pool {},
    /// Returns information about Tokens Paired in the Liquidity Pool
    Pair {},
    /// Returns information about the simulation of the swap in a [`SimulationResponse`] object.
    Simulation {
        offer_asset: Asset,
    },
    /// Returns information about the reverse simulation in a [`ReverseSimulationResponse`] object.
    ReverseSimulation {
        ask_asset: Asset,
    },

    /// Returns information about the cumulative prices in a [`CumulativePricesResponse`] object
    // CumulativePrices {},

    /// Returns Timestamp after which Swap operations would be permitted in the Liquidity Pool
    GetSwapOpeningDate {},
    /// Returns status of Fury Reward Tokens Bonded or allocated at discounted rate against Native or Pair Investment
    GetBondingDetails {
        user_address: String,
    },
    /// Returns Fury Equivalent for some UST amount (without operational overheads of swap)
    GetFuryEquivalentToUst {
        ust_count: Uint128,
    },
    /// Returns UST Equivalent for some Fury amount (without operational overheads of swap)
    GetUstEquivalentToFury {
        fury_count: Uint128,
    },
    /// Returns Platform Fee required for specific ExecuteMsg
    QueryPlatformFees {
        msg: Binary,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProxyCw20HookMsg {
    // ProvideLiquidity {
    //     /// the type of asset available in [`Asset`]
    //     assets: [Asset; 2],
    //     /// the slippage tolerance for sets the maximum percent of price movement
    //     slippage_tolerance: Option<Decimal>,
    //     /// Determines whether an autostake will be performed on the generator
    //     auto_stake: Option<bool>,
    //     /// the receiver of provide liquidity
    //     receiver: Option<String>,
    // },
    /// Sell a given amount of asset
    // Swap {
    //     belief_price: Option<Decimal>,
    //     max_spread: Option<Decimal>,
    //     to: Option<String>,
    // },
    /// Withdrawing liquidity from the pool against the LP Tokens
    WithdrawLiquidity {},
}

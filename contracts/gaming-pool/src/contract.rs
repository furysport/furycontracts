#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response,
    StdError, StdResult, Storage, SubMsg, Timestamp, Uint128, WasmMsg,
};

use cw2::set_contract_version;
use cw20::{
    AllowanceResponse, BalanceResponse, Cw20Coin, Cw20ExecuteMsg, Cw20ReceiveMsg, Expiration,
};

use cosmwasm_std::Coin;

use crate::allowances::{
    deduct_allowance, execute_burn_from, execute_decrease_allowance, execute_increase_allowance,
    execute_send_from, execute_transfer_from, query_allowance,
};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, ReceivedMsg};
use crate::state::{
    Config, GameDetails, GameResult, PoolDetails, PoolTeamDetails, PoolTypeDetails,
    WalletPercentage, WalletTransferDetails, CONFIG, CONTRACT_POOL_COUNT, GAME_DETAILS,
    GAME_RESULT_DUMMY, GAMING_FUNDS, PLATFORM_WALLET_PERCENTAGES, POOL_DETAILS, POOL_TEAM_DETAILS,
    POOL_TYPE_DETAILS,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:gaming-pool";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const DUMMY_WALLET: &str = "terra1t3czdl5h4w4qwgkzs80fdstj0z7rfv9v2j6uh3";

// Initial reward amount to gamer for joining a pool
const INITIAL_REWARD_AMOUNT: u128 = 0u128;
// Initial refund amount to gamer for joining a pool
const INITIAL_REFUND_AMOUNT: u128 = 0u128;

// Initial value of team points
const INITIAL_TEAM_POINTS: u64 = 0u64;

// Initial rank of team - set to a low rank more than max pool size
const INITIAL_TEAM_RANK: u64 = 100000u64;

const UNCLAIMED_REWARD: bool = false;
const CLAIMED_REWARD: bool = true;
const UNCLAIMED_REFUND: bool = false;
const CLAIMED_REFUND: bool = true;
const REWARDS_DISTRIBUTED: bool = true;
const REWARDS_NOT_DISTRIBUTED: bool = false;

const GAME_POOL_OPEN: u64 = 1u64;
const GAME_POOL_CLOSED: u64 = 2u64;
const GAME_CANCELLED: u64 = 3u64;
const GAME_COMPLETED: u64 = 4u64;

const DUMMY_GAME_ID: &str = "DUMMY_GAME_ID";
const DUMMY_TEAM_ID: &str = "DUMMY_TEAM_ID";

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
        platform_fee: msg.platform_fee,
    };
    CONFIG.save(deps.storage, &config)?;

    let dummy_wallet = String::from(DUMMY_WALLET);
    let main_address = deps.api.addr_validate(dummy_wallet.clone().as_str())?;
    GAME_RESULT_DUMMY.save(
        deps.storage,
        &main_address,
        &GameResult {
            gamer_address: DUMMY_WALLET.to_string(),
            game_id: DUMMY_GAME_ID.to_string(),
            team_id: DUMMY_TEAM_ID.to_string(),
            team_rank: INITIAL_TEAM_RANK,
            team_points: INITIAL_TEAM_POINTS,
            reward_amount: Uint128::from(INITIAL_REWARD_AMOUNT),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        },
    )?;
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
        ExecuteMsg::SetPlatformFeeWallets { wallet_percentages } => {
            set_platform_fee_wallets(deps, info, wallet_percentages)
        }
        ExecuteMsg::SetPoolTypeParams {
            pool_type,
            pool_fee,
            min_teams_for_pool,
            max_teams_for_pool,
            max_teams_for_gamer,
            wallet_percentages,
        } => set_pool_type_params(
            deps,
            env,
            info,
            pool_type,
            pool_fee,
            min_teams_for_pool,
            max_teams_for_pool,
            max_teams_for_gamer,
            wallet_percentages,
        ),
        ExecuteMsg::CreateGame { game_id } => create_game(deps, env, info, game_id),
        ExecuteMsg::CancelGame { game_id } => cancel_game(deps, env, info, game_id),
        ExecuteMsg::LockGame { game_id } => lock_game(deps, env, info, game_id),
        ExecuteMsg::CreatePool { game_id, pool_type } => {
            create_pool(deps, env, info, game_id, pool_type)
        }
        ExecuteMsg::ClaimReward { gamer } => claim_reward(deps, info, gamer),
        ExecuteMsg::ClaimRefund { gamer } => claim_refund(deps, info, gamer),
        ExecuteMsg::GamePoolRewardDistribute {
            game_id,
            pool_id,
            game_winners,
        } => game_pool_reward_distribute(deps, env, info, game_id, pool_id, game_winners),
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
        ReceivedMsg::GamePoolBidSubmit(gpbsc) => game_pool_bid_submit(
            deps,
            env,
            info,
            gpbsc.gamer,
            gpbsc.pool_type,
            gpbsc.pool_id,
            gpbsc.game_id,
            gpbsc.team_id,
            amount,
        ),
    }
}

fn set_platform_fee_wallets(
    deps: DepsMut,
    info: MessageInfo,
    wallet_percentages: Vec<WalletPercentage>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized {
            invoker: info.sender.to_string(),
        });
    }

    for wp in wallet_percentages {
        PLATFORM_WALLET_PERCENTAGES.save(
            deps.storage,
            wp.wallet_name.clone(),
            &WalletPercentage {
                wallet_name: wp.wallet_name.clone(),
                wallet_address: wp.wallet_address.clone(),
                percentage: wp.percentage,
            },
        )?;
    }
    return Ok(Response::default());
}

fn set_pool_type_params(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    pool_type: String,
    pool_fee: Uint128,
    min_teams_for_pool: u32,
    max_teams_for_pool: u32,
    max_teams_for_gamer: u32,
    wallet_percentages: Vec<WalletPercentage>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized {
            invoker: info.sender.to_string(),
        });
    }

    let mut rake_list: Vec<WalletPercentage> = Vec::new();
    for wp in wallet_percentages {
        rake_list.push(wp);
    }
    POOL_TYPE_DETAILS.save(
        deps.storage,
        pool_type.clone(),
        &PoolTypeDetails {
            pool_type: pool_type.clone(),
            pool_fee: pool_fee,
            min_teams_for_pool: min_teams_for_pool,
            max_teams_for_pool: max_teams_for_pool,
            max_teams_for_gamer: max_teams_for_gamer,
            rake_list: rake_list,
        },
    )?;
    return Ok(Response::default());
}

fn create_game(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    game_id: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized {
            invoker: info.sender.to_string(),
        });
    }

    GAME_DETAILS.save(
        deps.storage,
        game_id.clone(),
        &GameDetails {
            game_id: game_id.clone(),
            game_status: GAME_POOL_OPEN,
        },
    )?;
    return Ok(Response::new()
        .add_attribute("game_id", game_id.clone())
        .add_attribute("game_status", "GAME_POOL_OPEN".to_string()));
}

fn cancel_game(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    game_id: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized {
            invoker: info.sender.to_string(),
        });
    }
    let platform_fee = config.platform_fee;

    let gd = GAME_DETAILS.may_load(deps.storage, game_id.clone())?;
    let mut game;
    match gd {
        Some(gd) => {
            game = gd;
        }
        None => {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: String::from("Game status cannot be retrieved"),
            }));
        }
    }
    if game.game_status == GAME_COMPLETED {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Cant cancel game as it is already over"),
        }));
    }
    if game.game_status == GAME_CANCELLED {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Cant cancel game as it is already cancelled"),
        }));
    }

    GAME_DETAILS.save(
        deps.storage,
        game_id.clone(),
        &GameDetails {
            game_id: game_id.clone(),
            game_status: GAME_CANCELLED,
        },
    )?;

    // Get all pools
    let all_pools: Vec<String> = POOL_DETAILS
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for pool_id in all_pools {
        let pool;
        let pd = POOL_DETAILS.may_load(deps.storage, pool_id.clone())?;
        match pd {
            Some(pd) => {
                pool = pd;
            }
            None => {
                return Err(ContractError::Std(StdError::GenericErr {
                    msg: String::from("No pool details found for pool"),
                }));
            }
        };
        let pool_type;
        let ptd = POOL_TYPE_DETAILS.may_load(deps.storage, pool.pool_type.clone())?;
        match ptd {
            Some(ptd) => {
                pool_type = ptd;
            }
            None => {
                return Err(ContractError::Std(StdError::GenericErr {
                    msg: String::from("No pool type details found for pool"),
                }));
            }
        };
        let refund_amount = pool_type.pool_fee + platform_fee;

        // Get the existing teams for this pool
        let mut teams = Vec::new();
        let all_teams = POOL_TEAM_DETAILS.may_load(deps.storage, pool_id.clone())?;
        match all_teams {
            Some(some_teams) => {
                teams = some_teams;
            }
            None => {}
        }
        let mut updated_teams: Vec<PoolTeamDetails> = Vec::new();
        for team in teams {
            let mut gamer = team.gamer_address.clone();
            let gamer_addr = deps.api.addr_validate(&gamer)?;
            GAMING_FUNDS.update(
                deps.storage,
                &gamer_addr,
                |balance: Option<Uint128>| -> StdResult<_> {
                    Ok(balance.unwrap_or_default() - pool_type.pool_fee)
                },
            )?;

            // No transfer to be done to the gamers. Just update their refund amounts.
            // They have to come and collect their refund
            let mut updated_team = team.clone();
            updated_team.refund_amount = refund_amount;
            updated_team.claimed_refund = UNCLAIMED_REFUND;
            println!(
                "refund for {:?} is {:?}",
                team.team_id, updated_team.refund_amount
            );
            updated_teams.push(updated_team);
        }
        POOL_TEAM_DETAILS.save(deps.storage, pool_id.clone(), &updated_teams)?;
    }
    return Ok(Response::new()
        .add_attribute("game_id", game_id.clone())
        .add_attribute("game_status", "GAME_CANCELLED".to_string())
    ); 
}

fn lock_game(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    game_id: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized {
            invoker: info.sender.to_string(),
        });
    }
    let platform_fee = config.platform_fee;

    let gd = GAME_DETAILS.may_load(deps.storage, game_id.clone())?;
    let mut game;
    match gd {
        Some(gd) => {
            game = gd;
        }
        None => {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: String::from("Game status cannot be retrieved"),
            }));
        }
    }
    if game.game_status != GAME_POOL_OPEN {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Cant lock this game as it is not open for bidding"),
        }));
    }

    GAME_DETAILS.save(
        deps.storage,
        game_id.clone(),
        &GameDetails {
            game_id: game_id.clone(),
            game_status: GAME_POOL_CLOSED,
        },
    )?;

    // refund the gamers whose pool was not completed
    // Get all pools
    let all_pools: Vec<String> = POOL_DETAILS
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for pool_id in all_pools {
        let pool;
        let pd = POOL_DETAILS.may_load(deps.storage, pool_id.clone())?;
        match pd {
            Some(pd) => {
                pool = pd;
            }
            None => {
                return Err(ContractError::Std(StdError::GenericErr {
                    msg: String::from("No pool details found for pool"),
                }));
            }
        };
        let pool_type;
        let ptd = POOL_TYPE_DETAILS.may_load(deps.storage, pool.pool_type.clone())?;
        match ptd {
            Some(ptd) => {
                pool_type = ptd;
            }
            None => {
                return Err(ContractError::Std(StdError::GenericErr {
                    msg: String::from("No pool type details found for pool"),
                }));
            }
        };
        if pool.current_teams_count >= pool_type.min_teams_for_pool {
            continue;
        }
        let refund_amount = pool_type.pool_fee + platform_fee;

        // Get the existing teams for this pool
        let mut teams = Vec::new();
        let all_teams = POOL_TEAM_DETAILS.may_load(deps.storage, pool_id.clone())?;
        match all_teams {
            Some(some_teams) => {
                teams = some_teams;
            }
            None => {}
        }
        let mut updated_teams: Vec<PoolTeamDetails> = Vec::new();
        for team in teams {
            let mut gamer = team.gamer_address.clone();
            let gamer_addr = deps.api.addr_validate(&gamer)?;
            GAMING_FUNDS.update(
                deps.storage,
                &gamer_addr,
                |balance: Option<Uint128>| -> StdResult<_> {
                    Ok(balance.unwrap_or_default() - pool_type.pool_fee)
                },
            )?;

            // No transfer to be done to the gamers. Just update their refund amounts.
            // They have to come and collect their refund
            let mut updated_team = team.clone();
            updated_team.refund_amount = refund_amount;
            updated_team.claimed_refund = UNCLAIMED_REFUND;
            println!(
                "refund for {:?} is {:?}",
                team.team_id, updated_team.refund_amount
            );
            updated_teams.push(updated_team);
        }
        POOL_TEAM_DETAILS.save(deps.storage, pool_id.clone(), &updated_teams)?;
    }
    return Ok(Response::new()
        .add_attribute("game_id", game_id.clone())
        .add_attribute("game_status", "GAME_POOL_CLOSED".to_string())
    ); 
}

fn create_pool(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    game_id: String,
    pool_type: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized {
            invoker: info.sender.to_string(),
        });
    }

    let gd = GAME_DETAILS.may_load(deps.storage, game_id.clone())?;
    let mut game;
    match gd {
        Some(gd) => {
            game = gd;
        }
        None => {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: String::from("Game status cannot be retrieved"),
            }));
        }
    }
    if game.game_status != GAME_POOL_OPEN {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Game is not open for bidding"),
        }));
    }

    let dummy_wallet = String::from(DUMMY_WALLET);
    let address = deps.api.addr_validate(dummy_wallet.clone().as_str())?;
    let cpc = CONTRACT_POOL_COUNT.may_load(deps.storage, &address)?;
    let mut global_pool_id;
    match cpc {
        Some(cpc) => {
            global_pool_id = cpc;
        }
        None => {
            global_pool_id = Uint128::zero();
        }
    }
    let mut count = global_pool_id;
    CONTRACT_POOL_COUNT.update(
        deps.storage,
        &address,
        |global_pool_id: Option<Uint128>| -> StdResult<_> {
            Ok(global_pool_id.unwrap_or_default() + Uint128::from(1u128))
        },
    )?;
    count += Uint128::from(1u128);
    let pool_id_str: String = count.to_string();

    POOL_DETAILS.save(
        deps.storage,
        pool_id_str.clone(),
        &PoolDetails {
            game_id: game_id.clone(),
            pool_id: pool_id_str.clone(),
            pool_type: pool_type.clone(),
            current_teams_count: 0u32,
            rewards_distributed: REWARDS_NOT_DISTRIBUTED,
        },
    )?;
    return Ok(Response::new().add_attribute("pool_id", pool_id_str.clone()));
}

fn game_pool_bid_submit(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    gamer: String,
    pool_type: String,
    pool_id: String,
    game_id: String,
    team_id: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.minting_contract_address {
        return Err(ContractError::Unauthorized {
            invoker: info.sender.to_string(),
        });
    }
    let platform_fee = config.platform_fee;

    let gd = GAME_DETAILS.may_load(deps.storage, game_id.clone())?;
    let game;
    match gd {
        Some(gd) => {
            game = gd;
        }
        None => {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: String::from("Game status cannot be retrieved"),
            }));
        }
    }
    if game.game_status != GAME_POOL_OPEN {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Game is not open for bidding"),
        }));
    }

    let gamer_addr = deps.api.addr_validate(&gamer)?;

    let pool_type_details;
    let ptd = POOL_TYPE_DETAILS.may_load(deps.storage, pool_type.clone())?;
    match ptd {
        Some(ptd) => {
            pool_type_details = ptd;
        }
        None => {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: String::from("Cant get details for pool type "),
            }));
        }
    }
    let pool_fee = pool_type_details.pool_fee;
    let max_teams_for_pool = pool_type_details.max_teams_for_pool;
    let max_teams_for_gamer = pool_type_details.max_teams_for_gamer;

    if amount != pool_fee + platform_fee {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Amount being bid does not match the pool fee and the platform fee"),
        }));
    }

    let user_team_count;
    let uct = get_team_count_for_user_in_pool_type(
        deps.storage,
        gamer.clone(),
        game_id.clone(),
        pool_type.clone(),
    );
    match uct {
        Ok(uct) => {
            user_team_count = uct;
        }
        Err(e) => {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: String::from("Cant get user team count "),
            }));
        }
    }
    if user_team_count >= max_teams_for_gamer {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("User max team limit reached "),
        }));
    }

    let mut pool_id_return;
    let mut pool_details = query_pool_details(deps.storage, pool_id.clone())?;

    // check if the pool can accomodate the team
    if pool_details.current_teams_count < max_teams_for_pool {
        pool_id_return = pool_id.clone();
        pool_details.current_teams_count += 1;
        POOL_DETAILS.save(
            deps.storage,
            pool_id.clone(),
            &PoolDetails {
                pool_type: pool_type.clone(),
                pool_id: pool_id.clone(),
                game_id: pool_details.game_id.clone(),
                current_teams_count: pool_details.current_teams_count,
                rewards_distributed: pool_details.rewards_distributed,
            },
        )?;
        // Now save the team details
        save_team_details(
            deps.storage,
            env,
            gamer.clone(),
            pool_id.clone(),
            team_id.clone(),
            game_id.clone(),
            pool_type.clone(),
            Uint128::from(INITIAL_REWARD_AMOUNT),
            UNCLAIMED_REWARD,
            Uint128::from(INITIAL_REFUND_AMOUNT),
            UNCLAIMED_REFUND,
            INITIAL_TEAM_POINTS,
            INITIAL_TEAM_RANK,
        )?;
    } else {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("pool max team limit reached "),
        }));
    }
    GAMING_FUNDS.update(
        deps.storage,
        &gamer_addr,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + pool_fee) },
    )?;

    // Nothing required to transfer anything gaming fund has arrived in the gaming contract
    return Ok(Response::new().add_attribute("pool_id", pool_id_return.clone()));
}

fn save_team_details(
    storage: &mut dyn Storage,
    env: Env,
    gamer: String,
    pool_id: String,
    team_id: String,
    game_id: String,
    pool_type: String,
    reward_amount: Uint128,
    claimed_reward: bool,
    refund_amount: Uint128,
    claimed_refund: bool,
    team_points: u64,
    team_rank: u64,
) -> Result<Response, ContractError> {
    // Get the existing teams for this pool
    let mut teams = Vec::new();
    let all_teams = POOL_TEAM_DETAILS.may_load(storage, pool_id.clone())?;
    match all_teams {
        Some(some_teams) => {
            teams = some_teams;
        }
        None => {}
    }

    teams.push(PoolTeamDetails {
        gamer_address: gamer,
        game_id: game_id.clone(),
        pool_type: pool_type.clone(),
        pool_id: pool_id.clone(),
        team_id: team_id.clone(),
        reward_amount: reward_amount,
        claimed_reward: claimed_reward,
        refund_amount: refund_amount,
        claimed_refund: claimed_refund,
        team_points: team_points,
        team_rank: team_rank,
    });
    POOL_TEAM_DETAILS.save(storage, pool_id.clone(), &teams)?;

    return Ok(Response::new().add_attribute("team_id", team_id.clone()));
}

fn claim_reward(
    deps: DepsMut,
    info: MessageInfo,
    gamer: String,
) -> Result<Response, ContractError> {
    let gamer_addr = deps.api.addr_validate(&gamer)?;
    //Check if withdrawer is same as invoker
    if gamer_addr != info.sender {
        return Err(ContractError::Unauthorized {
            invoker: info.sender.to_string(),
        });
    }

    let mut user_reward = Uint128::zero();
    // Get all pools
    let all_pools: Vec<String> = POOL_DETAILS
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for pool_id in all_pools {
        // Get the existing teams for this pool
        let mut teams = Vec::new();
        let all_teams = POOL_TEAM_DETAILS.may_load(deps.storage, pool_id.clone())?;
        match all_teams {
            Some(some_teams) => {
                teams = some_teams;
            }
            None => {}
        }

        let existing_teams = teams.clone();
        let mut updated_teams = Vec::new();
        for team in existing_teams {
            let mut updated_team = team.clone();
            println!("Gamer {:?} gamer_address {:?} ", gamer, team.gamer_address);
            if gamer == team.gamer_address && team.claimed_reward == UNCLAIMED_REWARD {
                user_reward += team.reward_amount;
                updated_team.claimed_reward = CLAIMED_REWARD;
            }
            updated_teams.push(updated_team);
        }
        POOL_TEAM_DETAILS.save(deps.storage, pool_id.clone(), &updated_teams)?;
    }

	println!("reward amount is {:?}", user_reward);

	if user_reward == Uint128::zero() {
		return Err(ContractError::Std(StdError::GenericErr {
			msg: String::from("No reward for this user"),
		}));
	}

    // Do the transfer of reward to the actual gamer_addr from the contract
    transfer_from_contract_to_wallet(
        deps.storage,
        gamer.clone(),
        user_reward,
        "reward".to_string(),
    )
}

fn claim_refund(
    deps: DepsMut,
    info: MessageInfo,
    gamer: String,
) -> Result<Response, ContractError> {
    let gamer_addr = deps.api.addr_validate(&gamer)?;
    //Check if withdrawer is same as invoker
    if gamer_addr != info.sender {
        return Err(ContractError::Unauthorized {
            invoker: info.sender.to_string(),
        });
    }

    let mut user_refund = Uint128::zero();
    // Get all pools
    let all_pools: Vec<String> = POOL_DETAILS
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for pool_id in all_pools {
        // Get the existing teams for this pool
        let mut teams = Vec::new();
        let all_teams = POOL_TEAM_DETAILS.may_load(deps.storage, pool_id.clone())?;
        match all_teams {
            Some(some_teams) => {
                teams = some_teams;
            }
            None => {}
        }

        let existing_teams = teams.clone();
        let mut updated_teams = Vec::new();
        for team in existing_teams {
            let mut updated_team = team.clone();
            println!("Gamer {:?} gamer_address {:?} ", gamer, team.gamer_address);
            if gamer == team.gamer_address && team.claimed_refund == UNCLAIMED_REFUND {
                user_refund += team.refund_amount;
                updated_team.claimed_refund = CLAIMED_REFUND;
            }
            updated_teams.push(updated_team);
        }
        POOL_TEAM_DETAILS.save(deps.storage, pool_id.clone(), &updated_teams)?;
    }

	println!("refund amount is {:?}", user_refund);

	if user_refund == Uint128::zero() {
		return Err(ContractError::Std(StdError::GenericErr {
			msg: String::from("No refund for this user"),
		}));
	}

    // Do the transfer of refund to the actual gamer_addr from the contract
    transfer_from_contract_to_wallet(
        deps.storage,
        gamer.clone(),
        user_refund,
        "refund".to_string(),
    )
}

fn game_pool_reward_distribute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    game_id: String,
    pool_id: String,
    game_winners: Vec<GameResult>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin_address {
        return Err(ContractError::Unauthorized {
            invoker: info.sender.to_string(),
        });
    }
    let platform_fee = config.platform_fee;

    let gd = GAME_DETAILS.may_load(deps.storage, game_id.clone())?;
    let mut game;
    match gd {
        Some(gd) => {
            game = gd;
        }
        None => {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: String::from("Game status cannot be retrieved"),
            }));
        }
    }
    if game.game_status == GAME_CANCELLED {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Rewards cant be distributed as game is cancelled"),
        }));
    }
    if game.game_status == GAME_POOL_OPEN {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Rewards cant be distributed as game not yet started"),
        }));
    }
    GAME_DETAILS.save(
        deps.storage,
        game_id.clone(),
        &GameDetails {
            game_id: game_id.clone(),
            game_status: GAME_COMPLETED,
        },
    )?;

    let pool_details = query_pool_details(deps.storage, pool_id.clone())?;
	if pool_details.rewards_distributed == REWARDS_DISTRIBUTED {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Rewards are already distributed for this pool"),
        }));
	}	
    let pool_count = pool_details.current_teams_count;
    let pool_type = pool_details.pool_type;
    POOL_DETAILS.save(
        deps.storage,
        pool_id.clone(),
        &PoolDetails {
            game_id: game_id.clone(),
            pool_id: pool_id.clone(),
            pool_type: pool_type.clone(),
            current_teams_count: pool_details.current_teams_count,
            rewards_distributed: REWARDS_DISTRIBUTED,
        },
    )?;

    let pool_type_details;
    let ptd = POOL_TYPE_DETAILS.may_load(deps.storage, pool_type.clone())?;
    match ptd {
        Some(ptd) => {
            pool_type_details = ptd;
        }
        None => {
            return Err(ContractError::Std(StdError::GenericErr {
                msg: String::from("Cant get details for pool type"),
            }));
        }
    }

    let pool_fee = pool_type_details.pool_fee;
    let total_reward = pool_fee
        .checked_mul(Uint128::from(pool_count))
        .unwrap_or_default();

    let mut winner_rewards = Uint128::zero();
    let winners = game_winners.clone();
    for winner in winners {
        winner_rewards += winner.reward_amount;
    }
    if total_reward <= winner_rewards {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("reward amounts do not match"),
        }));
    }
    let rake_amount = total_reward - winner_rewards;
    println!(
        "total_reward {:?} winner_rewards {:?} rake_amount {:?}",
        total_reward, winner_rewards, rake_amount
    );

    let mut wallet_transfer_details: Vec<WalletTransferDetails> = Vec::new();

    let total_platform_fee = platform_fee
        .checked_mul(Uint128::from(pool_count))
        .unwrap_or_default();
    // Transfer total_platform_fee to platform wallets
    // These are the refund and development wallets
    let all_wallet_names: Vec<String> = PLATFORM_WALLET_PERCENTAGES
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for wallet_name in all_wallet_names {
        let wallet = PLATFORM_WALLET_PERCENTAGES.load(deps.storage, wallet_name.clone())?;
        let wallet_address = wallet.wallet_address;
        let mut proportionate_amount = total_platform_fee
            .checked_mul(Uint128::from(wallet.percentage))
            .unwrap_or_default()
            .checked_div(Uint128::from(100u128))
            .unwrap_or_default();
        // Transfer proportionate_amount to the corresponding platform wallet
        let transfer_detail = WalletTransferDetails {
            wallet_address: wallet_address.clone(),
            amount: proportionate_amount,
        };
        wallet_transfer_details.push(transfer_detail);
        println!(
            "transferring {:?} to {:?}",
            proportionate_amount,
            wallet_address.clone()
        );
    }

    // Get all teams for this pool
    let mut reward_given_so_far = Uint128::zero();
    let mut all_teams: Vec<PoolTeamDetails> = Vec::new();
    let ptd = POOL_TEAM_DETAILS.may_load(deps.storage, pool_id.clone())?;
    match ptd {
        Some(ptd) => {
            all_teams = ptd;
        }
        None => {}
    }
    let mut updated_teams: Vec<PoolTeamDetails> = Vec::new();
    for team in all_teams {
        // No transfer to be done to the winners. Just update their reward amounts.
        // They have to come and collect their rewards
        let mut updated_team = team.clone();
        let winners = game_winners.clone();
        for winner in winners {
            if team.gamer_address == winner.gamer_address
                && team.team_id == winner.team_id
                && team.game_id == winner.game_id
            {
                updated_team.reward_amount = winner.reward_amount;
                updated_team.team_rank = winner.team_rank;
                updated_team.team_points = winner.team_points;
                reward_given_so_far += winner.reward_amount;
                println!(
                    "reward for {:?} is {:?}",
                    team.team_id, updated_team.reward_amount
                );
            }
        }
        updated_teams.push(updated_team);
    }
    POOL_TEAM_DETAILS.save(deps.storage, pool_id.clone(), &updated_teams)?;

    // Transfer rake_amount to all the rake wallets. Can also be only one rake wallet
    for wallet in pool_type_details.rake_list {
        let mut wallet_address = wallet.wallet_address;
        let mut proportionate_amount = rake_amount
            .checked_mul(Uint128::from(wallet.percentage))
            .unwrap_or_default()
            .checked_div(Uint128::from(100u128))
            .unwrap_or_default();
        // Transfer proportionate_amount to the corresponding rake wallet
        let transfer_detail = WalletTransferDetails {
            wallet_address: wallet_address.clone(),
            amount: proportionate_amount,
        };
        wallet_transfer_details.push(transfer_detail);
        println!(
            "transferring {:?} to {:?}",
            proportionate_amount,
            wallet_address.clone()
        );
    }

    let rsp = transfer_to_multiple_wallets(
        deps.storage,
        wallet_transfer_details,
        "rake_and_platform_fee".to_string(),
    )?;
    return Ok(rsp
        .add_attribute("game_status", "GAME_COMPLETED".to_string())
        .add_attribute("game_id", game_id.clone())
        .add_attribute("pool_status", "POOL_REWARD_DISTRIBUTED".to_string())
        .add_attribute("pool_id", pool_id.clone())
    );
}

fn transfer_to_multiple_wallets(
    store: &dyn Storage,
    wallet_details: Vec<WalletTransferDetails>,
    action: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(store)?;
    let mut rsp = Response::new();
    for wallet in wallet_details {
        let transfer_msg = Cw20ExecuteMsg::Transfer {
            recipient: wallet.wallet_address,
            amount: wallet.amount,
        };
        let exec = WasmMsg::Execute {
            contract_addr: config.minting_contract_address.to_string(),
            msg: to_binary(&transfer_msg).unwrap(),
            funds: vec![],
        };
        let send: SubMsg = SubMsg::new(exec);
        rsp = rsp.add_submessage(send);
    }
    let data_msg = format!("Amount transferred").into_bytes();
    Ok(rsp
        .add_attribute("action", action)
        .set_data(data_msg))
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
        QueryMsg::PoolTeamDetails { pool_id } => {
            to_binary(&query_pool_team_details(deps.storage, pool_id)?)
        }
        QueryMsg::PoolDetails { pool_id } => to_binary(&query_pool_details(deps.storage, pool_id)?),
        QueryMsg::PoolTypeDetails { pool_type } => {
            to_binary(&query_pool_type_details(deps.storage, pool_type)?)
        }
        QueryMsg::AllTeams {} => to_binary(&query_all_teams(deps.storage)?),
        QueryMsg::QueryReward { gamer } => to_binary(&query_reward(deps.storage, gamer)?),
        QueryMsg::QueryGameResult {
            gamer,
            game_id,
            pool_id,
            team_id,
        } => to_binary(&query_game_result(deps, gamer, game_id, pool_id, team_id)?),
        QueryMsg::GameDetails { game_id } => to_binary(&query_game_details(deps.storage, game_id)?),
        QueryMsg::PoolTeamDetailsWithTeamId { pool_id, team_id } => {
            to_binary(&query_team_details(deps.storage, pool_id, team_id)?)
        }
        QueryMsg::AllPoolsInGame { game_id } => to_binary(&query_all_pools_in_game(deps.storage, game_id)?),
        QueryMsg::PoolCollection { game_id, pool_id } => to_binary(&query_pool_collection(deps.storage, game_id, pool_id)?),
    }
}

pub fn query_pool_type_details(
    storage: &dyn Storage,
    pool_type: String,
) -> StdResult<PoolTypeDetails> {
    let ptd = POOL_TYPE_DETAILS.may_load(storage, pool_type)?;
    match ptd {
        Some(ptd) => return Ok(ptd),
        None => return Err(StdError::generic_err("No pool type details found")),
    };
}

pub fn query_pool_team_details(
    storage: &dyn Storage,
    pool_id: String,
) -> StdResult<Vec<PoolTeamDetails>> {
    let ptd = POOL_TEAM_DETAILS.may_load(storage, pool_id)?;
    match ptd {
        Some(ptd) => return Ok(ptd),
        None => return Err(StdError::generic_err("No team details found")),
    };
}

fn query_all_teams(storage: &dyn Storage) -> StdResult<Vec<PoolTeamDetails>> {
    let mut all_teams = Vec::new();
    let all_pools: Vec<String> = POOL_DETAILS
        .keys(storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for pool_id in all_pools {
        let team_details = POOL_TEAM_DETAILS.load(storage, pool_id.clone())?;
        for team in team_details {
            all_teams.push(team);
        }
    }
    return Ok(all_teams);
}

fn query_reward(storage: &dyn Storage, gamer: String) -> StdResult<Uint128> {
    let mut user_reward = Uint128::zero();
    // Get all pools
    let all_pools: Vec<String> = POOL_DETAILS
        .keys(storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for pool_id in all_pools {
        // Get the existing teams for this pool
        let mut teams = Vec::new();
        let all_teams = POOL_TEAM_DETAILS.may_load(storage, pool_id.clone())?;
        match all_teams {
            Some(some_teams) => {
                teams = some_teams;
            }
            None => {}
        }
        for team in teams {
            if gamer == team.gamer_address && team.claimed_reward == UNCLAIMED_REWARD {
                user_reward += team.reward_amount;
            }
        }
    }
    return Ok(user_reward);
}

fn query_game_result(
    deps: Deps,
    gamer: String,
    game_id: String,
    pool_id: String,
    team_id: String,
) -> StdResult<GameResult> {
    let mut reward_amount = Uint128::zero();
    let mut refund_amount = Uint128::zero();
    let mut team_rank = INITIAL_TEAM_RANK;
    let mut team_points = INITIAL_TEAM_POINTS;

    let dummy_wallet = String::from(DUMMY_WALLET);
    let address = deps.api.addr_validate(dummy_wallet.clone().as_str())?;
    let grd = GAME_RESULT_DUMMY.may_load(deps.storage, &address)?;
    let mut game_result;
    match grd {
        Some(grd) => {
            game_result = grd;
        }
        None => return Err(StdError::generic_err("No game result details found")),
    }

    // Get the existing teams for this pool
    let mut teams = Vec::new();
    let all_teams = POOL_TEAM_DETAILS.may_load(deps.storage, pool_id.clone())?;
    match all_teams {
        Some(some_teams) => {
            teams = some_teams;
        }
        None => {}
    }
    for team in teams {
        if gamer == team.gamer_address
            && team_id == team.team_id
            && game_id == team.game_id
            && pool_id == team.pool_id
        {
            team_rank = team.team_rank;
            team_points = team.team_points;
            if team.claimed_reward == UNCLAIMED_REWARD {
                reward_amount += team.reward_amount;
            }
            if team.claimed_refund == UNCLAIMED_REFUND {
                refund_amount += team.refund_amount;
            }
        }
    }
    game_result.gamer_address = gamer.clone();
    game_result.game_id = game_id.clone();
    game_result.team_id = team_id.clone();
    game_result.team_rank = team_rank;
    game_result.team_points = team_points;
    game_result.reward_amount = reward_amount;
    game_result.refund_amount = refund_amount;
    return Ok(game_result);
}

fn query_pool_details(storage: &dyn Storage, pool_id: String) -> StdResult<PoolDetails> {
    let pd = POOL_DETAILS.may_load(storage, pool_id.clone())?;
    match pd {
        Some(pd) => return Ok(pd),
        None => return Err(StdError::generic_err("No pool details found")),
    };
}

fn get_team_count_for_user_in_pool_type(
    storage: &dyn Storage,
    gamer: String,
    game_id: String,
    pool_type: String,
) -> StdResult<u32> {
    let mut count = 0;
    let all_pools: Vec<String> = POOL_TEAM_DETAILS
        .keys(storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for pool_id in all_pools {
        let team_details = POOL_TEAM_DETAILS.load(storage, pool_id.clone())?;
        for team in team_details {
            if team.pool_type == pool_type && team.game_id == game_id && team.gamer_address == gamer
            {
                count += 1;
            }
        }
    }
    println!("Team count for user in given pool type : {:?}", count);
    return Ok(count);
}

fn query_game_details(storage: &dyn Storage, game_id: String) -> StdResult<GameDetails> {
    let gameDetail = GAME_DETAILS.may_load(storage, game_id)?;
    match gameDetail {
        Some(gameDetail) => return Ok(gameDetail),
        None => return Err(StdError::generic_err("Game detail found")),
    };
}

fn query_team_details(
    storage: &dyn Storage,
    pool_id: String,
    team_id: String,
) -> StdResult<PoolTeamDetails> {
    let team_details = POOL_TEAM_DETAILS.load(storage, pool_id.clone())?;
    for team in team_details {
        if team.team_id == team_id.to_string() {
            return Ok(team.clone());
        }
    }
    return Err(StdError::generic_err("Pool Team Details not found"));
}

fn query_all_pools_in_game(
    storage: &dyn Storage,
    game_id: String,
) -> StdResult<Vec<PoolDetails>> {
    let mut all_pool_details = Vec::new();
    let all_pools: Vec<String> = POOL_DETAILS
        .keys(storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for pool_name in all_pools {
        let pool_details = POOL_DETAILS.load(storage, pool_name)?;
        if pool_details.game_id == game_id {
            all_pool_details.push(pool_details);
        }
    }
    return Ok(all_pool_details);
}

fn query_pool_collection(
    storage: &dyn Storage,
    game_id: String,
    pool_id: String,
) -> StdResult<Uint128> {
    let pd = POOL_DETAILS.may_load(storage, pool_id.clone())?;
    let pool;
    match pd {
        Some(pd) => { pool = pd }
        None => return Err(StdError::generic_err("No pool details found")),
    };

    let ptd = POOL_TYPE_DETAILS.may_load(storage, pool.pool_type.clone())?;
    let pool_type;
    match ptd {
        Some(ptd) => {
            pool_type = ptd;
        }
        None => return Err(StdError::generic_err("No pool type details found")),
    };

    let pool_collection = pool_type.pool_fee
        .checked_mul(Uint128::from(pool.current_teams_count))
        .unwrap_or_default();
    return Ok(pool_collection);
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Addr, CosmosMsg, StdError, SubMsg, WasmMsg};

    use super::*;
    use crate::msg::InstantiateMarketingInfo;

    use cosmwasm_std::coin;

    #[test]
    fn test_create_and_query_game() {
        let mut deps = mock_dependencies(&[]);
        let platform_fee = Uint128::from(300000u128);

        let owner1Info = mock_info("Owner001", &[coin(1000, "stake")]);
        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );
        let queryRes = query_game_details(&mut deps.storage, "Game001".to_string());
        match queryRes {
            Ok(gameDetail) => {
                assert_eq!(gameDetail.game_id, "Game001".to_string());
                assert_eq!(gameDetail.game_status, GAME_POOL_OPEN);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
    }

    #[test]
    fn test_create_and_query_pool_detail() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Owner001", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);
        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        let rsp = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToOne".to_string(),
        );
        let mut poolId = String::new();

        match rsp {
            Ok(rsp) => {
                poolId = rsp.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let queryRes = query_pool_details(&mut deps.storage, poolId);
        match queryRes {
            Ok(poolDetail) => {
                assert_eq!(poolDetail.game_id, "Game001".to_string());
                assert_eq!(poolDetail.pool_type, "oneToOne".to_string());
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(3, 4);
            }
        }
    }
    #[test]
    fn test_save_and_query_team_detail() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Owner001", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);
        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        let rsp = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToOne".to_string(),
        );
        let mut poolId = String::new();

        match rsp {
            Ok(rsp) => {
                poolId = rsp.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let rsp_save_team = save_team_details(
            &mut deps.storage,
            mock_env(),
            "Gamer001".to_string(),
            poolId.to_string(),
            "Team001".to_string(),
            "Game001".to_string(),
            "oneToOne".to_string(),
            Uint128::from(144262u128),
            false,
            Uint128::from(0u128),
            false,
            100,
            2,
        );

        let mut teamId = String::new();

        match rsp_save_team {
            Ok(rsp_save_team) => {
                teamId = rsp_save_team.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(2, 3);
            }
        }

        let queryRes =
            query_team_details(&mut deps.storage, poolId.to_string(), teamId.to_string());
        match queryRes {
            Ok(poolTeamDetail) => {
                assert_eq!(poolTeamDetail.pool_id, poolId.to_string());
                //assert_eq!(gameDetail.game_status, GAME_POOL_OPEN);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(3, 4);
            }
        }
    }

    #[test]
    fn test_get_team_count_for_user_in_pool_type() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Owner001", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);
        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        let rsp = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToOne".to_string(),
        );
        let mut poolId = String::new();

        match rsp {
            Ok(rsp) => {
                poolId = rsp.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let rsp_save_team_1 = save_team_details(
            &mut deps.storage,
            mock_env(),
            "Gamer001".to_string(),
            poolId.to_string(),
            "Team001".to_string(),
            "Game001".to_string(),
            "oneToOne".to_string(),
            Uint128::from(144262u128),
            false,
            Uint128::from(0u128),
            false,
            100,
            2,
        );
        let rsp_save_team_2 = save_team_details(
            &mut deps.storage,
            mock_env(),
            "Gamer001".to_string(),
            poolId.to_string(),
            "Team002".to_string(),
            "Game001".to_string(),
            "oneToOne".to_string(),
            Uint128::from(144262u128),
            false,
            Uint128::from(0u128),
            false,
            100,
            2,
        );
        let rsp_save_team_3 = save_team_details(
            &mut deps.storage,
            mock_env(),
            "Gamer001".to_string(),
            poolId.to_string(),
            "Team003".to_string(),
            "Game001".to_string(),
            "oneToOne".to_string(),
            Uint128::from(144262u128),
            false,
            Uint128::from(0u128),
            false,
            100,
            2,
        );

        let team_count = get_team_count_for_user_in_pool_type(
            &mut deps.storage,
            "Gamer001".to_string(),
            "Game001".to_string(),
            "oneToOne".to_string(),
        );

        match team_count {
            Ok(team_count) => {
                assert_eq!(team_count, 3);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(2, 3);
            }
        }
    }

    #[test]
    fn test_query_all_teams() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Owner001", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);
        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        let mut rsp = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToOne".to_string(),
        );
        let mut poolId = String::new();

        match rsp {
            Ok(rsp) => {
                poolId = rsp.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let rsp_save_team_1 = save_team_details(
            &mut deps.storage,
            mock_env(),
            "Gamer001".to_string(),
            poolId.to_string(),
            "Team001".to_string(),
            "Game001".to_string(),
            "oneToOne".to_string(),
            Uint128::from(144262u128),
            false,
            Uint128::from(0u128),
            false,
            100,
            2,
        );
        let rsp_save_team_2 = save_team_details(
            &mut deps.storage,
            mock_env(),
            "Gamer001".to_string(),
            poolId.to_string(),
            "Team002".to_string(),
            "Game001".to_string(),
            "oneToOne".to_string(),
            Uint128::from(144262u128),
            false,
            Uint128::from(0u128),
            false,
            100,
            2,
        );
        let rsp_save_team_3 = save_team_details(
            &mut deps.storage,
            mock_env(),
            "Gamer001".to_string(),
            poolId.to_string(),
            "Team003".to_string(),
            "Game001".to_string(),
            "oneToOne".to_string(),
            Uint128::from(144262u128),
            false,
            Uint128::from(0u128),
            false,
            100,
            2,
        );

        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game002".to_string(),
        );
        rsp = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game002".to_string(),
            "many".to_string(),
        );
        match rsp {
            Ok(rsp) => {
                poolId = rsp.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let rsp_save_team_4 = save_team_details(
            &mut deps.storage,
            mock_env(),
            "Gamer001".to_string(),
            poolId.to_string(),
            "Team004".to_string(),
            "Game002".to_string(),
            "many".to_string(),
            Uint128::from(144262u128),
            false,
            Uint128::from(0u128),
            false,
            100,
            2,
        );
        let rsp_save_team_5 = save_team_details(
            &mut deps.storage,
            mock_env(),
            "Gamer001".to_string(),
            poolId.to_string(),
            "Team005".to_string(),
            "Game002".to_string(),
            "many".to_string(),
            Uint128::from(144262u128),
            false,
            Uint128::from(0u128),
            false,
            100,
            2,
        );
        let rsp_save_team_6 = save_team_details(
            &mut deps.storage,
            mock_env(),
            "Gamer001".to_string(),
            poolId.to_string(),
            "Team006".to_string(),
            "Game002".to_string(),
            "many".to_string(),
            Uint128::from(144262u128),
            false,
            Uint128::from(0u128),
            false,
            100,
            2,
        );

        let team_count = query_all_teams(&mut deps.storage);

        match team_count {
            Ok(team_count) => {
                assert_eq!(team_count.len(), 6);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
    }

    #[test]
    fn test_game_pool_bid_submit_when_pool_team_in_range() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Gamer001", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);

        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        let mut rake_list: Vec<WalletPercentage> = Vec::new();

        let rake_1 = WalletPercentage {
            wallet_address: "rake_1".to_string(),
            wallet_name: "rake_1".to_string(),
            percentage: 1u32,
        };
        rake_list.push(rake_1);
        let rake_2 = WalletPercentage {
            wallet_address: "rake_2".to_string(),
            wallet_name: "rake_2".to_string(),
            percentage: 2u32,
        };
        rake_list.push(rake_2);

        let rake_3 = WalletPercentage {
            wallet_address: "rake_3".to_string(),
            wallet_name: "rake_3".to_string(),
            percentage: 3u32,
        };
        rake_list.push(rake_3);

        set_pool_type_params(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "oneToOne".to_string(),
            Uint128::from(144262u128),
            2,
            10,
            2,
            rake_list,
        );

        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        let rsp = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToOne".to_string(),
        );
        let mut poolId = String::new();

        match rsp {
            Ok(rsp) => {
                poolId = rsp.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let rewardInfo = mock_info("rewardInfo", &[]);
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            owner1Info.clone(),
            "Gamer001".to_string(),
            "oneToOne".to_string(),
            poolId.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        let queryRes = query_pool_details(&mut deps.storage, "1".to_string());
        match queryRes {
            Ok(poolDetail) => {
                assert_eq!(poolDetail.pool_id, "1".to_string());
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(3, 4);
            }
        }
    }

    #[test]
    fn test_game_pool_bid_submit_when_pool_team_not_in_range() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Gamer001", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);

        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        let mut rake_list: Vec<WalletPercentage> = Vec::new();

        let rake_1 = WalletPercentage {
            wallet_address: "rake_1".to_string(),
            wallet_name: "rake_1".to_string(),
            percentage: 1u32,
        };
        rake_list.push(rake_1);
        let rake_2 = WalletPercentage {
            wallet_address: "rake_2".to_string(),
            wallet_name: "rake_2".to_string(),
            percentage: 2u32,
        };
        rake_list.push(rake_2);

        let rake_3 = WalletPercentage {
            wallet_address: "rake_3".to_string(),
            wallet_name: "rake_3".to_string(),
            percentage: 3u32,
        };
        rake_list.push(rake_3);

        set_pool_type_params(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "oneToOne".to_string(),
            Uint128::from(144262u128),
            1,
            1,
            1,
            rake_list,
        );

        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        let rsp = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToOne".to_string(),
        );
        let mut poolId = String::new();

        match rsp {
            Ok(rsp) => {
                poolId = rsp.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let rewardInfo = mock_info("rewardInfo", &[]);
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            owner1Info.clone(),
            "Gamer001".to_string(),
            "oneToOne".to_string(),
            poolId.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            owner1Info.clone(),
            "Gamer001".to_string(),
            "oneToOne".to_string(),
            poolId.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        let queryRes = query_pool_details(&mut deps.storage, "2".to_string());
        match queryRes {
            Ok(poolDetail) => {
                // there should not be any pool with id 2
                assert_eq!(1, 2);
            }
            Err(e) => {
                // there should not be any pool with id 2
                assert_eq!(1, 1);
            }
        }
    }

    #[test]
    fn test_crete_different_pool_type_and_add_multiple_game_for_given_user() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Gamer001", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);

        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        let mut rake_list: Vec<WalletPercentage> = Vec::new();

        let rake_1 = WalletPercentage {
            wallet_address: "rake_1".to_string(),
            wallet_name: "rake_1".to_string(),
            percentage: 1u32,
        };
        rake_list.push(rake_1);
        let rake_2 = WalletPercentage {
            wallet_address: "rake_2".to_string(),
            wallet_name: "rake_2".to_string(),
            percentage: 2u32,
        };
        rake_list.push(rake_2);

        let rake_3 = WalletPercentage {
            wallet_address: "rake_3".to_string(),
            wallet_name: "rake_3".to_string(),
            percentage: 3u32,
        };
        rake_list.push(rake_3);

        set_pool_type_params(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "oneToOne".to_string(),
            Uint128::from(144262u128),
            2,
            10,
            10,
            rake_list.clone(),
        );
        set_pool_type_params(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "multiple".to_string(),
            Uint128::from(144262u128),
            2,
            10,
            10,
            rake_list.clone(),
        );

        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        // create multiple pool
        let mut pool_id_1 = String::new();
        let rsp_1 = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToOne".to_string(),
        );
        match rsp_1 {
            Ok(rsp_1) => {
                pool_id_1 = rsp_1.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let mut pool_id_2 = String::new();
        let rsp_2 = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "multiple".to_string(),
        );
        match rsp_2 {
            Ok(rsp_2) => {
                pool_id_2 = rsp_2.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(2, 3);
            }
        }

        // create  pool with same pool type as in pool_id_1
        let mut pool_id_3 = String::new();
        let rsp_3 = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToOne".to_string(),
        );
        match rsp_3 {
            Ok(rsp_3) => {
                pool_id_3 = rsp_3.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(3, 4);
            }
        }

        let rewardInfo = mock_info("rewardInfo", &[]);
        // Adding multile team to pool_1 for Game001
        let ownerXInfo = mock_info("cwtoken11111", &[coin(1000, "stake")]);
        // Adding same team twice in same pool
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer001".to_string(),
            "oneToOne".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer001".to_string(),
            "oneToOne".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer001".to_string(),
            "oneToOne".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team002".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer001".to_string(),
            "oneToOne".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team003".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );

        let query_pool_details_1 = query_pool_details(&mut deps.storage, pool_id_1.to_string());
        match query_pool_details_1 {
            Ok(pool_detail_1) => {
                assert_eq!(pool_detail_1.current_teams_count, 4u32);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(4, 5);
            }
        }

        // Adding multile team to pool_2 for Game001. some of team is already added in pool_1
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer001".to_string(),
            "multiple".to_string(),
            pool_id_2.to_string(),
            "Game001".to_string(),
            "Team003".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer001".to_string(),
            "multiple".to_string(),
            pool_id_2.to_string(),
            "Game001".to_string(),
            "Team004".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer001".to_string(),
            "multiple".to_string(),
            pool_id_2.to_string(),
            "Game001".to_string(),
            "Team005".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );

        let query_pool_details_2 = query_pool_details(&mut deps.storage, pool_id_2.to_string());
        match query_pool_details_2 {
            Ok(pool_detail_2) => {
                assert_eq!(pool_detail_2.current_teams_count, 3u32);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(5, 6);
            }
        }

        // Adding same team to another pool of same pool type
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer001".to_string(),
            "oneToOne".to_string(),
            pool_id_3.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer001".to_string(),
            "multiple".to_string(),
            pool_id_3.to_string(),
            "Game001".to_string(),
            "Team004".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        let query_pool_details_3 = query_pool_details(&mut deps.storage, pool_id_3.to_string());
        match query_pool_details_3 {
            Ok(pool_detail_3) => {
                assert_eq!(pool_detail_3.current_teams_count, 2u32);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(6, 7);
            }
        }
    }

    #[test]
    fn test_max_team_per_pool_type_for_given_user() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Gamer002", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);

        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        let mut rake_list: Vec<WalletPercentage> = Vec::new();
        let rake_1 = WalletPercentage {
            wallet_address: "rake_1".to_string(),
            wallet_name: "rake_1".to_string(),
            percentage: 1u32,
        };
        rake_list.push(rake_1);
        let rake_2 = WalletPercentage {
            wallet_address: "rake_2".to_string(),
            wallet_name: "rake_2".to_string(),
            percentage: 2u32,
        };
        rake_list.push(rake_2);

        let rake_3 = WalletPercentage {
            wallet_address: "rake_3".to_string(),
            wallet_name: "rake_3".to_string(),
            percentage: 3u32,
        };
        rake_list.push(rake_3);

        set_pool_type_params(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "oneToTwo".to_string(),
            Uint128::from(144262u128),
            2,
            10,
            2,
            rake_list.clone(),
        );
        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        // create multiple pool
        let mut pool_id_1 = String::new();
        let rsp_1 = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToTwo".to_string(),
        );
        match rsp_1 {
            Ok(rsp_1) => {
                pool_id_1 = rsp_1.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
        let rewardInfo = mock_info("rewardInfo", &[]);
        let ownerXInfo = mock_info("cwtoken11111", &[coin(1000, "stake")]);
        // Adding same team twice in same pool
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team002".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team003".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );

        let query_pool_details_1 = query_pool_details(&mut deps.storage, pool_id_1.to_string());
        match query_pool_details_1 {
            Ok(pool_detail_1) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                assert_eq!(pool_detail_1.current_teams_count, 2u32);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(2, 3);
            }
        }
    }

    #[test]
    fn test_game_pool_reward_distribute() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Gamer002", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);

        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        let mut rake_list: Vec<WalletPercentage> = Vec::new();
        let rake_1 = WalletPercentage {
            wallet_address: "rake_1".to_string(),
            wallet_name: "rake_1".to_string(),
            percentage: 1u32,
        };
        rake_list.push(rake_1);
        let rake_2 = WalletPercentage {
            wallet_address: "rake_2".to_string(),
            wallet_name: "rake_2".to_string(),
            percentage: 2u32,
        };
        rake_list.push(rake_2);

        let rake_3 = WalletPercentage {
            wallet_address: "rake_3".to_string(),
            wallet_name: "rake_3".to_string(),
            percentage: 3u32,
        };
        rake_list.push(rake_3);

        set_pool_type_params(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "oneToTwo".to_string(),
            Uint128::from(144262u128),
            2,
            10,
            5,
            rake_list.clone(),
        );
        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        // create multiple pool
        let mut pool_id_1 = String::new();
        let rsp_1 = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToTwo".to_string(),
        );
        match rsp_1 {
            Ok(rsp_1) => {
                pool_id_1 = rsp_1.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
        let rewardInfo = mock_info("rewardInfo", &[]);
        let ownerXInfo = mock_info("cwtoken11111", &[coin(1000, "stake")]);
        // Adding same team twice in same pool
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team002".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team003".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );

        let query_pool_details_1 = query_pool_details(&mut deps.storage, pool_id_1.to_string());
        match query_pool_details_1 {
            Ok(pool_detail_1) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                assert_eq!(pool_detail_1.current_teams_count, 3u32);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(2, 3);
            }
        }

        let game_result_1 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team001".to_string(),
            team_rank: 1u64,
            team_points: 100u64,
            reward_amount: Uint128::from(100u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_2 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team002".to_string(),
            team_rank: 2u64,
            team_points: 200u64,
            reward_amount: Uint128::from(200u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_3 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team003".to_string(),
            team_rank: 2u64,
            team_points: 300u64,
            reward_amount: Uint128::from(300u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let mut game_results: Vec<GameResult> = Vec::new();
        game_results.push(game_result_1);
        game_results.push(game_result_2);
        game_results.push(game_result_3);

        let lock_game_rsp = lock_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );
        match lock_game_rsp {
            Ok(lock_game_rsp) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                //assert_eq!(pool_detail_1.current_teams_count, 3u32);
                assert_eq!(
                    lock_game_rsp.attributes[1].value.clone(),
                    "GAME_POOL_CLOSED".to_string()
                );
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(3, 4);
            }
        }

        let game_pool_reward_distribute_rsp = game_pool_reward_distribute(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            pool_id_1.to_string(),
            game_results,
        );

        match game_pool_reward_distribute_rsp {
            Ok(game_pool_reward_distribute_rsp) => {}
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(4, 5);
            }
        }

        let query_game_status_res = query_game_details(&mut deps.storage, "Game001".to_string());
        match query_game_status_res {
            Ok(query_game_status_res) => {
                assert_eq!(query_game_status_res.game_status, GAME_COMPLETED);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
        let team_details = POOL_TEAM_DETAILS.load(&mut deps.storage, pool_id_1.clone());
        for team in team_details {
            assert_eq!(team[0].reward_amount, Uint128::from(100u128));
            assert_eq!(team[1].reward_amount, Uint128::from(200u128));
            assert_eq!(team[2].reward_amount, Uint128::from(300u128));
        }
    }

    #[test]
    fn test_claim_refund() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Gamer002", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);

        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        let mut rake_list: Vec<WalletPercentage> = Vec::new();
        let rake_1 = WalletPercentage {
            wallet_address: "rake_1".to_string(),
            wallet_name: "rake_1".to_string(),
            percentage: 1u32,
        };
        rake_list.push(rake_1);
        let rake_2 = WalletPercentage {
            wallet_address: "rake_2".to_string(),
            wallet_name: "rake_2".to_string(),
            percentage: 2u32,
        };
        rake_list.push(rake_2);

        let rake_3 = WalletPercentage {
            wallet_address: "rake_3".to_string(),
            wallet_name: "rake_3".to_string(),
            percentage: 3u32,
        };
        rake_list.push(rake_3);

        set_pool_type_params(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "oneToTwo".to_string(),
            Uint128::from(144262u128),
            2,
            10,
            5,
            rake_list.clone(),
        );
        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        // create multiple pool
        let mut pool_id_1 = String::new();
        let rsp_1 = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToTwo".to_string(),
        );
        match rsp_1 {
            Ok(rsp_1) => {
                pool_id_1 = rsp_1.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
        let rewardInfo = mock_info("rewardInfo", &[]);
        let ownerXInfo = mock_info("cwtoken11111", &[coin(1000, "stake")]);
        // Adding same team twice in same pool
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );

        let cancelInfo = mock_info("cancelInfo", &[]);
        let cancel_rsp = cancel_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        let claim_refund_rsp = claim_refund(deps.as_mut(), owner1Info.clone(), "Gamer002".to_string());
        match claim_refund_rsp {
            Ok(claim_refund_rsp) => {
                let amt = claim_refund_rsp.attributes[0].value.clone();
				let expamt = Uint128::from(144262u128) + platform_fee;
				let expamtStr = expamt.to_string();
                assert_eq!(amt, expamtStr);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(5, 6);
            }
        }
    }

    #[test]
    fn test_cancel_game() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Gamer002", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);

        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        let mut rake_list: Vec<WalletPercentage> = Vec::new();
        let rake_1 = WalletPercentage {
            wallet_address: "rake_1".to_string(),
            wallet_name: "rake_1".to_string(),
            percentage: 1u32,
        };
        rake_list.push(rake_1);
        let rake_2 = WalletPercentage {
            wallet_address: "rake_2".to_string(),
            wallet_name: "rake_2".to_string(),
            percentage: 2u32,
        };
        rake_list.push(rake_2);

        let rake_3 = WalletPercentage {
            wallet_address: "rake_3".to_string(),
            wallet_name: "rake_3".to_string(),
            percentage: 3u32,
        };
        rake_list.push(rake_3);

        set_pool_type_params(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "oneToTwo".to_string(),
            Uint128::from(144262u128),
            2,
            10,
            5,
            rake_list.clone(),
        );
        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        // create multiple pool
        let mut pool_id_1 = String::new();
        let rsp_1 = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToTwo".to_string(),
        );
        match rsp_1 {
            Ok(rsp_1) => {
                pool_id_1 = rsp_1.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
        let rewardInfo = mock_info("rewardInfo", &[]);
        let ownerXInfo = mock_info("cwtoken11111", &[coin(1000, "stake")]);
        // Adding same team twice in same pool
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team002".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team003".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );

        let query_pool_details_1 = query_pool_details(&mut deps.storage, pool_id_1.to_string());
        match query_pool_details_1 {
            Ok(pool_detail_1) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                assert_eq!(pool_detail_1.current_teams_count, 3u32);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(2, 3);
            }
        }

        let game_result_1 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team001".to_string(),
            team_rank: 1u64,
            team_points: 100u64,
            reward_amount: Uint128::from(100u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_2 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team002".to_string(),
            team_rank: 2u64,
            team_points: 200u64,
            reward_amount: Uint128::from(200u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_3 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team003".to_string(),
            team_rank: 2u64,
            team_points: 300u64,
            reward_amount: Uint128::from(300u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let mut game_results: Vec<GameResult> = Vec::new();
        game_results.push(game_result_1);
        game_results.push(game_result_2);
        game_results.push(game_result_3);

        let lock_game_rsp = lock_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );
        match lock_game_rsp {
            Ok(lock_game_rsp) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                //assert_eq!(pool_detail_1.current_teams_count, 3u32);
                assert_eq!(
                    lock_game_rsp.attributes[1].value.clone(),
                    "GAME_POOL_CLOSED".to_string()
                );
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(3, 4);
            }
        }

        let cancelInfo = mock_info("cancelInfo", &[]);
        let game_pool_reward_distribute_rsp = cancel_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        match game_pool_reward_distribute_rsp {
            Ok(game_pool_reward_distribute_rsp) => {}
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(4, 5);
            }
        }

        let query_game_status_res = query_game_details(&mut deps.storage, "Game001".to_string());
        match query_game_status_res {
            Ok(query_game_status_res) => {
                assert_eq!(query_game_status_res.game_status, GAME_CANCELLED);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(5, 6);
            }
        }
        let team_details = POOL_TEAM_DETAILS.load(&mut deps.storage, pool_id_1.clone());
        for team in team_details {
            assert_eq!(team[0].reward_amount, Uint128::zero());
            assert_eq!(team[1].reward_amount, Uint128::zero());
            assert_eq!(team[2].reward_amount, Uint128::zero());
        }
    }

    #[test]
    fn test_claim_reward() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Gamer002", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);

        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        let mut rake_list: Vec<WalletPercentage> = Vec::new();
        let rake_1 = WalletPercentage {
            wallet_address: "rake_1".to_string(),
            wallet_name: "rake_1".to_string(),
            percentage: 1u32,
        };
        rake_list.push(rake_1);
        let rake_2 = WalletPercentage {
            wallet_address: "rake_2".to_string(),
            wallet_name: "rake_2".to_string(),
            percentage: 2u32,
        };
        rake_list.push(rake_2);

        let rake_3 = WalletPercentage {
            wallet_address: "rake_3".to_string(),
            wallet_name: "rake_3".to_string(),
            percentage: 3u32,
        };
        rake_list.push(rake_3);

        set_pool_type_params(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "oneToTwo".to_string(),
            Uint128::from(144262u128),
            2,
            10,
            5,
            rake_list.clone(),
        );
        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        // create multiple pool
        let mut pool_id_1 = String::new();
        let rsp_1 = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToTwo".to_string(),
        );
        match rsp_1 {
            Ok(rsp_1) => {
                pool_id_1 = rsp_1.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
        let platform_fee = Uint128::from(300000u128);
        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let rewardInfo = mock_info("rewardInfo", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            rewardInfo.clone(),
            instantiate_msg,
        );
        let ownerXInfo = mock_info("cwtoken11111", &[coin(1000, "stake")]);
        // Adding same team twice in same pool
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team002".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team003".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );

        let query_pool_details_1 = query_pool_details(&mut deps.storage, pool_id_1.to_string());
        match query_pool_details_1 {
            Ok(pool_detail_1) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                assert_eq!(pool_detail_1.current_teams_count, 3u32);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(2, 3);
            }
        }

        let game_result_1 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team001".to_string(),
            team_rank: 1u64,
            team_points: 100u64,
            reward_amount: Uint128::from(100u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_2 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team002".to_string(),
            team_rank: 2u64,
            team_points: 200u64,
            reward_amount: Uint128::from(200u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_3 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team003".to_string(),
            team_rank: 2u64,
            team_points: 300u64,
            reward_amount: Uint128::from(300u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let mut game_results: Vec<GameResult> = Vec::new();
        game_results.push(game_result_1);
        game_results.push(game_result_2);
        game_results.push(game_result_3);

        let lock_game_rsp = lock_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );
        match lock_game_rsp {
            Ok(lock_game_rsp) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                //assert_eq!(pool_detail_1.current_teams_count, 3u32);
                assert_eq!(
                    lock_game_rsp.attributes[1].value.clone(),
                    "GAME_POOL_CLOSED".to_string()
                );
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(3, 4);
            }
        }

        let game_pool_reward_distribute_rsp = game_pool_reward_distribute(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            pool_id_1.to_string(),
            game_results,
        );

        match game_pool_reward_distribute_rsp {
            Ok(game_pool_reward_distribute_rsp) => {}
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(4, 5);
            }
        }

        let mut query_game_status_res =
            query_game_details(&mut deps.storage, "Game001".to_string());
        match query_game_status_res {
            Ok(query_game_status_res) => {
                assert_eq!(query_game_status_res.game_status, GAME_COMPLETED);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(5, 6);
            }
        }
        let team_details = POOL_TEAM_DETAILS.load(&mut deps.storage, pool_id_1.clone());
        for team in team_details {
            assert_eq!(team[0].reward_amount, Uint128::from(100u128));
            assert_eq!(team[1].reward_amount, Uint128::from(200u128));
            assert_eq!(team[2].reward_amount, Uint128::from(300u128));
        }

        let claim_reward_rsp =
            claim_reward(deps.as_mut(), owner1Info.clone(), "Gamer002".to_string());
        match claim_reward_rsp {
            Ok(claim_reward_rsp) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                //assert_eq!(pool_detail_1.current_teams_count, 3u32);
                assert_eq!(
                    claim_reward_rsp.attributes[0].value.clone(),
                    "600".to_string()
                );
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(6, 7);
            }
        }
        query_game_status_res = query_game_details(&mut deps.storage, "Game001".to_string());
        match query_game_status_res {
            Ok(query_game_status_res) => {
                assert_eq!(query_game_status_res.game_status, GAME_COMPLETED);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(7, 8);
            }
        }

        let team_details = POOL_TEAM_DETAILS.load(&mut deps.storage, pool_id_1.clone());
        for team in team_details {
            assert_eq!(team[0].reward_amount, Uint128::from(100u128)); // TODO This reward should be 0 after full functionality working.
            assert_eq!(team[1].reward_amount, Uint128::from(200u128)); // TODO This reward should be 0 after full functionality working.
            assert_eq!(team[2].reward_amount, Uint128::from(300u128)); // TODO This reward should be 0 after full functionality working.
            assert_eq!(team[0].claimed_reward, CLAIMED_REWARD);
            assert_eq!(team[1].claimed_reward, CLAIMED_REWARD);
            assert_eq!(team[2].claimed_reward, CLAIMED_REWARD);
        }
    }

    #[test]
    fn test_claim_reward_twice() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Gamer002", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);

        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        let mut rake_list: Vec<WalletPercentage> = Vec::new();
        let rake_1 = WalletPercentage {
            wallet_address: "rake_1".to_string(),
            wallet_name: "rake_1".to_string(),
            percentage: 1u32,
        };
        rake_list.push(rake_1);
        let rake_2 = WalletPercentage {
            wallet_address: "rake_2".to_string(),
            wallet_name: "rake_2".to_string(),
            percentage: 2u32,
        };
        rake_list.push(rake_2);

        let rake_3 = WalletPercentage {
            wallet_address: "rake_3".to_string(),
            wallet_name: "rake_3".to_string(),
            percentage: 3u32,
        };
        rake_list.push(rake_3);

        set_pool_type_params(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "oneToTwo".to_string(),
            Uint128::from(144262u128),
            2,
            10,
            5,
            rake_list.clone(),
        );
        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        // create multiple pool
        let mut pool_id_1 = String::new();
        let rsp_1 = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToTwo".to_string(),
        );
        match rsp_1 {
            Ok(rsp_1) => {
                pool_id_1 = rsp_1.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
        let rewardInfo = mock_info("rewardInfo", &[]);
        let ownerXInfo = mock_info("cwtoken11111", &[coin(1000, "stake")]);
        // Adding same team twice in same pool
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team002".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team003".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );

        let query_pool_details_1 = query_pool_details(&mut deps.storage, pool_id_1.to_string());
        match query_pool_details_1 {
            Ok(pool_detail_1) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                assert_eq!(pool_detail_1.current_teams_count, 3u32);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(2, 3);
            }
        }

        let game_result_1 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team001".to_string(),
            team_rank: 1u64,
            team_points: 100u64,
            reward_amount: Uint128::from(100u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_2 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team002".to_string(),
            team_rank: 2u64,
            team_points: 200u64,
            reward_amount: Uint128::from(200u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_3 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team003".to_string(),
            team_rank: 2u64,
            team_points: 300u64,
            reward_amount: Uint128::from(300u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let mut game_results: Vec<GameResult> = Vec::new();
        game_results.push(game_result_1);
        game_results.push(game_result_2);
        game_results.push(game_result_3);

        let lock_game_rsp = lock_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );
        match lock_game_rsp {
            Ok(lock_game_rsp) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                //assert_eq!(pool_detail_1.current_teams_count, 3u32);
                assert_eq!(
                    lock_game_rsp.attributes[1].value.clone(),
                    "GAME_POOL_CLOSED".to_string()
                );
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(3, 4);
            }
        }

        let game_pool_reward_distribute_rsp = game_pool_reward_distribute(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            pool_id_1.to_string(),
            game_results,
        );

        match game_pool_reward_distribute_rsp {
            Ok(game_pool_reward_distribute_rsp) => {}
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(4, 5);
            }
        }

        let mut query_game_status_res =
            query_game_details(&mut deps.storage, "Game001".to_string());
        match query_game_status_res {
            Ok(query_game_status_res) => {
                assert_eq!(query_game_status_res.game_status, GAME_COMPLETED);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(5, 6);
            }
        }
        let team_details = POOL_TEAM_DETAILS.load(&mut deps.storage, pool_id_1.clone());
        for team in team_details {
            assert_eq!(team[0].reward_amount, Uint128::from(100u128));
            assert_eq!(team[1].reward_amount, Uint128::from(200u128));
            assert_eq!(team[2].reward_amount, Uint128::from(300u128));
        }

        let claim_reward_rsp =
            claim_reward(deps.as_mut(), owner1Info.clone(), "Gamer002".to_string());
        match claim_reward_rsp {
            Ok(claim_reward_rsp) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                //assert_eq!(pool_detail_1.current_teams_count, 3u32);
                assert_eq!(
                    claim_reward_rsp.attributes[0].value.clone(),
                    "600".to_string()
                );
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(6, 7);
            }
        }
        query_game_status_res = query_game_details(&mut deps.storage, "Game001".to_string());
        match query_game_status_res {
            Ok(query_game_status_res) => {
                assert_eq!(query_game_status_res.game_status, GAME_COMPLETED);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(7, 8);
            }
        }

        let team_details = POOL_TEAM_DETAILS.load(&mut deps.storage, pool_id_1.clone());
        for team in team_details {
            assert_eq!(team[0].reward_amount, Uint128::from(100u128)); // TODO This reward should be 0 after full functionality working.
            assert_eq!(team[1].reward_amount, Uint128::from(200u128)); // TODO This reward should be 0 after full functionality working.
            assert_eq!(team[2].reward_amount, Uint128::from(300u128)); // TODO This reward should be 0 after full functionality working.
            assert_eq!(team[0].claimed_reward, CLAIMED_REWARD);
            assert_eq!(team[1].claimed_reward, CLAIMED_REWARD);
            assert_eq!(team[2].claimed_reward, CLAIMED_REWARD);
        }

        let claim_reward_rsp_2 =
            claim_reward(deps.as_mut(), owner1Info.clone(), "Gamer002".to_string());
        match claim_reward_rsp_2 {
            Ok(claim_reward_rsp_2) => {
				// IT should not come here
				assert_eq!(1,2);
            }
            Err(e) => {
                let outstr = format!("error parsing header: {:?}", e);
				println!("{:?}", outstr);
				assert_eq!(outstr, "error parsing header: Std(GenericErr { msg: \"No reward for this user\" })");
            }
        }
    }

    #[test]
    fn test_refund_game_pool_close_with_team_less_than_minimum_team_count() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Gamer002", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);

        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        let mut rake_list: Vec<WalletPercentage> = Vec::new();
        let rake_1 = WalletPercentage {
            wallet_address: "rake_1".to_string(),
            wallet_name: "rake_1".to_string(),
            percentage: 1u32,
        };
        rake_list.push(rake_1);
        let rake_2 = WalletPercentage {
            wallet_address: "rake_2".to_string(),
            wallet_name: "rake_2".to_string(),
            percentage: 2u32,
        };
        rake_list.push(rake_2);

        let rake_3 = WalletPercentage {
            wallet_address: "rake_3".to_string(),
            wallet_name: "rake_3".to_string(),
            percentage: 3u32,
        };
        rake_list.push(rake_3);

        set_pool_type_params(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "oneToTwo".to_string(),
            Uint128::from(144262u128),
            10,
            20,
            5,
            rake_list.clone(),
        );
        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        // create multiple pool
        let mut pool_id_1 = String::new();
        let rsp_1 = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToTwo".to_string(),
        );
        match rsp_1 {
            Ok(rsp_1) => {
                pool_id_1 = rsp_1.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
        let platform_fee = Uint128::from(300000u128);
        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let rewardInfo = mock_info("rewardInfo", &[]);
        let ownerXInfo = mock_info("cwtoken11111", &[coin(1000, "stake")]);
        // Adding same team twice in same pool
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team002".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team003".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );

        let query_pool_details_1 = query_pool_details(&mut deps.storage, pool_id_1.to_string());
        match query_pool_details_1 {
            Ok(pool_detail_1) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                assert_eq!(pool_detail_1.current_teams_count, 3u32);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(2, 3);
            }
        }

        let game_result_1 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team001".to_string(),
            team_rank: 1u64,
            team_points: 100u64,
            reward_amount: Uint128::from(100u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_2 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team002".to_string(),
            team_rank: 2u64,
            team_points: 200u64,
            reward_amount: Uint128::from(200u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_3 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team003".to_string(),
            team_rank: 2u64,
            team_points: 300u64,
            reward_amount: Uint128::from(300u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let mut game_results: Vec<GameResult> = Vec::new();
        game_results.push(game_result_1);
        game_results.push(game_result_2);
        game_results.push(game_result_3);

        let lock_game_rsp = lock_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );
        match lock_game_rsp {
            Ok(lock_game_rsp) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                //assert_eq!(pool_detail_1.current_teams_count, 3u32);
                assert_eq!(
                    lock_game_rsp.attributes[1].value.clone(),
                    "GAME_POOL_CLOSED".to_string()
                );
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(3, 4);
            }
        }
        let team_details = POOL_TEAM_DETAILS.load(&mut deps.storage, pool_id_1.clone());
        for team in team_details {
            //let mut gamer_addr = team[0].gamer_address.clone();
            let gamer_addr = Addr::unchecked(team[0].gamer_address.clone().as_str()); //owner1Info //deps.api.addr_validate(&gamer);
                                                                                      //let address = deps.api.addr_validate(team[0].gamer_address.clone().as_str());
            let gf_res = GAMING_FUNDS.load(&mut deps.storage, &gamer_addr);
            //let mut global_pool_id;
            match gf_res {
                Ok(gf_res) => {
                    println!("error parsing header: {:?}", gf_res);
                    assert_eq!(gf_res, Uint128::from(0u128)); 
                }
                Err(e) => {
                    println!("error parsing header: {:?}", e);
                    assert_eq!(4, 5);
                }
            }
        }
    }

    #[test]
    fn test_cancel_on_completed_game() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Gamer002", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);

        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        let mut rake_list: Vec<WalletPercentage> = Vec::new();
        let rake_1 = WalletPercentage {
            wallet_address: "rake_1".to_string(),
            wallet_name: "rake_1".to_string(),
            percentage: 1u32,
        };
        rake_list.push(rake_1);
        let rake_2 = WalletPercentage {
            wallet_address: "rake_2".to_string(),
            wallet_name: "rake_2".to_string(),
            percentage: 2u32,
        };
        rake_list.push(rake_2);

        let rake_3 = WalletPercentage {
            wallet_address: "rake_3".to_string(),
            wallet_name: "rake_3".to_string(),
            percentage: 3u32,
        };
        rake_list.push(rake_3);

        set_pool_type_params(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "oneToTwo".to_string(),
            Uint128::from(144262u128),
            2,
            10,
            5,
            rake_list.clone(),
        );
        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        // create multiple pool
        let mut pool_id_1 = String::new();
        let rsp_1 = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToTwo".to_string(),
        );
        match rsp_1 {
            Ok(rsp_1) => {
                pool_id_1 = rsp_1.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
        let rewardInfo = mock_info("rewardInfo", &[]);

        let ownerXInfo = mock_info("cwtoken11111", &[coin(1000, "stake")]);
        // Adding same team twice in same pool
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team002".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team003".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );

        let query_pool_details_1 = query_pool_details(&mut deps.storage, pool_id_1.to_string());
        match query_pool_details_1 {
            Ok(pool_detail_1) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                assert_eq!(pool_detail_1.current_teams_count, 3u32);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(2, 3);
            }
        }

        let game_result_1 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team001".to_string(),
            team_rank: 1u64,
            team_points: 100u64,
            reward_amount: Uint128::from(100u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_2 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team002".to_string(),
            team_rank: 2u64,
            team_points: 200u64,
            reward_amount: Uint128::from(200u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_3 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team003".to_string(),
            team_rank: 2u64,
            team_points: 300u64,
            reward_amount: Uint128::from(300u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let mut game_results: Vec<GameResult> = Vec::new();
        game_results.push(game_result_1);
        game_results.push(game_result_2);
        game_results.push(game_result_3);

        let lock_game_rsp = lock_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );
        match lock_game_rsp {
            Ok(lock_game_rsp) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                //assert_eq!(pool_detail_1.current_teams_count, 3u32);
                assert_eq!(
                    lock_game_rsp.attributes[1].value.clone(),
                    "GAME_POOL_CLOSED".to_string()
                );
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(3, 4);
            }
        }

        let game_pool_reward_distribute_rsp = game_pool_reward_distribute(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            pool_id_1.to_string(),
            game_results,
        );

        match game_pool_reward_distribute_rsp {
            Ok(game_pool_reward_distribute_rsp) => {}
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(4, 5);
            }
        }

        let mut query_game_status_res =
            query_game_details(&mut deps.storage, "Game001".to_string());
        match query_game_status_res {
            Ok(query_game_status_res) => {
                assert_eq!(query_game_status_res.game_status, GAME_COMPLETED);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(5, 6);
            }
        }

        let game_cancel_rsp = cancel_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        match game_cancel_rsp {
            Ok(game_cancel_rsp) => {
                assert_eq!(6, 7);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(
                    e.to_string(),
                    "Generic error: Cant cancel game as it is already over".to_string()
                );
            }
        }
    }

    #[test]
    fn test_reward_distribute_non_completed_game() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Gamer002", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);

        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        let mut rake_list: Vec<WalletPercentage> = Vec::new();
        let rake_1 = WalletPercentage {
            wallet_address: "rake_1".to_string(),
            wallet_name: "rake_1".to_string(),
            percentage: 1u32,
        };
        rake_list.push(rake_1);
        let rake_2 = WalletPercentage {
            wallet_address: "rake_2".to_string(),
            wallet_name: "rake_2".to_string(),
            percentage: 2u32,
        };
        rake_list.push(rake_2);

        let rake_3 = WalletPercentage {
            wallet_address: "rake_3".to_string(),
            wallet_name: "rake_3".to_string(),
            percentage: 3u32,
        };
        rake_list.push(rake_3);

        set_pool_type_params(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "oneToTwo".to_string(),
            Uint128::from(144262u128),
            2,
            10,
            5,
            rake_list.clone(),
        );
        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        // create multiple pool
        let mut pool_id_1 = String::new();
        let rsp_1 = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToTwo".to_string(),
        );
        match rsp_1 {
            Ok(rsp_1) => {
                pool_id_1 = rsp_1.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }

        let game_result_1 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team001".to_string(),
            team_rank: 1u64,
            team_points: 100u64,
            reward_amount: Uint128::from(100u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_2 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team002".to_string(),
            team_rank: 2u64,
            team_points: 200u64,
            reward_amount: Uint128::from(200u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_3 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team003".to_string(),
            team_rank: 2u64,
            team_points: 300u64,
            reward_amount: Uint128::from(300u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let mut game_results: Vec<GameResult> = Vec::new();
        game_results.push(game_result_1);
        game_results.push(game_result_2);
        game_results.push(game_result_3);

        let mut game_pool_reward_distribute_rsp = game_pool_reward_distribute(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            pool_id_1.to_string(),
            game_results.clone(),
        );

        match game_pool_reward_distribute_rsp {
            Ok(game_pool_reward_distribute_rsp) => {
                assert_eq!(2, 3);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(
                    e.to_string(),
                    "Generic error: Rewards cant be distributed as game not yet started"
                        .to_string()
                );
            }
        }

        let rewardInfo = mock_info("rewardInfo", &[]);
        let ownerXInfo = mock_info("cwtoken11111", &[coin(1000, "stake")]);
        // Adding same team twice in same pool
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team002".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team003".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );

        let query_pool_details_1 = query_pool_details(&mut deps.storage, pool_id_1.to_string());
        match query_pool_details_1 {
            Ok(pool_detail_1) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                assert_eq!(pool_detail_1.current_teams_count, 3u32);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(3, 4);
            }
        }

        game_pool_reward_distribute_rsp = game_pool_reward_distribute(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            pool_id_1.to_string(),
            game_results,
        );

        match game_pool_reward_distribute_rsp {
            Ok(game_pool_reward_distribute_rsp) => {
                assert_eq!(4, 5);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(
                    e.to_string(),
                    "Generic error: Rewards cant be distributed as game not yet started"
                        .to_string()
                );
            }
        }
    }

    #[test]
    fn test_game_pool_reward_distribute_again() {
        let mut deps = mock_dependencies(&[]);
        let owner1Info = mock_info("Gamer002", &[coin(1000, "stake")]);
        let platform_fee = Uint128::from(300000u128);

        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let adminInfo = mock_info("admin11111", &[]);
        instantiate(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            instantiate_msg,
        );

        let mut rake_list: Vec<WalletPercentage> = Vec::new();
        let rake_1 = WalletPercentage {
            wallet_address: "rake_1".to_string(),
            wallet_name: "rake_1".to_string(),
            percentage: 1u32,
        };
        rake_list.push(rake_1);
        let rake_2 = WalletPercentage {
            wallet_address: "rake_2".to_string(),
            wallet_name: "rake_2".to_string(),
            percentage: 2u32,
        };
        rake_list.push(rake_2);

        let rake_3 = WalletPercentage {
            wallet_address: "rake_3".to_string(),
            wallet_name: "rake_3".to_string(),
            percentage: 3u32,
        };
        rake_list.push(rake_3);

        set_pool_type_params(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "oneToTwo".to_string(),
            Uint128::from(144262u128),
            2,
            10,
            5,
            rake_list.clone(),
        );
        create_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );

        // create multiple pool
        let mut pool_id_1 = String::new();
        let rsp_1 = create_pool(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            "oneToTwo".to_string(),
        );
        match rsp_1 {
            Ok(rsp_1) => {
                pool_id_1 = rsp_1.attributes[0].value.clone();
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(1, 2);
            }
        }
        let platform_fee = Uint128::from(300000u128);
        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };
        let rewardInfo = mock_info("rewardInfo", &[]);
        let ownerXInfo = mock_info("cwtoken11111", &[coin(1000, "stake")]);
        // Adding same team twice in same pool
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team001".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team002".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );
        game_pool_bid_submit(
            deps.as_mut(),
            mock_env(),
            ownerXInfo.clone(),
            "Gamer002".to_string(),
            "oneToTwo".to_string(),
            pool_id_1.to_string(),
            "Game001".to_string(),
            "Team003".to_string(),
            Uint128::from(144262u128) + platform_fee,
        );

        let query_pool_details_1 = query_pool_details(&mut deps.storage, pool_id_1.to_string());
        match query_pool_details_1 {
            Ok(pool_detail_1) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                assert_eq!(pool_detail_1.current_teams_count, 3u32);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(2, 3);
            }
        }

        let game_result_1 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team001".to_string(),
            team_rank: 1u64,
            team_points: 100u64,
            reward_amount: Uint128::from(100u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_2 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team002".to_string(),
            team_rank: 2u64,
            team_points: 200u64,
            reward_amount: Uint128::from(200u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let game_result_3 = GameResult {
            gamer_address: "Gamer002".to_string(),
            game_id: "Game001".to_string(),
            team_id: "Team003".to_string(),
            team_rank: 2u64,
            team_points: 300u64,
            reward_amount: Uint128::from(300u128),
            refund_amount: Uint128::from(INITIAL_REFUND_AMOUNT),
        };
        let mut game_results: Vec<GameResult> = Vec::new();
        game_results.push(game_result_1);
        game_results.push(game_result_2);
        game_results.push(game_result_3);

        let lock_game_rsp = lock_game(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
        );
        match lock_game_rsp {
            Ok(lock_game_rsp) => {
                //Since max allowed team for gamer under this pooltype is 2 so it will not allow 3rd team creation under this pooltype.
                //assert_eq!(pool_detail_1.current_teams_count, 3u32);
                assert_eq!(
                    lock_game_rsp.attributes[1].value.clone(),
                    "GAME_POOL_CLOSED".to_string()
                );
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(3, 4);
            }
        }

        let game_pool_reward_distribute_rsp = game_pool_reward_distribute(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            pool_id_1.to_string(),
            game_results.clone(),
        );

        match game_pool_reward_distribute_rsp {
            Ok(game_pool_reward_distribute_rsp) => {}
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(4, 5);
            }
        }

        let query_game_status_res = query_game_details(&mut deps.storage, "Game001".to_string());
        match query_game_status_res {
            Ok(query_game_status_res) => {
                assert_eq!(query_game_status_res.game_status, GAME_COMPLETED);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(5, 6);
            }
        }
        let team_details = POOL_TEAM_DETAILS.load(&mut deps.storage, pool_id_1.clone());
        for team in team_details {
            assert_eq!(team[0].reward_amount, Uint128::from(100u128));
            assert_eq!(team[1].reward_amount, Uint128::from(200u128));
            assert_eq!(team[2].reward_amount, Uint128::from(300u128));
        }

        let game_pool_reward_distribute_rsp_2 = game_pool_reward_distribute(
            deps.as_mut(),
            mock_env(),
            adminInfo.clone(),
            "Game001".to_string(),
            pool_id_1.to_string(),
            game_results,
        );

        match game_pool_reward_distribute_rsp_2 {
            Ok(game_pool_reward_distribute_rsp_2) => {
                assert_eq!(6, 7);
            }
            Err(e) => {
                println!("error parsing header: {:?}", e);
                assert_eq!(
                    e.to_string(),
                    "Generic error: Rewards are already distributed for this pool".to_string()
                );
            }
        }
    }

    #[test]
    fn test_set_platform_fee_wallets() {
        let mut deps = mock_dependencies(&[]);
        let platform_fee = Uint128::from(300000u128);

        let instantiate_msg = InstantiateMsg {
            minting_contract_address: "cwtoken11111".to_string(),
            admin_address: "admin11111".to_string(),
            platform_fee: platform_fee,
        };

        let adminInfo = mock_info("admin11111", &[]);
        let mut rake_list: Vec<WalletPercentage> = Vec::new();
        let rake_1 = WalletPercentage {
            wallet_address: "rake_1".to_string(),
            wallet_name: "rake_1".to_string(),
            percentage: 1u32,
        };
        rake_list.push(rake_1);
        let rake_2 = WalletPercentage {
            wallet_address: "rake_2".to_string(),
            wallet_name: "rake_2".to_string(),
            percentage: 2u32,
        };
        rake_list.push(rake_2);

        let rake_3 = WalletPercentage {
            wallet_address: "rake_3".to_string(),
            wallet_name: "rake_3".to_string(),
            percentage: 3u32,
        };
        rake_list.push(rake_3);

        set_platform_fee_wallets(deps.as_mut(), adminInfo, rake_list);

        let wallets = PLATFORM_WALLET_PERCENTAGES.load(&mut deps.storage, "test".to_string());

        for wallet in wallets {
            assert_eq!(wallet.wallet_name, "rake_1".to_string());
            assert_eq!(wallet.wallet_name, "rake_2".to_string());
            assert_eq!(wallet.wallet_name, "rake_3".to_string());
        }
    }
}

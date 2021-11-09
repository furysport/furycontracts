#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response, StdError, StdResult,
    Uint128, WasmMsg,
};

use cw2::set_contract_version;
use cw20::{
    AllowanceResponse, BalanceResponse, Cw20Coin, Cw20ExecuteMsg, Cw20ReceiveMsg, Expiration,
};

use crate::allowances::{
    deduct_allowance, execute_burn_from, execute_decrease_allowance, execute_increase_allowance,
    execute_send_from, execute_transfer_from, query_allowance,
};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{
    ClubOwnershipDetails, ClubStakingDetails, Config, CLUB_OWNERSHIP_DETAILS, CLUB_STAKING_DETAILS,
    CONFIG, CONTRACT_WALLET, REWARD,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw20-base";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const MAIN_WALLET: &str = "terra1t3czdl5h4w4qwgkzs80fdstj0z7rfv9v2j6uh3";

const CLUB_PRICE: u128 = 1000000000u128;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        cw20_token_address: deps.api.addr_validate(&msg.cw20_token_address)?,
        admin_address: deps.api.addr_validate(&msg.admin_address)?,
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
        ExecuteMsg::BuyAClub { buyer, club_name } => {
            buy_a_club(deps, env, info, buyer, club_name, Uint128::from(CLUB_PRICE))
        }
        ExecuteMsg::StakeOnAClub {
            staker,
            club_name,
            amount,
            duration,
        } => stake_on_a_club(deps, env, info, staker, club_name, amount, duration),
        ExecuteMsg::SetRewardAmount { amount } => set_reward_amount(deps, info, amount),
        ExecuteMsg::CalculateAndDistributeRewards {} => {
            calculate_and_distribute_rewards(deps, info)
        }
        ExecuteMsg::IncreaseAllowance {
            spender,
            amount,
            expires,
        } => execute_increase_allowance(deps, env, info, spender, amount, expires),
        ExecuteMsg::DecreaseAllowance {
            spender,
            amount,
            expires,
        } => execute_decrease_allowance(deps, env, info, spender, amount, expires),
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => execute_transfer_from(deps, env, info, owner, recipient, amount),
        ExecuteMsg::BurnFrom { owner, amount } => execute_burn_from(deps, env, info, owner, amount),
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            msg,
        } => execute_send_from(deps, env, info, owner, contract, amount, msg),
    }
}

fn buy_a_club(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    buyer: String,
    club_name: String,
    price: Uint128,
) -> Result<Response, ContractError> {
    let buyer_addr = deps.api.addr_validate(&buyer)?;
    //Check if buyer is same as invoker
    if buyer_addr != info.sender {
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
    if ownership_details.is_none() {
        // Deduct amount form the buyers wallet
        //ToDo: Assuming that above operation went good. Need to implement this
        // Now save the ownership details
        CLUB_OWNERSHIP_DETAILS.save(
            deps.storage,
            club_name.clone(),
            &ClubOwnershipDetails {
                club_name: club_name,
                start_timestamp: env.block.time,
                locking_period: 21 * 24 * 60 * 60, //21 days
                owner_address: buyer.clone(),
                price_paid: price,
            },
        )?;
        //If successfully bought save the funds in contract wallet
        CONTRACT_WALLET.update(
            deps.storage,
            &buyer_addr,
            |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + price) },
        )?;
    } else {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("Cant have more than one owner for a club"),
        }));
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
    duration: u64,
) -> Result<Response, ContractError> {
    let staker_addr = deps.api.addr_validate(&staker)?;
    //Check if buyer is same as invoker
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
    if ownership_details.is_some() {
        // Now save the staking details
        // Get the exisint stakes for this club
        let all_stakes;
        let all_stakes_result = CLUB_STAKING_DETAILS.may_load(deps.storage, club_name.clone());
        match all_stakes_result {
            Ok(asr) => {
                all_stakes = asr;
            }
            Err(e) => {
                return Err(ContractError::Std(StdError::from(e)));
            }
        }
        let mut stakes = Vec::new();
        if all_stakes.is_some() {
            //There are some stakes for this club
            stakes = all_stakes.unwrap();
        }
        stakes.push(ClubStakingDetails {
            staker_address: staker.clone(),
            staking_start_timestamp: env.block.time,
            staked_amount: amount,
            staking_duration: duration,
            club_name: club_name.clone(),
        });
        CLUB_STAKING_DETAILS.save(deps.storage, club_name, &stakes)?;
        //If successfully staked, save the funds in contract wallet
        CONTRACT_WALLET.update(
            deps.storage,
            &staker_addr,
            |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
        )?;
        // // Deduct amount form the stakers wallet
        // let config = CONFIG.load(deps.storage)?;
        // let res = Response::new()
        //     .add_message(WasmMsg::Execute {
        //         contract_addr: config.cw20_token_address.to_string(),
        //         funds: vec![],
        //         msg: to_binary(&Cw20ExecuteMsg::Send {
        //             contract: config.club_staking_address.to_string(),
        //             amount: amount,
        //             msg: to_binary(data: &T)
        //         })?,
        //     })
        //     .add_attributes(vec![
        //         attr("action", "stake"),
        //         attr("club", club_name),
        //         attr("address", info.sender),
        //         attr("amount", amount),
        //     ]);
    } else {
        return Err(ContractError::Std(StdError::GenericErr {
            msg: String::from("The club is not available for staking"),
        }));
    }
    return Ok(Response::default());
}

fn set_reward_amount(
    deps: DepsMut,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    // Check if this is executed by main/transaction wallet
    let config = CONFIG.load(deps.storage)?;
    if info.sender == config.cw20_token_address {
        return Err(ContractError::Unauthorized {});
    }
    REWARD.save(deps.storage, &amount)?;
    return Ok(Response::default());
}

fn calculate_and_distribute_rewards(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // Check if this is executed by main/transaction wallet
    let config = CONFIG.load(deps.storage)?;
    if info.sender == config.cw20_token_address {
        return Err(ContractError::Unauthorized {});
    }
    let total_reward = REWARD.may_load(deps.storage)?.unwrap_or_default();
    if total_reward > Uint128::zero() {
        let _winner_other_split: (Uint128, Uint128) = calculate_20_80_share(total_reward);
    }
    return Ok(Response::default());
}

fn calculate_20_80_share(amount: Uint128) -> (Uint128, Uint128) {
    let winner_share = amount
        .checked_mul(Uint128::from(20u128))
        .unwrap_or_default()
        .checked_div(Uint128::from(100u128))
        .unwrap_or_default();
    let other_share = amount
        .checked_mul(Uint128::from(80u128))
        .unwrap_or_default()
        .checked_div(Uint128::from(100u128))
        .unwrap_or_default();
    return (winner_share, other_share);
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Allowance { owner, spender } => {
            to_binary(&query_allowance(deps, owner, spender)?)
        }
        QueryMsg::AllAllowances {
            owner,
            start_after,
            limit,
        } => to_binary(&query_allowance(deps, owner.clone(), owner.clone())?),
        QueryMsg::ClubStakingDetails { club_name } => {
            to_binary(&query_club_staking_details(deps, club_name)?)
        }
        QueryMsg::ClubOwnershipDetails { club_name } => {
            to_binary(&query_club_ownership_details(deps, club_name)?)
        }
        QueryMsg::AllStakes {} => to_binary(&query_all_stakes(deps)?),
        QueryMsg::GetClubRankingByStakes {} => to_binary(&get_clubs_ranking_by_stakes(deps)?),
        QueryMsg::RewardAmount {} => to_binary(&query_reward_amount(deps)?),
    }
}

pub fn query_club_staking_details(
    deps: Deps,
    club_name: String,
) -> StdResult<Vec<ClubStakingDetails>> {
    let csd = CLUB_STAKING_DETAILS.may_load(deps.storage, club_name)?;
    match csd {
        Some(csd) => return Ok(csd),
        None => return Err(StdError::generic_err("No staking details found")),
    };
}

fn query_all_stakes(deps: Deps) -> StdResult<Vec<ClubStakingDetails>> {
    let mut all_stakes = Vec::new();
    let all_clubs: Vec<String> = CLUB_STAKING_DETAILS
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for club_name in all_clubs {
        let staking_details = CLUB_STAKING_DETAILS.load(deps.storage, club_name)?;
        for stake in staking_details {
            all_stakes.push(stake);
        }
    }
    return Ok(all_stakes);
}

fn get_clubs_ranking_by_stakes(deps: Deps) -> StdResult<Vec<(String, Uint128)>> {
    let mut all_stakes = Vec::new();
    let all_clubs: Vec<String> = CLUB_STAKING_DETAILS
        .keys(deps.storage, None, None, Order::Ascending)
        .map(|k| String::from_utf8(k).unwrap())
        .collect();
    for club_name in all_clubs {
        let _tp = query_club_staking_details(deps, club_name.clone())?;
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

fn query_club_ownership_details(deps: Deps, club_name: String) -> StdResult<ClubOwnershipDetails> {
    let cod = CLUB_OWNERSHIP_DETAILS.may_load(deps.storage, club_name)?;
    match cod {
        Some(cod) => return Ok(cod),
        None => return Err(StdError::generic_err("No ownership details found")),
    };
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary, Addr, CosmosMsg, StdError, SubMsg, WasmMsg};

    use super::*;
    use crate::msg::InstantiateMarketingInfo;

    fn get_balance<T: Into<String>>(deps: Deps, address: T) -> Uint128 {
        query_balance(deps, address.into()).unwrap().balance
    }

    // this will set up the instantiation for other tests
    fn do_instantiate_with_minter(
        deps: DepsMut,
        addr: &str,
        amount: Uint128,
        minter: &str,
        cap: Option<Uint128>,
    ) -> TokenInfoResponse {
        _do_instantiate(
            deps,
            addr,
            amount,
            Some(MinterResponse {
                minter: minter.to_string(),
                cap,
            }),
        )
    }

    // this will set up the instantiation for other tests
    fn do_instantiate(deps: DepsMut, addr: &str, amount: Uint128) -> TokenInfoResponse {
        _do_instantiate(deps, addr, amount, None)
    }

    // this will set up the instantiation for other tests
    fn _do_instantiate(
        mut deps: DepsMut,
        addr: &str,
        amount: Uint128,
        mint: Option<MinterResponse>,
    ) -> TokenInfoResponse {
        let instantiate_msg = InstantiateMsg {
            name: "Auto Gen".to_string(),
            symbol: "AUTO".to_string(),
            decimals: 3,
            initial_balances: vec![Cw20Coin {
                address: addr.to_string(),
                amount,
            }],
            mint: mint.clone(),
            marketing: None,
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();
        let res = instantiate(deps.branch(), env, info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        let meta = query_token_info(deps.as_ref()).unwrap();
        assert_eq!(
            meta,
            TokenInfoResponse {
                name: "Auto Gen".to_string(),
                symbol: "AUTO".to_string(),
                decimals: 3,
                total_supply: amount,
            }
        );
        assert_eq!(get_balance(deps.as_ref(), addr), amount);
        assert_eq!(query_minter(deps.as_ref()).unwrap(), mint,);
        meta
    }

    const PNG_HEADER: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

    mod instantiate {
        use super::*;

        #[test]
        fn basic() {
            let mut deps = mock_dependencies(&[]);
            let amount = Uint128::from(11223344u128);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![Cw20Coin {
                    address: String::from("addr0000"),
                    amount,
                }],
                mint: None,
                marketing: None,
            };
            let info = mock_info("creator", &[]);
            let env = mock_env();
            let res = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
            assert_eq!(0, res.messages.len());

            assert_eq!(
                query_token_info(deps.as_ref()).unwrap(),
                TokenInfoResponse {
                    name: "Cash Token".to_string(),
                    symbol: "CASH".to_string(),
                    decimals: 9,
                    total_supply: amount,
                }
            );
            assert_eq!(
                get_balance(deps.as_ref(), "addr0000"),
                Uint128::new(11223344)
            );
        }

        #[test]
        fn mintable() {
            let mut deps = mock_dependencies(&[]);
            let amount = Uint128::new(11223344);
            let minter = String::from("asmodat");
            let limit = Uint128::new(511223344);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![Cw20Coin {
                    address: "addr0000".into(),
                    amount,
                }],
                mint: Some(MinterResponse {
                    minter: minter.clone(),
                    cap: Some(limit),
                }),
                marketing: None,
            };
            let info = mock_info("creator", &[]);
            let env = mock_env();
            let res = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
            assert_eq!(0, res.messages.len());

            assert_eq!(
                query_token_info(deps.as_ref()).unwrap(),
                TokenInfoResponse {
                    name: "Cash Token".to_string(),
                    symbol: "CASH".to_string(),
                    decimals: 9,
                    total_supply: amount,
                }
            );
            assert_eq!(
                get_balance(deps.as_ref(), "addr0000"),
                Uint128::new(11223344)
            );
            assert_eq!(
                query_minter(deps.as_ref()).unwrap(),
                Some(MinterResponse {
                    minter,
                    cap: Some(limit),
                }),
            );
        }

        #[test]
        fn mintable_over_cap() {
            let mut deps = mock_dependencies(&[]);
            let amount = Uint128::new(11223344);
            let minter = String::from("asmodat");
            let limit = Uint128::new(11223300);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![Cw20Coin {
                    address: String::from("addr0000"),
                    amount,
                }],
                mint: Some(MinterResponse {
                    minter,
                    cap: Some(limit),
                }),
                marketing: None,
            };
            let info = mock_info("creator", &[]);
            let env = mock_env();
            let err = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap_err();
            assert_eq!(
                err,
                StdError::generic_err("Initial supply greater than cap").into()
            );
        }

        mod marketing {
            use super::*;

            #[test]
            fn basic() {
                let mut deps = mock_dependencies(&[]);
                let instantiate_msg = InstantiateMsg {
                    name: "Cash Token".to_string(),
                    symbol: "CASH".to_string(),
                    decimals: 9,
                    initial_balances: vec![],
                    mint: None,
                    marketing: Some(InstantiateMarketingInfo {
                        project: Some("Project".to_owned()),
                        description: Some("Description".to_owned()),
                        marketing: Some("marketing".to_owned()),
                        logo: Some(Logo::Url("url".to_owned())),
                    }),
                };

                let info = mock_info("creator", &[]);
                let env = mock_env();
                let res = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
                assert_eq!(0, res.messages.len());

                assert_eq!(
                    query_marketing_info(deps.as_ref()).unwrap(),
                    MarketingInfoResponse {
                        project: Some("Project".to_owned()),
                        description: Some("Description".to_owned()),
                        marketing: Some(Addr::unchecked("marketing")),
                        logo: Some(LogoInfo::Url("url".to_owned())),
                    }
                );

                let err = query_download_logo(deps.as_ref()).unwrap_err();
                assert!(
                    matches!(err, StdError::NotFound { .. }),
                    "Expected StdError::NotFound, received {}",
                    err
                );
            }

            #[test]
            fn invalid_marketing() {
                let mut deps = mock_dependencies(&[]);
                let instantiate_msg = InstantiateMsg {
                    name: "Cash Token".to_string(),
                    symbol: "CASH".to_string(),
                    decimals: 9,
                    initial_balances: vec![],
                    mint: None,
                    marketing: Some(InstantiateMarketingInfo {
                        project: Some("Project".to_owned()),
                        description: Some("Description".to_owned()),
                        marketing: Some("m".to_owned()),
                        logo: Some(Logo::Url("url".to_owned())),
                    }),
                };

                let info = mock_info("creator", &[]);
                let env = mock_env();
                instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap_err();

                let err = query_download_logo(deps.as_ref()).unwrap_err();
                assert!(
                    matches!(err, StdError::NotFound { .. }),
                    "Expected StdError::NotFound, received {}",
                    err
                );
            }
        }
    }

    #[test]
    fn can_mint_by_minter() {
        let mut deps = mock_dependencies(&[]);

        let genesis = String::from("genesis");
        let amount = Uint128::new(11223344);
        let minter = String::from("asmodat");
        let limit = Uint128::new(511223344);
        do_instantiate_with_minter(deps.as_mut(), &genesis, amount, &minter, Some(limit));

        // minter can mint coins to some winner
        let winner = String::from("lucky");
        let prize = Uint128::new(222_222_222);
        let msg = ExecuteMsg::Mint {
            recipient: winner.clone(),
            amount: prize,
        };

        let info = mock_info(minter.as_ref(), &[]);
        let env = mock_env();
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(0, res.messages.len());
        assert_eq!(get_balance(deps.as_ref(), genesis), amount);
        assert_eq!(get_balance(deps.as_ref(), winner.clone()), prize);

        // but cannot mint nothing
        let msg = ExecuteMsg::Mint {
            recipient: winner.clone(),
            amount: Uint128::zero(),
        };
        let info = mock_info(minter.as_ref(), &[]);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::InvalidZeroAmount {});

        // but if it exceeds cap (even over multiple rounds), it fails
        // cap is enforced
        let msg = ExecuteMsg::Mint {
            recipient: winner,
            amount: Uint128::new(333_222_222),
        };
        let info = mock_info(minter.as_ref(), &[]);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::CannotExceedCap {});
    }

    #[test]
    fn others_cannot_mint() {
        let mut deps = mock_dependencies(&[]);
        do_instantiate_with_minter(
            deps.as_mut(),
            &String::from("genesis"),
            Uint128::new(1234),
            &String::from("minter"),
            None,
        );

        let msg = ExecuteMsg::Mint {
            recipient: String::from("lucky"),
            amount: Uint128::new(222),
        };
        let info = mock_info("anyone else", &[]);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});
    }

    #[test]
    fn no_one_mints_if_minter_unset() {
        let mut deps = mock_dependencies(&[]);
        do_instantiate(deps.as_mut(), &String::from("genesis"), Uint128::new(1234));

        let msg = ExecuteMsg::Mint {
            recipient: String::from("lucky"),
            amount: Uint128::new(222),
        };
        let info = mock_info("genesis", &[]);
        let env = mock_env();
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::Unauthorized {});
    }

    #[test]
    fn instantiate_multiple_accounts() {
        let mut deps = mock_dependencies(&[]);
        let amount1 = Uint128::from(11223344u128);
        let addr1 = String::from("addr0001");
        let amount2 = Uint128::from(7890987u128);
        let addr2 = String::from("addr0002");
        let instantiate_msg = InstantiateMsg {
            name: "Bash Shell".to_string(),
            symbol: "BASH".to_string(),
            decimals: 6,
            initial_balances: vec![
                Cw20Coin {
                    address: addr1.clone(),
                    amount: amount1,
                },
                Cw20Coin {
                    address: addr2.clone(),
                    amount: amount2,
                },
            ],
            mint: None,
            marketing: None,
        };
        let info = mock_info("creator", &[]);
        let env = mock_env();
        let res = instantiate(deps.as_mut(), env, info, instantiate_msg).unwrap();
        assert_eq!(0, res.messages.len());

        assert_eq!(
            query_token_info(deps.as_ref()).unwrap(),
            TokenInfoResponse {
                name: "Bash Shell".to_string(),
                symbol: "BASH".to_string(),
                decimals: 6,
                total_supply: amount1 + amount2,
            }
        );
        assert_eq!(get_balance(deps.as_ref(), addr1), amount1);
        assert_eq!(get_balance(deps.as_ref(), addr2), amount2);
    }

    #[test]
    fn queries_work() {
        let mut deps = mock_dependencies(&coins(2, "token"));
        let addr1 = String::from("addr0001");
        let amount1 = Uint128::from(12340000u128);

        let expected = do_instantiate(deps.as_mut(), &addr1, amount1);

        // check meta query
        let loaded = query_token_info(deps.as_ref()).unwrap();
        assert_eq!(expected, loaded);

        let _info = mock_info("test", &[]);
        let env = mock_env();
        // check balance query (full)
        let data = query(
            deps.as_ref(),
            env.clone(),
            QueryMsg::Balance { address: addr1 },
        )
        .unwrap();
        let loaded: BalanceResponse = from_binary(&data).unwrap();
        assert_eq!(loaded.balance, amount1);

        // check balance query (empty)
        let data = query(
            deps.as_ref(),
            env,
            QueryMsg::Balance {
                address: String::from("addr0002"),
            },
        )
        .unwrap();
        let loaded: BalanceResponse = from_binary(&data).unwrap();
        assert_eq!(loaded.balance, Uint128::zero());
    }

    #[test]
    fn transfer() {
        let mut deps = mock_dependencies(&coins(2, "token"));
        let addr1 = String::from("addr0001");
        let addr2 = String::from("addr0002");
        let amount1 = Uint128::from(12340000u128);
        let transfer = Uint128::from(76543u128);
        let too_much = Uint128::from(12340321u128);

        do_instantiate(deps.as_mut(), &addr1, amount1);

        // cannot transfer nothing
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Transfer {
            recipient: addr2.clone(),
            amount: Uint128::zero(),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::InvalidZeroAmount {});

        // cannot send more than we have
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Transfer {
            recipient: addr2.clone(),
            amount: too_much,
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));

        // cannot send from empty account
        let info = mock_info(addr2.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Transfer {
            recipient: addr1.clone(),
            amount: transfer,
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));

        // valid transfer
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Transfer {
            recipient: addr2.clone(),
            amount: transfer,
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.messages.len(), 0);

        let remainder = amount1.checked_sub(transfer).unwrap();
        assert_eq!(get_balance(deps.as_ref(), addr1), remainder);
        assert_eq!(get_balance(deps.as_ref(), addr2), transfer);
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            amount1
        );
    }

    #[test]
    fn burn() {
        let mut deps = mock_dependencies(&coins(2, "token"));
        let addr1 = String::from("addr0001");
        let amount1 = Uint128::from(12340000u128);
        let burn = Uint128::from(76543u128);
        let too_much = Uint128::from(12340321u128);

        do_instantiate(deps.as_mut(), &addr1, amount1);

        // cannot burn nothing
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Burn {
            amount: Uint128::zero(),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::InvalidZeroAmount {});
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            amount1
        );

        // cannot burn more than we have
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Burn { amount: too_much };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            amount1
        );

        // valid burn reduces total supply
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Burn { amount: burn };
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.messages.len(), 0);

        let remainder = amount1.checked_sub(burn).unwrap();
        assert_eq!(get_balance(deps.as_ref(), addr1), remainder);
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            remainder
        );
    }

    #[test]
    fn send() {
        let mut deps = mock_dependencies(&coins(2, "token"));
        let addr1 = String::from("addr0001");
        let contract = String::from("addr0002");
        let amount1 = Uint128::from(12340000u128);
        let transfer = Uint128::from(76543u128);
        let too_much = Uint128::from(12340321u128);
        let send_msg = Binary::from(r#"{"some":123}"#.as_bytes());

        do_instantiate(deps.as_mut(), &addr1, amount1);

        // cannot send nothing
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Send {
            contract: contract.clone(),
            amount: Uint128::zero(),
            msg: send_msg.clone(),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert_eq!(err, ContractError::InvalidZeroAmount {});

        // cannot send more than we have
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Send {
            contract: contract.clone(),
            amount: too_much,
            msg: send_msg.clone(),
        };
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        assert!(matches!(err, ContractError::Std(StdError::Overflow { .. })));

        // valid transfer
        let info = mock_info(addr1.as_ref(), &[]);
        let env = mock_env();
        let msg = ExecuteMsg::Send {
            contract: contract.clone(),
            amount: transfer,
            msg: send_msg.clone(),
        };
        let res = execute(deps.as_mut(), env, info, msg).unwrap();
        assert_eq!(res.messages.len(), 1);

        // ensure proper send message sent
        // this is the message we want delivered to the other side
        let binary_msg = Cw20ReceiveMsg {
            sender: addr1.clone(),
            amount: transfer,
            msg: send_msg,
        }
        .into_binary()
        .unwrap();
        // and this is how it must be wrapped for the vm to process it
        assert_eq!(
            res.messages[0],
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract.clone(),
                msg: binary_msg,
                funds: vec![],
            }))
        );

        // ensure balance is properly transferred
        let remainder = amount1.checked_sub(transfer).unwrap();
        assert_eq!(get_balance(deps.as_ref(), addr1), remainder);
        assert_eq!(get_balance(deps.as_ref(), contract), transfer);
        assert_eq!(
            query_token_info(deps.as_ref()).unwrap().total_supply,
            amount1
        );
    }

    mod marketing {
        use super::*;

        #[test]
        fn update_unauthorised() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("marketing".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let err = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: Some("New project".to_owned()),
                    description: Some("Better description".to_owned()),
                    marketing: Some("creator".to_owned()),
                },
            )
            .unwrap_err();

            assert_eq!(err, ContractError::Unauthorized {});

            // Ensure marketing didn't change
            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("marketing")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_project() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: Some("New project".to_owned()),
                    description: None,
                    marketing: None,
                },
            )
            .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("New project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn clear_project() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: Some("".to_owned()),
                    description: None,
                    marketing: None,
                },
            )
            .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: None,
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_description() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: None,
                    description: Some("Better description".to_owned()),
                    marketing: None,
                },
            )
            .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Better description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn clear_description() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: None,
                    description: Some("".to_owned()),
                    marketing: None,
                },
            )
            .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: None,
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_marketing() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: None,
                    description: None,
                    marketing: Some("marketing".to_owned()),
                },
            )
            .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("marketing")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_marketing_invalid() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let err = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: None,
                    description: None,
                    marketing: Some("m".to_owned()),
                },
            )
            .unwrap_err();

            assert!(
                matches!(err, ContractError::Std(_)),
                "Expected Std error, received: {}",
                err
            );

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn clear_marketing() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UpdateMarketing {
                    project: None,
                    description: None,
                    marketing: Some("".to_owned()),
                },
            )
            .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: None,
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_logo_url() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UploadLogo(Logo::Url("new_url".to_owned())),
            )
            .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("new_url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_logo_png() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UploadLogo(Logo::Embedded(EmbeddedLogo::Png(PNG_HEADER.into()))),
            )
            .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Embedded),
                }
            );

            assert_eq!(
                query_download_logo(deps.as_ref()).unwrap(),
                DownloadLogoResponse {
                    mime_type: "image/png".to_owned(),
                    data: PNG_HEADER.into(),
                }
            );
        }

        #[test]
        fn update_logo_svg() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let img = "<?xml version=\"1.0\"?><svg></svg>".as_bytes();
            let res = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UploadLogo(Logo::Embedded(EmbeddedLogo::Svg(img.into()))),
            )
            .unwrap();

            assert_eq!(res.messages, vec![]);

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Embedded),
                }
            );

            assert_eq!(
                query_download_logo(deps.as_ref()).unwrap(),
                DownloadLogoResponse {
                    mime_type: "image/svg+xml".to_owned(),
                    data: img.into(),
                }
            );
        }

        #[test]
        fn update_logo_png_oversized() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let img = [&PNG_HEADER[..], &[1; 6000][..]].concat();
            let err = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UploadLogo(Logo::Embedded(EmbeddedLogo::Png(img.into()))),
            )
            .unwrap_err();

            assert_eq!(err, ContractError::LogoTooBig {});

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_logo_svg_oversized() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let img = [
                "<?xml version=\"1.0\"?><svg>",
                std::str::from_utf8(&[b'x'; 6000]).unwrap(),
                "</svg>",
            ]
            .concat()
            .into_bytes();

            let err = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UploadLogo(Logo::Embedded(EmbeddedLogo::Svg(img.into()))),
            )
            .unwrap_err();

            assert_eq!(err, ContractError::LogoTooBig {});

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_logo_png_invalid() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let img = &[1];
            let err = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UploadLogo(Logo::Embedded(EmbeddedLogo::Png(img.into()))),
            )
            .unwrap_err();

            assert_eq!(err, ContractError::InvalidPngHeader {});

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }

        #[test]
        fn update_logo_svg_invalid() {
            let mut deps = mock_dependencies(&[]);
            let instantiate_msg = InstantiateMsg {
                name: "Cash Token".to_string(),
                symbol: "CASH".to_string(),
                decimals: 9,
                initial_balances: vec![],
                mint: None,
                marketing: Some(InstantiateMarketingInfo {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some("creator".to_owned()),
                    logo: Some(Logo::Url("url".to_owned())),
                }),
            };

            let info = mock_info("creator", &[]);

            instantiate(deps.as_mut(), mock_env(), info.clone(), instantiate_msg).unwrap();

            let img = &[1];

            let err = execute(
                deps.as_mut(),
                mock_env(),
                info,
                ExecuteMsg::UploadLogo(Logo::Embedded(EmbeddedLogo::Svg(img.into()))),
            )
            .unwrap_err();

            assert_eq!(err, ContractError::InvalidXmlPreamble {});

            assert_eq!(
                query_marketing_info(deps.as_ref()).unwrap(),
                MarketingInfoResponse {
                    project: Some("Project".to_owned()),
                    description: Some("Description".to_owned()),
                    marketing: Some(Addr::unchecked("creator")),
                    logo: Some(LogoInfo::Url("url".to_owned())),
                }
            );

            let err = query_download_logo(deps.as_ref()).unwrap_err();
            assert!(
                matches!(err, StdError::NotFound { .. }),
                "Expected StdError::NotFound, received {}",
                err
            );
        }
    }

    #[test]
    fn transfer_to_categories() {
        let mut deps = mock_dependencies(&[]);
        let distribute_from = String::from("addr0001");
        let distribute_to = String::from("addr0002");
        let amount = Uint128::from(1000u128);

        do_instantiate(deps.as_mut(), &distribute_from, amount);

        let init_from_balance = get_balance(deps.as_ref(), distribute_from.clone());
        let init_to_balance = get_balance(deps.as_ref(), distribute_to.clone());

        // Transfer the funds
        let mut_deps = &mut deps.as_mut();
        let res = distribute_vested(
            mut_deps,
            distribute_from.clone(),
            distribute_to.clone(),
            amount,
        );

        let calc_new_from_balance = init_from_balance - amount;
        let calc_new_to_balance = init_to_balance + amount;

        let new_from_balance = get_balance(deps.as_ref(), distribute_from);
        let new_to_balance = get_balance(deps.as_ref(), distribute_to);
        // check that the transfer happened
        assert_eq!(calc_new_from_balance, new_from_balance);
        assert_eq!(calc_new_to_balance, new_to_balance);
    }

    #[test]
    fn fail_transfer_to_categories() {
        let mut deps = mock_dependencies(&[]);
        let distribute_from = String::from("addr0001");
        let distribute_to = String::from("addr0002");
        let amount1 = Uint128::from(1000u128);

        do_instantiate(deps.as_mut(), &distribute_from, amount1);

        let init_from_balance = get_balance(deps.as_ref(), distribute_from.clone());
        let init_to_balance = get_balance(deps.as_ref(), distribute_to.clone());

        let amount = init_from_balance + Uint128::from(1000u128);

        // Try to transfer more than the funds available - it should fail
        let mut_deps = &mut deps.as_mut();
        let res = distribute_vested(
            mut_deps,
            distribute_from.clone(),
            distribute_to.clone(),
            amount,
        );

        let new_from_balance = get_balance(deps.as_ref(), distribute_from);
        let new_to_balance = get_balance(deps.as_ref(), distribute_to);

        // check that the transfer did not happen
        assert_eq!(new_from_balance, init_from_balance);
        assert_eq!(new_to_balance, init_to_balance);
    }

    fn get_vesting_details() -> VestingDetails {
        let now = mock_env().block.time;
        let category_address = String::from("addr0002");
        return VestingDetails {
            vesting_start_timestamp: now,
            initial_vesting_count: Uint128::zero(),
            initial_vesting_consumed: Uint128::zero(),
            vesting_periodicity: 300, // In seconds
            vesting_count_per_period: Uint128::from(10u128),
            total_vesting_token_count: Uint128::from(2000u128),
            total_claimed_tokens_till_now: Uint128::zero(),
            last_claimed_timestamp: None,
            tokens_available_to_claim: Uint128::zero(),
            last_vesting_timestamp: None,
            cliff_period: 0, // in months
            category_address: Some(category_address),
        };
    }

    #[test]
    fn test_vesting_at_tge() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today
        println!("now {:?}", now);

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.tokens_available_to_claim += vesting_details.vesting_count_per_period;
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }
    #[test]
    fn test_vesting_at_tge_with_initial_seed() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(1000u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }
    #[test]
    fn test_vesting_at_tge_no_initial_seed_first_interval() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity);
        let vcpp = vesting_details.vesting_count_per_period.u128();
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount.u128(), vcpp);
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_initial_seed_first_interval() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(1010u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }
    #[test]
    fn test_vesting_at_tge_no_initial_seed_2_uncalc_interval() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(20u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }
    #[test]
    fn test_vesting_at_tge_with_initial_seed_2_uncalc_interval() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(1020u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1vested_1uncalc_interval() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();

        vesting_details.tokens_available_to_claim = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp =
            now.minus_seconds(vesting_details.vesting_periodicity * 2);

        vesting_details.last_vesting_timestamp =
            Some(now.minus_seconds(vesting_details.vesting_periodicity));

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(10u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1claimed_1uncalc_interval() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();

        vesting_details.total_claimed_tokens_till_now = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp =
            now.minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);

        vesting_details.last_vesting_timestamp = Some(
            now.minus_seconds(vesting_details.vesting_periodicity)
                .minus_seconds(5),
        );

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(10u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1claimed_half_uncalc_interval() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();

        vesting_details.total_claimed_tokens_till_now = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp =
            now.minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);

        vesting_details.last_vesting_timestamp = Some(
            now.minus_seconds(vesting_details.vesting_periodicity)
                .minus_seconds(5),
        );

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(10u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today
        println!("now {:?}", now);

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.tokens_available_to_claim += vesting_details.vesting_count_per_period;
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_initial_seed_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::zero());
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_first_interval_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity);
        let vcpp = vesting_details.vesting_count_per_period.u128();
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount.u128(), 0u128);
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_initial_seed_first_interval_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_2_uncalc_interval_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_initial_seed_2_uncalc_intervalwith_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1vested_1uncalc_interval_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.tokens_available_to_claim = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp =
            now.minus_seconds(vesting_details.vesting_periodicity * 2);

        vesting_details.last_vesting_timestamp =
            Some(now.minus_seconds(vesting_details.vesting_periodicity));

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1claimed_1uncalc_interval_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.total_claimed_tokens_till_now = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp =
            now.minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);

        vesting_details.last_vesting_timestamp = Some(
            now.minus_seconds(vesting_details.vesting_periodicity)
                .minus_seconds(5),
        );

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1claimed_half_uncalc_interval_with_cliff() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        vesting_details.total_claimed_tokens_till_now = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp =
            now.minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);

        vesting_details.last_vesting_timestamp = Some(
            now.minus_seconds(vesting_details.vesting_periodicity)
                .minus_seconds(5),
        );

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today
        println!("now {:?}", now);

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;
        vesting_details.tokens_available_to_claim += vesting_details.vesting_count_per_period;
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(0u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_initial_seed_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today
                                         // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(1000u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_first_interval_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity);
        let vcpp = vesting_details.vesting_count_per_period.u128();
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount.u128(), vcpp);
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_initial_seed_first_interval_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(1010u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_2_uncalc_interval_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(20u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_with_initial_seed_2_uncalc_interval_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        // vesting at TGE
        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;
        vesting_details.initial_vesting_count = Uint128::from(1000u128);
        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);
        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(1020u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1vested_1uncalc_interval_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;

        vesting_details.tokens_available_to_claim = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);

        vesting_details.last_vesting_timestamp =
            Some(now.minus_seconds(vesting_details.vesting_periodicity));

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(10u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1claimed_1uncalc_interval_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;

        vesting_details.total_claimed_tokens_till_now = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);

        vesting_details.last_vesting_timestamp = Some(
            now.minus_seconds(vesting_details.vesting_periodicity)
                .minus_seconds(5),
        );

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(10u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting_at_tge_no_initial_seed_1claimed_half_uncalc_interval_with_cliff_period_over() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();
        vesting_details.cliff_period = 1;
        let vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(30 * 24 * 60 * 60);
        vesting_details.vesting_start_timestamp = vesting_start_timestamp;

        vesting_details.total_claimed_tokens_till_now = Uint128::from(10u128);

        vesting_details.vesting_start_timestamp = vesting_details
            .vesting_start_timestamp
            .minus_seconds(vesting_details.vesting_periodicity * 2);
        vesting_details.vesting_start_timestamp =
            vesting_details.vesting_start_timestamp.minus_seconds(5);

        vesting_details.last_vesting_timestamp = Some(
            now.minus_seconds(vesting_details.vesting_periodicity)
                .minus_seconds(5),
        );

        let vested_amount = calculate_tokens_for_this_period(
            Addr::unchecked(category_address.clone()),
            now,
            vesting_details,
        );
        match vested_amount {
            Ok(va) => {
                assert_eq!(va.amount, Uint128::from(10u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }

    #[test]
    fn test_vesting() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today

        let mut vesting_details = get_vesting_details();

        // vesting_periodicity = 86400; // in seconds
        // vesting_started_before = 92; // in days
        // cliff_period = 3; // in months
        // vesting_start_timestamp = mock_env()
        //     .block
        //     .time
        //     .minus_seconds(vesting_started_before * 86400);
        // last_vesting_timestamp = mock_env().block.time;
        // total_vesting_token_count = Uint128::from(200u128);
        // total_claimed_tokens_till_now = Uint128::from(0u128);
        // tokens_available_to_claim = Uint128::from(10000u128);
        // let vested_amount = calculate_tokens_for_this_period(
        //     Addr::unchecked(category_address.clone()),
        //     now,
        //     VestingDetails {
        //         vesting_start_timestamp: vesting_start_timestamp,
        //         initial_vesting_count: initial_vesting_count,
        //         initial_vesting_consumed: initial_vesting_consumed,
        //         vesting_periodicity: vesting_periodicity,
        //         vesting_count_per_period: vesting_count_per_period,
        //         total_vesting_token_count: total_vesting_token_count,
        //         total_claimed_tokens_till_now: total_claimed_tokens_till_now,
        //         last_claimed_timestamp: last_claimed_timestamp,
        //         tokens_available_to_claim: tokens_available_to_claim,
        //         last_vesting_timestamp: last_vesting_timestamp,
        //         cliff_period: cliff_period,
        //         category_address: Some(category_address.clone()),
        //     },
        // );
        // match vested_amount {
        //     Ok(va) => {
        //         assert_eq!(va.amount, Uint128::from(200u128));
        //     }
        //     Err(e) => {
        //         assert_eq!(1, 0);
        //     }
        // }

        // vesting_periodicity = 86400; // in seconds
        // vesting_started_before = 90; // in days
        // cliff_period = 3; // in months
        // vesting_start_timestamp = mock_env()
        //     .block
        //     .time
        //     .minus_seconds(vesting_started_before * 86400);
        // last_vesting_timestamp = mock_env().block.time;
        // total_vesting_token_count = Uint128::from(200u128);
        // total_claimed_tokens_till_now = Uint128::from(0u128);
        // tokens_available_to_claim = Uint128::from(10000u128);
        // let vested_amount = calculate_tokens_for_this_period(
        //     Addr::unchecked(category_address.clone()),
        //     now,
        //     VestingDetails {
        //         vesting_start_timestamp: vesting_start_timestamp,
        //         initial_vesting_count: initial_vesting_count,
        //         initial_vesting_consumed: initial_vesting_consumed,
        //         vesting_periodicity: vesting_periodicity,
        //         vesting_count_per_period: vesting_count_per_period,
        //         total_vesting_token_count: total_vesting_token_count,
        //         total_claimed_tokens_till_now: total_claimed_tokens_till_now,
        //         last_claimed_timestamp: last_claimed_timestamp,
        //         tokens_available_to_claim: tokens_available_to_claim,
        //         last_vesting_timestamp: last_vesting_timestamp,
        //         cliff_period: cliff_period,
        //         category_address: Some(category_address.clone()),
        //     },
        // );
        // match vested_amount {
        //     Ok(va) => {
        //         assert_eq!(va.amount, Uint128::zero());
        //     }
        //     Err(e) => {
        //         assert_eq!(1, 0);
        //     }
        // }

        // vesting_periodicity = 86400; // in seconds
        // vesting_started_before = 89; // in days
        // cliff_period = 3; // in months
        // let mut vesting_start_timestamp = mock_env()
        //     .block
        //     .time
        //     .minus_seconds(vesting_started_before * 86400);
        // last_vesting_timestamp = mock_env().block.time;
        // total_vesting_token_count = Uint128::from(200u128);
        // total_claimed_tokens_till_now = Uint128::from(0u128);
        // tokens_available_to_claim = Uint128::from(10000u128);
        // let vested_amount = calculate_tokens_for_this_period(
        //     Addr::unchecked(category_address.clone()),
        //     now,
        //     VestingDetails {
        //         vesting_start_timestamp: vesting_start_timestamp,
        //         initial_vesting_count: initial_vesting_count,
        //         initial_vesting_consumed: initial_vesting_consumed,
        //         vesting_periodicity: vesting_periodicity,
        //         vesting_count_per_period: vesting_count_per_period,
        //         total_vesting_token_count: total_vesting_token_count,
        //         total_claimed_tokens_till_now: total_claimed_tokens_till_now,
        //         last_claimed_timestamp: last_claimed_timestamp,
        //         tokens_available_to_claim: tokens_available_to_claim,
        //         last_vesting_timestamp: last_vesting_timestamp,
        //         cliff_period: cliff_period,
        //         category_address: Some(category_address.clone()),
        //     },
        // );
        // match vested_amount {
        //     Ok(va) => {
        //         assert_eq!(va.amount, Uint128::zero());
        //     }
        //     Err(e) => {
        //         assert_eq!(1, 0);
        //     }
        // }

        // vesting_periodicity = 86400; // in seconds
        // vesting_started_before = 89; // in days
        // cliff_period = 0; // in months
        // let mut vesting_start_seconds =
        //     mock_env().block.time.seconds() - vesting_started_before * 86400;
        // last_vesting_timestamp = mock_env().block.time;
        // total_vesting_token_count = Uint128::from(200u128);
        // total_claimed_tokens_till_now = Uint128::from(0u128);
        // tokens_available_to_claim = Uint128::from(10000u128);
        // let vested_amount = calculate_tokens_for_this_period(
        //     Addr::unchecked(category_address.clone()),
        //     now,
        //     VestingDetails {
        //         vesting_start_timestamp: vesting_start_timestamp,
        //         initial_vesting_count: initial_vesting_count,
        //         initial_vesting_consumed: initial_vesting_consumed,
        //         vesting_periodicity: vesting_periodicity,
        //         vesting_count_per_period: vesting_count_per_period,
        //         total_vesting_token_count: total_vesting_token_count,
        //         total_claimed_tokens_till_now: total_claimed_tokens_till_now,
        //         last_claimed_timestamp: last_claimed_timestamp,
        //         tokens_available_to_claim: tokens_available_to_claim,
        //         last_vesting_timestamp: last_vesting_timestamp,
        //         cliff_period: cliff_period,
        //         category_address: Some(category_address.clone()),
        //     },
        // );
        // match vested_amount {
        //     Ok(va) => {
        //         assert_eq!(va.amount, Uint128::from(8900u128));
        //     }
        //     Err(e) => {
        //         assert_eq!(1, 0);
        //     }
        // }

        // vesting_periodicity = 0; // in seconds - immediately vest
        // vesting_started_before = 89; // in days
        // cliff_period = 0; // in months
        // vesting_start_seconds = mock_env().block.time.seconds() - vesting_started_before * 86400;
        // last_vesting_timestamp = mock_env().block.time;
        // total_vesting_token_count = Uint128::from(200u128);
        // total_claimed_tokens_till_now = Uint128::from(0u128);
        // tokens_available_to_claim = Uint128::from(10000u128);
        // let vested_amount = calculate_tokens_for_this_period(
        //     Addr::unchecked(category_address.clone()),
        //     now,
        //     VestingDetails {
        //         vesting_start_timestamp: vesting_start_timestamp,
        //         initial_vesting_count: initial_vesting_count,
        //         initial_vesting_consumed: initial_vesting_consumed,
        //         vesting_periodicity: vesting_periodicity,
        //         vesting_count_per_period: vesting_count_per_period,
        //         total_vesting_token_count: total_vesting_token_count,
        //         total_claimed_tokens_till_now: total_claimed_tokens_till_now,
        //         last_claimed_timestamp: last_claimed_timestamp,
        //         tokens_available_to_claim: tokens_available_to_claim,
        //         last_vesting_timestamp: last_vesting_timestamp,
        //         cliff_period: cliff_period,
        //         category_address: Some(category_address.clone()),
        //     },
        // );
        // match vested_amount {
        //     Ok(va) => {
        //         assert_eq!(va.amount, Uint128::zero());
        //     }
        //     Err(e) => {
        //         assert_eq!(1, 0);
        //     }
        // }
    }
}

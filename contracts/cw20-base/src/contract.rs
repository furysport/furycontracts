#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, to_binary, Addr, Attribute, Binary, Deps, DepsMut, Env, MessageInfo, OverflowError,
    OverflowOperation, Response, StdError, StdResult, Timestamp, Uint128,
};

use cw2::set_contract_version;
use cw20::{
    AllowanceResponse, BalanceResponse, Cw20Coin, Cw20ReceiveMsg, DownloadLogoResponse,
    EmbeddedLogo, Expiration, Logo, LogoInfo, MarketingInfoResponse, MinterResponse,
    TokenInfoResponse,
};

use crate::allowances::{
    deduct_allowance, execute_burn_from, execute_decrease_allowance, execute_increase_allowance,
    execute_send_from, execute_transfer_from, query_allowance,
};
use crate::enumerable::{query_all_accounts, query_all_allowances};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{
    MinterData, TokenInfo, VestingDetails, ALLOWANCES, BALANCES, LOGO, MARKETING_INFO, TOKEN_INFO,
    VESTING_DETAILS,
};
use log::{info, trace, warn};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw20-base";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const LOGO_SIZE_CAP: usize = 5 * 1024;

const MAIN_WALLET: &str = "terra1t3czdl5h4w4qwgkzs80fdstj0z7rfv9v2j6uh3";

const GAMIFIED_AIRDROP_WALLET: &str = "terra144tlfg2zqtphwfpwrtqvvzyl362a406v2r08rj";

const ADVISOR_WALLET: &str = "terra17yvv240qq4c6alyrcgvk6wnf402u8gp3d3nxgm";

const PRIVATE_SALE_WALLET: &str = "terra1e7maadq8sdk6aqaz2vwzxrsfr3tu8svz2sw850";

const NITIN_WALLET: &str = "terra1jq6ffpwfj08rx9wxu02ussv6pequm0tkzfjq22";

const AJAY_WALLET: &str = "terra1mk9nav0hv5r8f7dwymjxml8yft78qkt6fuqae7";

const SAMEER_WALLET: &str = "terra1cm7rklc6m2r8klnqj505ymntf3xrqtatthc64e";

/// Checks if data starts with XML preamble
fn verify_xml_preamble(data: &[u8]) -> Result<(), ContractError> {
    // The easiest way to perform this check would be just match on regex, however regex
    // compilation is heavy and probably not worth it.

    let preamble = data
        .split_inclusive(|c| *c == b'>')
        .next()
        .ok_or(ContractError::InvalidXmlPreamble {})?;

    const PREFIX: &[u8] = b"<?xml ";
    const POSTFIX: &[u8] = b"?>";

    if !(preamble.starts_with(PREFIX) && preamble.ends_with(POSTFIX)) {
        Err(ContractError::InvalidXmlPreamble {})
    } else {
        Ok(())
    }

    // Additionally attributes format could be validated as they are well defined, as well as
    // comments presence inside of preable, but it is probably not worth it.
}

/// Validates XML logo
fn verify_xml_logo(logo: &[u8]) -> Result<(), ContractError> {
    verify_xml_preamble(logo)?;

    if logo.len() > LOGO_SIZE_CAP {
        Err(ContractError::LogoTooBig {})
    } else {
        Ok(())
    }
}

/// Validates png logo
fn verify_png_logo(logo: &[u8]) -> Result<(), ContractError> {
    // PNG header format:
    // 0x89 - magic byte, out of ASCII table to fail on 7-bit systems
    // "PNG" ascii representation
    // [0x0d, 0x0a] - dos style line ending
    // 0x1a - dos control character, stop displaying rest of the file
    // 0x0a - unix style line ending
    const HEADER: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if logo.len() > LOGO_SIZE_CAP {
        Err(ContractError::LogoTooBig {})
    } else if !logo.starts_with(&HEADER) {
        Err(ContractError::InvalidPngHeader {})
    } else {
        Ok(())
    }
}

/// Checks if passed logo is correct, and if not, returns an error
fn verify_logo(logo: &Logo) -> Result<(), ContractError> {
    match logo {
        Logo::Embedded(EmbeddedLogo::Svg(logo)) => verify_xml_logo(&logo),
        Logo::Embedded(EmbeddedLogo::Png(logo)) => verify_png_logo(&logo),
        Logo::Url(_) => Ok(()), // Any reasonable url validation would be regex based, probably not worth it
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    // check valid token info
    msg.validate()?;
    // create initial accounts
    let total_supply = create_accounts(&mut deps, &msg.initial_balances)?;

    if let Some(limit) = msg.get_cap() {
        if total_supply > limit {
            return Err(StdError::generic_err("Initial supply greater than cap").into());
        }
    }

    let mint = match msg.mint {
        Some(m) => Some(MinterData {
            minter: deps.api.addr_validate(&m.minter)?,
            cap: m.cap,
        }),
        None => None,
    };

    // store token info
    let data = TokenInfo {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
        total_supply,
        mint,
    };
    TOKEN_INFO.save(deps.storage, &data)?;

    if let Some(marketing) = msg.marketing {
        let logo = if let Some(logo) = marketing.logo {
            verify_logo(&logo)?;
            LOGO.save(deps.storage, &logo)?;

            match logo {
                Logo::Url(url) => Some(LogoInfo::Url(url)),
                Logo::Embedded(_) => Some(LogoInfo::Embedded),
            }
        } else {
            None
        };

        let data = MarketingInfoResponse {
            project: marketing.project,
            description: marketing.description,
            marketing: marketing
                .marketing
                .map(|addr| deps.api.addr_validate(&addr))
                .transpose()?,
            logo,
        };
        MARKETING_INFO.save(deps.storage, &data)?;
    }

    instantiate_category_vesting_schedules(deps, env)?;

    Ok(Response::default())
}

fn instantiate_category_vesting_schedules(
    deps: DepsMut,
    env: Env,
) -> Result<Response, ContractError> {
    let vesting_start_timestamp = env.block.time;
    let ga_address = deps.api.addr_validate(GAMIFIED_AIRDROP_WALLET)?;
    let ga_vesting_details = VestingDetails {
        vesting_start_timestamp: vesting_start_timestamp,
        initial_vesting_count: Uint128::from(3_950_000_000_000u128),
        initial_vesting_consumed: Uint128::zero(),
        vesting_periodicity: 5 * 60, // (every 5 minutes) 24 * 60 * 60, // (daily)
        vesting_count_per_period: Uint128::from(69_490_740_740u128),
        total_vesting_token_count: Uint128::from(79_000_000_000_000u128),
        total_claimed_tokens_till_now: Uint128::zero(),
        last_claimed_timestamp: vesting_start_timestamp,
        tokens_available_to_claim: Uint128::zero(),
        last_vesting_timestamp: vesting_start_timestamp,
        cliff_period: 0,
        category_address: None,
    };
    VESTING_DETAILS.save(deps.storage, &ga_address, &ga_vesting_details)?;

    //Save vesting details for advisors
    let advisors_address = deps.api.addr_validate(ADVISOR_WALLET)?;
    let advisors_vesting_details = VestingDetails {
        vesting_start_timestamp: vesting_start_timestamp,
        initial_vesting_count: Uint128::zero(),
        initial_vesting_consumed: Uint128::zero(),
        vesting_periodicity: 5 * 60, // (every 5 minutes) 24 * 60 * 60, // (daily)24 * 60 * 60, //daily
        vesting_count_per_period: Uint128::from(40_833_333_333u128),
        total_vesting_token_count: Uint128::from(14_700_000_000_000u128),
        total_claimed_tokens_till_now: Uint128::zero(),
        last_claimed_timestamp: vesting_start_timestamp,
        tokens_available_to_claim: Uint128::zero(),
        last_vesting_timestamp: vesting_start_timestamp,
        cliff_period: 4,
        category_address: None,
    };
    VESTING_DETAILS.save(deps.storage, &advisors_address, &advisors_vesting_details)?;

    //Save vesting details for Private Sale
    let priv_sale_address = deps.api.addr_validate(PRIVATE_SALE_WALLET)?;
    let priv_sale_vesting_details = VestingDetails {
        vesting_start_timestamp: vesting_start_timestamp,
        initial_vesting_count: Uint128::from(4_200_000_000_000u128),
        initial_vesting_consumed: Uint128::zero(),
        vesting_periodicity: 5 * 60, // (every 5 minutes) 24 * 60 * 60,24 * 60 * 60, //daily
        vesting_count_per_period: Uint128::from(210_000_000_000u128),
        total_vesting_token_count: Uint128::from(42_000_000_000_000u128),
        total_claimed_tokens_till_now: Uint128::zero(),
        last_claimed_timestamp: vesting_start_timestamp,
        tokens_available_to_claim: Uint128::zero(),
        last_vesting_timestamp: vesting_start_timestamp,
        cliff_period: 0,
        category_address: None,
    };
    VESTING_DETAILS.save(deps.storage, &priv_sale_address, &priv_sale_vesting_details)?;

    instantiate_sub_category_vesting_schedules(
        deps,
        env,
        vec![
            (
                PRIVATE_SALE_WALLET, //private-sale
                NITIN_WALLET,        //nitin-wallet
                Uint128::from(16_800_000_000_000u128),
            ),
            (
                PRIVATE_SALE_WALLET, //private-sale
                AJAY_WALLET,         //ajay-wallet
                Uint128::from(12_600_000_000_000u128),
            ),
            (
                PRIVATE_SALE_WALLET, //private-sale
                SAMEER_WALLET,       //sameer-wallet
                Uint128::from(12_600_000_000_000u128),
            ),
        ],
    )?;

    Ok(Response::default())
}

fn instantiate_sub_category_vesting_schedules(
    deps: DepsMut,
    env: Env,
    investors: Vec<(&str, &str, Uint128)>,
) -> Result<Response, ContractError> {
    for investor in investors {
        //Check if the amount is greater than zero
        if investor.2 > Uint128::zero() {
            //Get the category address
            let category_address = deps.api.addr_validate(investor.0)?;
            //Get the category vesting details
            let cat_vesting_details = VESTING_DETAILS.may_load(deps.storage, &category_address)?;
            match cat_vesting_details {
                Some(cvd) => {
                    // Get the investor address.
                    let address = deps.api.addr_validate(investor.1)?;
                    let investment = investor.2;
                    let category_max_amount = cvd.total_vesting_token_count;
                    let sub_cat_vesting_details = VestingDetails {
                        vesting_start_timestamp: cvd.vesting_start_timestamp,
                        initial_vesting_count: cvd
                            .initial_vesting_count
                            .checked_mul(investment)
                            .unwrap()
                            .checked_div(category_max_amount)
                            .unwrap(),
                        initial_vesting_consumed: Uint128::zero(),
                        vesting_periodicity: cvd.vesting_periodicity,
                        vesting_count_per_period: cvd
                            .vesting_count_per_period
                            .checked_mul(investment)
                            .unwrap()
                            .checked_div(category_max_amount)
                            .unwrap(),
                        total_vesting_token_count: investment,
                        total_claimed_tokens_till_now: Uint128::zero(),
                        last_claimed_timestamp: cvd.vesting_start_timestamp,
                        tokens_available_to_claim: Uint128::zero(),
                        last_vesting_timestamp: cvd.vesting_start_timestamp,
                        cliff_period: cvd.cliff_period,
                        category_address: Some(String::from(investor.0)),
                    };
                    VESTING_DETAILS.save(deps.storage, &address, &sub_cat_vesting_details)?;
                }
                None => {
                    let mut err_msg = String::from("No vesting details found for address ");
                    err_msg.push_str(investor.0);
                    return Err(ContractError::Std(StdError::NotFound {
                        kind: String::from(err_msg),
                    }));
                }
            };
        };
    }
    Ok(Response::default())
}

pub fn create_accounts(deps: &mut DepsMut, accounts: &[Cw20Coin]) -> StdResult<Uint128> {
    let mut total_supply = Uint128::zero();
    for row in accounts {
        let address = deps.api.addr_validate(&row.address)?;
        BALANCES.save(deps.storage, &address, &row.amount)?;
        total_supply += row.amount;
    }
    Ok(total_supply)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Transfer { recipient, amount } => {
            execute_transfer(deps, env, info, recipient, amount)
        }
        ExecuteMsg::Burn { amount } => execute_burn(deps, env, info, amount),
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => execute_send(deps, env, info, contract, amount, msg),
        ExecuteMsg::Mint { recipient, amount } => execute_mint(deps, env, info, recipient, amount),
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
        ExecuteMsg::UpdateMarketing {
            project,
            description,
            marketing,
        } => execute_update_marketing(deps, env, info, project, description, marketing),
        ExecuteMsg::UploadLogo(logo) => execute_upload_logo(deps, env, info, logo),
        ExecuteMsg::PeriodicallyTransferToCategories {} => {
            periodically_transfer_to_categories(deps, env, info)
        }
        ExecuteMsg::PeriodicallyCalculateVesting {} => {
            periodically_calculate_vesting(deps, env, info)
        }
        ExecuteMsg::ClaimVestedTokens { amount } => claim_vested_tokens(deps, env, info, amount),
    }
}

pub fn execute_transfer(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let rcpt_addr = deps.api.addr_validate(&recipient)?;

    BALANCES.update(
        deps.storage,
        &info.sender,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    BALANCES.update(
        deps.storage,
        &rcpt_addr,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    let res = Response::new()
        .add_attribute("action", "transfer")
        .add_attribute("from", info.sender)
        .add_attribute("to", recipient)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn execute_burn(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    // lower balance
    BALANCES.update(
        deps.storage,
        &info.sender,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    // reduce total_supply
    TOKEN_INFO.update(deps.storage, |mut info| -> StdResult<_> {
        info.total_supply = info.total_supply.checked_sub(amount)?;
        Ok(info)
    })?;

    let res = Response::new()
        .add_attribute("action", "burn")
        .add_attribute("from", info.sender)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn execute_mint(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let mut config = TOKEN_INFO.load(deps.storage)?;
    if config.mint.is_none() || config.mint.as_ref().unwrap().minter != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // update supply and enforce cap
    config.total_supply += amount;
    if let Some(limit) = config.get_cap() {
        if config.total_supply > limit {
            return Err(ContractError::CannotExceedCap {});
        }
    }
    TOKEN_INFO.save(deps.storage, &config)?;

    // add amount to recipient balance
    let rcpt_addr = deps.api.addr_validate(&recipient)?;
    BALANCES.update(
        deps.storage,
        &rcpt_addr,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    let res = Response::new()
        .add_attribute("action", "mint")
        .add_attribute("to", recipient)
        .add_attribute("amount", amount);
    Ok(res)
}

pub fn execute_send(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    contract: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let rcpt_addr = deps.api.addr_validate(&contract)?;

    // move the tokens to the contract
    BALANCES.update(
        deps.storage,
        &info.sender,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    BALANCES.update(
        deps.storage,
        &rcpt_addr,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    let res = Response::new()
        .add_attribute("action", "send")
        .add_attribute("from", &info.sender)
        .add_attribute("to", &contract)
        .add_attribute("amount", amount)
        .add_message(
            Cw20ReceiveMsg {
                sender: info.sender.into(),
                amount,
                msg,
            }
            .into_cosmos_msg(contract)?,
        );
    Ok(res)
}

pub fn execute_update_marketing(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    project: Option<String>,
    description: Option<String>,
    marketing: Option<String>,
) -> Result<Response, ContractError> {
    let mut marketing_info = MARKETING_INFO
        .may_load(deps.storage)?
        .ok_or(ContractError::Unauthorized {})?;

    if marketing_info
        .marketing
        .as_ref()
        .ok_or(ContractError::Unauthorized {})?
        != &info.sender
    {
        return Err(ContractError::Unauthorized {});
    }

    match project {
        Some(empty) if empty.trim().is_empty() => marketing_info.project = None,
        Some(project) => marketing_info.project = Some(project),
        None => (),
    }

    match description {
        Some(empty) if empty.trim().is_empty() => marketing_info.description = None,
        Some(description) => marketing_info.description = Some(description),
        None => (),
    }

    match marketing {
        Some(empty) if empty.trim().is_empty() => marketing_info.marketing = None,
        Some(marketing) => marketing_info.marketing = Some(deps.api.addr_validate(&marketing)?),
        None => (),
    }

    if marketing_info.project.is_none()
        && marketing_info.description.is_none()
        && marketing_info.marketing.is_none()
        && marketing_info.logo.is_none()
    {
        MARKETING_INFO.remove(deps.storage);
    } else {
        MARKETING_INFO.save(deps.storage, &marketing_info)?;
    }

    let res = Response::new().add_attribute("action", "update_marketing");
    Ok(res)
}

pub fn execute_upload_logo(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    logo: Logo,
) -> Result<Response, ContractError> {
    let mut marketing_info = MARKETING_INFO
        .may_load(deps.storage)?
        .ok_or(ContractError::Unauthorized {})?;

    verify_logo(&logo)?;

    if marketing_info
        .marketing
        .as_ref()
        .ok_or(ContractError::Unauthorized {})?
        != &info.sender
    {
        return Err(ContractError::Unauthorized {});
    }

    LOGO.save(deps.storage, &logo)?;

    let logo_info = match logo {
        Logo::Url(url) => LogoInfo::Url(url),
        Logo::Embedded(_) => LogoInfo::Embedded,
    };

    marketing_info.logo = Some(logo_info);
    MARKETING_INFO.save(deps.storage, &marketing_info)?;

    let res = Response::new().add_attribute("action", "upload_logo");
    Ok(res)
}

fn periodically_transfer_to_categories(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    //capture the current system time
    let now = env.block.time;

    let distribute_from = String::from(MAIN_WALLET);
    let address = deps.api.addr_validate(distribute_from.clone().as_str())?;

    //Check if the sender (one who is executing this contract) is main
    if address != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Fetch all tokens that can be distributed as per vesting logic
    let distribution_details = populate_transfer_details(&deps, now)?;

    // Calculate the total amount to be vested
    let total_transfer_amount = calculate_total_distribution(&distribution_details);
    //Get the balance available in main wallet
    let balance = BALANCES
        .may_load(deps.storage, &address)?
        .unwrap_or_default();

    //Check if there is sufficient balance with main wallet
    // return error otherwise
    if balance < total_transfer_amount {
        return Err(ContractError::Std(StdError::overflow(OverflowError::new(
            OverflowOperation::Sub,
            balance,
            total_transfer_amount,
        ))));
    }
    let mut attribs: Vec<Attribute> = Vec::new();
    for elem in distribution_details {
        // Transfer the funds
        let res = distribute_vested(
            &mut deps,
            distribute_from.clone(),
            elem.spender_address.clone(),
            elem.amount,
        )?;
        for attrib in res.attributes {
            attribs.push(attrib);
        }
        // Save distribution information
        let res = update_vesting_details(
            &mut deps,
            elem.spender_address.clone(),
            env.block.time,
            Some(elem),
            None,
        )?;
        for attrib in res.attributes {
            attribs.push(attrib);
        }
        attribs.push(Attribute::new("kuchha hua", "Pata nahi"));
    }
    Ok(Response::new().add_attributes(attribs))
}

fn distribute_vested(
    deps: &mut DepsMut,
    sender: String,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let rcpt_addr = deps.api.addr_validate(&recipient)?;
    let sender_addr = deps.api.addr_validate(&sender)?;

    BALANCES.update(
        deps.storage,
        &sender_addr,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    BALANCES.update(
        deps.storage,
        &rcpt_addr,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    let res = Response::new()
        .add_attribute("action", "transfer")
        .add_attribute("from", sender)
        .add_attribute("to", recipient)
        .add_attribute("amount", amount);
    Ok(res)
}

fn claim_vested_tokens(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    //Get vesting information for the sender of this message
    let vd = VESTING_DETAILS.may_load(deps.storage, &info.sender)?;
    match vd {
        Some(vd) => {
            let owner_addr_str = vd.category_address;
            match owner_addr_str {
                Some(owner_addr_str) => {
                    let owner_addr = deps.api.addr_validate(&owner_addr_str)?;
                    // deduct allowance before doing anything else have enough allowance
                    deduct_allowance(deps.storage, &owner_addr, &info.sender, &env.block, amount)?;

                    // deduct amount form category address
                    BALANCES.update(
                        deps.storage,
                        &owner_addr,
                        |balance: Option<Uint128>| -> StdResult<_> {
                            Ok(balance.unwrap_or_default().checked_sub(amount)?)
                        },
                    )?;
                    // add amount form sender address
                    BALANCES.update(
                        deps.storage,
                        &info.sender,
                        |balance: Option<Uint128>| -> StdResult<_> {
                            Ok(balance.unwrap_or_default() + amount)
                        },
                    )?;

                    //Update vesting info for sender
                    VESTING_DETAILS.update(deps.storage, &info.sender, |vd| -> StdResult<_> {
                        match vd {
                            Some(mut v) => {
                                v.total_claimed_tokens_till_now =
                                    v.total_claimed_tokens_till_now + amount;
                                v.tokens_available_to_claim = v.tokens_available_to_claim - amount;
                                v.last_claimed_timestamp = env.block.time;
                                Ok(v)
                            }
                            None => Err(StdError::GenericErr {
                                msg: String::from("Vesting Details not found"),
                            }),
                        }
                    })?;

                    let res = Response::new().add_attributes(vec![
                        attr("action", "transfer_from"),
                        attr("from", owner_addr),
                        attr("to", info.sender.to_string().clone()),
                        attr("by", info.sender),
                        attr("amount", amount),
                    ]);
                    return Ok(res);
                }
                None => {
                    return Err(ContractError::Std(StdError::NotFound {
                        kind: String::from("No parent category found"),
                    }))
                }
            }
        }
        None => {
            return Err(ContractError::Std(StdError::NotFound {
                kind: String::from("No vesting details found"),
            }))
        }
    };
}

fn periodically_calculate_vesting(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let now = env.block.time;
    let invoker = String::from(MAIN_WALLET);
    let address = deps.api.addr_validate(invoker.clone().as_str())?;

    //Check if the sender (one who is executing this contract) is main
    if address != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    // Fetch all tokens that can be vested as per vesting logic
    let vested_details = populate_vesting_details(&deps, now)?;
    // Calculate the total amount to be vested
    let total_vested_amount = calculate_total_distribution(&vested_details);
    //Get the balance available in main wallet
    let balance = BALANCES
        .may_load(deps.storage, &address)?
        .unwrap_or_default();
    //Check if there is sufficient balance with main wallet
    // return error otherwise
    if balance < total_vested_amount {
        return Err(ContractError::Std(StdError::overflow(OverflowError::new(
            OverflowOperation::Sub,
            balance,
            total_vested_amount,
        ))));
    }
    let mut attribs: Vec<Attribute> = Vec::new();
    for elem in vested_details {
        if elem.amount.u128() > 0 {
            //Update the allowancs
            let spender_addr = deps.api.addr_validate(&elem.spender_address)?;
            if spender_addr == info.sender {
                return Err(ContractError::CannotSetOwnAccount {});
            }
            let category_address = elem.clone().category_address.unwrap_or_default();
            let owner_addr = deps.api.addr_validate(&category_address)?;
            let key = (&owner_addr, &spender_addr);
            let allowance = ALLOWANCES.load(deps.storage, key);
            match allowance {
                Ok(mut a) => {
                    // update the new amount
                    a.allowance = a
                        .allowance
                        .checked_add(elem.amount)
                        .map_err(StdError::overflow)?;
                    ALLOWANCES.save(deps.storage, key, &a)?;
                }
                Err(_) => {
                    // Add the new amount
                    let allowance_response = AllowanceResponse {
                        allowance: elem.amount,
                        expires: Expiration::Never {},
                    };
                    ALLOWANCES.save(deps.storage, key, &allowance_response)?;
                }
            }
            //Save the vesting details
            let res = update_vesting_details(
                &mut deps,
                elem.clone().spender_address,
                env.block.time,
                None,
                Some(elem),
            )?;
            for attrib in res.attributes {
                attribs.push(attrib);
            }
        }
    }
    Ok(Response::new().add_attributes(attribs))
}

fn update_vesting_details(
    deps: &mut DepsMut,
    address: String,
    execution_timestamp: Timestamp,
    transferred: Option<VestingInfo>,
    vestable: Option<VestingInfo>,
) -> Result<Response, ContractError> {
    let addr = deps.api.addr_validate(&address)?;
    match transferred {
        Some(transferred) => {
            VESTING_DETAILS.update(deps.storage, &addr, |vd| -> StdResult<_> {
                match vd {
                    Some(mut v) => {
                        let new_count = v.total_claimed_tokens_till_now + transferred.amount;
                        if new_count <= v.total_vesting_token_count {
                            v.total_claimed_tokens_till_now = new_count;
                            v.last_vesting_timestamp = execution_timestamp;
                            v.last_claimed_timestamp = execution_timestamp;
                        }
                        v.initial_vesting_consumed = v.initial_vesting_count;
                        Ok(v)
                    }
                    None => Err(StdError::GenericErr {
                        msg: String::from("Vesting Details not found"),
                    }),
                }
            })?;
        }
        None => (),
    }
    match vestable {
        Some(vestable) => {
            VESTING_DETAILS.update(deps.storage, &addr, |vd| -> StdResult<_> {
                match vd {
                    Some(mut v) => {
                        let new_count = v.tokens_available_to_claim + vestable.amount;
                        let mut new_vestable_tokens = new_count;
                        if v.total_claimed_tokens_till_now + new_count > v.total_vesting_token_count
                        {
                            new_vestable_tokens =
                                v.total_vesting_token_count - v.total_claimed_tokens_till_now;
                        }
                        v.tokens_available_to_claim += new_vestable_tokens;
                        if v.vesting_start_timestamp.nanos() == v.last_vesting_timestamp.nanos() {
                            v.tokens_available_to_claim = v.initial_vesting_count;
                            v.initial_vesting_consumed = v.initial_vesting_count;
                        }
                        v.last_vesting_timestamp = execution_timestamp;
                        Ok(v)
                    }
                    None => Err(StdError::GenericErr {
                        msg: String::from("Vesting Details not found"),
                    }),
                }
            })?;
        }
        None => (),
    }
    Ok(Response::default())
}

fn populate_transfer_details(
    deps: &DepsMut,
    now: Timestamp,
) -> Result<Vec<VestingInfo>, ContractError> {
    let mut distribution_details: Vec<VestingInfo> = Vec::new();

    let ga_address = String::from(GAMIFIED_AIRDROP_WALLET);
    let ga_vesting_info = calculate_vesting_for_now(deps, ga_address, now)?;
    distribution_details.push(ga_vesting_info);

    //Tokens to be transferred to Private Sale wallet
    let ps_address = String::from(PRIVATE_SALE_WALLET);
    let ps_vesting_info = calculate_vesting_for_now(deps, ps_address, now)?;
    distribution_details.push(ps_vesting_info);
    Ok(distribution_details)
}

fn populate_vesting_details(
    deps: &DepsMut,
    now: Timestamp,
) -> Result<Vec<VestingInfo>, ContractError> {
    let mut distribution_details: Vec<VestingInfo> = Vec::new();

    // For Nitin
    let nitin_address = String::from(NITIN_WALLET);
    let nitin_vesting_info = calculate_vesting_for_now(deps, nitin_address, now)?;
    if nitin_vesting_info.amount.u128() > 0 {
        distribution_details.push(nitin_vesting_info);
    }

    // For Ajay
    let ajay_address = String::from(AJAY_WALLET);
    let ajay_vesting_info = calculate_vesting_for_now(deps, ajay_address, now)?;
    if ajay_vesting_info.amount.u128() > 0 {
        distribution_details.push(ajay_vesting_info);
    }

    // For Sameer
    let sameer_address = String::from(SAMEER_WALLET);
    let sameer_vesting_info = calculate_vesting_for_now(deps, sameer_address, now)?;
    if sameer_vesting_info.amount.u128() > 0 {
        distribution_details.push(sameer_vesting_info);
    }

    Ok(distribution_details)
}

#[derive(Clone, Default)]
pub struct VestingInfo {
    pub spender_address: String,
    pub category_address: Option<String>,
    pub amount: Uint128,
}

fn calculate_vesting_for_now(
    deps: &DepsMut,
    address: String,
    now: Timestamp,
) -> Result<VestingInfo, ContractError> {
    let mut message = String::from("entered calculate_vesting_for_now: ");
    let wallet_address = deps.api.addr_validate(&address)?;
    message.push_str(" address is valid ");
    let vested_detais = VESTING_DETAILS.may_load(deps.storage, &wallet_address);
    match vested_detais {
        Ok(vested_detais) => {
            message.push_str(" Vesting details found ");
            let vd = vested_detais.unwrap();
            let vesting_info = calculate_tokens_for_this_period(wallet_address, now, vd)?;

            Ok(vesting_info)
        }
        Err(e) => Err(ContractError::Std(StdError::GenericErr {
            msg: e.to_string(),
        })),
    }
}

fn calculate_tokens_for_this_period(
    wallet_address: Addr,
    now: Timestamp,
    vd: VestingDetails,
) -> Result<VestingInfo, ContractError> {
    println!("entered calculate_vesting_for_now: ");
    let mut seconds_lapsed = 0;
    let now_seconds: u64 = now.seconds();
    println!("now_seconds = {}", now_seconds);
    let vesting_start_seconds = vd.vesting_start_timestamp.seconds();
    println!("vesting_start_seconds = {:?}", vesting_start_seconds);
    println!("vd.vesting_periodicity = {}", vd.vesting_periodicity);
    if vd.vesting_periodicity > 0 {
        let mut vesting_intervals = 0;
        if now_seconds > (vesting_start_seconds + (vd.cliff_period * 30 * 24 * 60 * 60)) {
            // the now time is greater (ahead) of vesting start + cliff
            seconds_lapsed =
                now_seconds - (vesting_start_seconds + (vd.cliff_period * 30 * 24 * 60 * 60));
            println!("seconds_lapsed_1 = {}", seconds_lapsed);
            let total_vesting_intervals = seconds_lapsed / vd.vesting_periodicity;
            println!("total_vesting_intervals = {}", total_vesting_intervals);
            println!(
                "vd.last_vesting_timestamp.seconds() = {}",
                vd.last_vesting_timestamp.seconds()
            );
            println!("vesting_start_seconds = {}", vesting_start_seconds);
            println!("vd.cliff_period = {}", vd.cliff_period);
            let seconds_till_last_vesting = vd.last_vesting_timestamp.seconds()
                - (vesting_start_seconds + vd.cliff_period * 30 * 24 * 60 * 60);
            println!("seconds_till_last_vesting = {}", seconds_till_last_vesting);
            let total_vested_intervals = (seconds_till_last_vesting) / vd.vesting_periodicity;
            println!("total_vested_intervals = {}", total_vested_intervals);

            vesting_intervals = total_vesting_intervals - total_vested_intervals;
            println!("vesting_intervals = {}", vesting_intervals);
        }
        let tokens_for_this_period_result = vd
            .vesting_count_per_period
            .checked_mul(Uint128::from(vesting_intervals));
        let mut tokens_for_this_period: Uint128;
        match tokens_for_this_period_result {
            Ok(tokens) => {
                println!("tokens = {}", tokens);
                //Add the initial vested tokens that are not yet claimed
                tokens_for_this_period = tokens;
            }
            Err(e) => {
                println!("error = {:?}", e);
                let mut message = String::from("error = ");
                message.push_str(e.to_string().as_str());
                tokens_for_this_period = Uint128::zero();
            }
        }
        if vd.total_vesting_token_count
            < (tokens_for_this_period
                + vd.total_claimed_tokens_till_now
                + vd.tokens_available_to_claim)
        {
            tokens_for_this_period = vd.total_vesting_token_count
                - (vd.total_claimed_tokens_till_now + vd.tokens_available_to_claim);
        }
        println!("tokens_for_this_period = {}", tokens_for_this_period);
        Ok(VestingInfo {
            spender_address: wallet_address.to_string(),
            category_address: vd.category_address,
            amount: tokens_for_this_period
                + (vd.initial_vesting_count - vd.initial_vesting_count),
        })
    } else {
        return Err(ContractError::Std(StdError::generic_err(String::from(
            "No vesting for this address",
        ))));
    }
}

fn calculate_total_distribution(distribution_details: &Vec<VestingInfo>) -> Uint128 {
    let mut total = Uint128::zero();
    for elem in distribution_details {
        total += elem.amount;
    }
    return total;
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::TokenInfo {} => to_binary(&query_token_info(deps)?),
        QueryMsg::Minter {} => to_binary(&query_minter(deps)?),
        QueryMsg::Allowance { owner, spender } => {
            to_binary(&query_allowance(deps, owner, spender)?)
        }
        QueryMsg::AllAllowances {
            owner,
            start_after,
            limit,
        } => to_binary(&query_all_allowances(deps, owner, start_after, limit)?),
        QueryMsg::AllAccounts { start_after, limit } => {
            to_binary(&query_all_accounts(deps, start_after, limit)?)
        }
        QueryMsg::MarketingInfo {} => to_binary(&query_marketing_info(deps)?),
        QueryMsg::DownloadLogo {} => to_binary(&query_download_logo(deps)?),
        QueryMsg::VestingDetails { address } => to_binary(&query_vesting_details(deps, address)?),
    }
}

pub fn query_balance(deps: Deps, address: String) -> StdResult<BalanceResponse> {
    let address = deps.api.addr_validate(&address)?;
    let balance = BALANCES
        .may_load(deps.storage, &address)?
        .unwrap_or_default();
    Ok(BalanceResponse { balance })
}
pub fn query_vesting_details(deps: Deps, address: String) -> StdResult<VestingDetails> {
    let address = deps.api.addr_validate(&address)?;
    let vd = VESTING_DETAILS.may_load(deps.storage, &address)?;
    match vd {
        Some(vd) => return Ok(vd),
        None => return Err(StdError::generic_err("No vesting details found")),
    };
}
pub fn query_token_info(deps: Deps) -> StdResult<TokenInfoResponse> {
    let info = TOKEN_INFO.load(deps.storage)?;
    let res = TokenInfoResponse {
        name: info.name,
        symbol: info.symbol,
        decimals: info.decimals,
        total_supply: info.total_supply,
    };
    Ok(res)
}

pub fn query_minter(deps: Deps) -> StdResult<Option<MinterResponse>> {
    let meta = TOKEN_INFO.load(deps.storage)?;
    let minter = match meta.mint {
        Some(m) => Some(MinterResponse {
            minter: m.minter.into(),
            cap: m.cap,
        }),
        None => None,
    };
    Ok(minter)
}

pub fn query_marketing_info(deps: Deps) -> StdResult<MarketingInfoResponse> {
    Ok(MARKETING_INFO.may_load(deps.storage)?.unwrap_or_default())
}

pub fn query_download_logo(deps: Deps) -> StdResult<DownloadLogoResponse> {
    let logo = LOGO.load(deps.storage)?;
    match logo {
        Logo::Embedded(EmbeddedLogo::Svg(logo)) => Ok(DownloadLogoResponse {
            mime_type: "image/svg+xml".to_owned(),
            data: logo,
        }),
        Logo::Embedded(EmbeddedLogo::Png(logo)) => Ok(DownloadLogoResponse {
            mime_type: "image/png".to_owned(),
            data: logo,
        }),
        Logo::Url(_) => Err(StdError::not_found("logo")),
    }
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
            category_address: Some(String::from("terra1e7maadq8sdk6aqaz2vwzxrsfr3tu8svz2sw850")),
            cliff_period: 0,
            initial_vesting_consumed: Uint128::zero(),
            initial_vesting_count: Uint128::from(1680000000000u128),
            last_claimed_timestamp: Timestamp::from_nanos(1635835145194509256u64),
            last_vesting_timestamp: Timestamp::from_nanos(1635835210499741601u64),
            tokens_available_to_claim: Uint128::from(1680000000000u128),
            total_claimed_tokens_till_now: Uint128::zero(),
            total_vesting_token_count: Uint128::from(16800000000000u128),
            vesting_count_per_period: Uint128::from(84000000000u128),
            vesting_periodicity: 300,
            vesting_start_timestamp: Timestamp::from_nanos(1635835145194509256u64),
            // vesting_start_timestamp: now,
            // initial_vesting_count: Uint128::from(0u128),
            // initial_vesting_consumed: Uint128::from(0u128),
            // vesting_periodicity: 300, // In seconds
            // vesting_count_per_period: Uint128::from(10u128),
            // total_vesting_token_count: Uint128::from(2000u128),
            // total_claimed_tokens_till_now: Uint128::from(0u128),
            // last_claimed_timestamp: now,
            // tokens_available_to_claim: Uint128::from(0u128),
            // last_vesting_timestamp: now,
            // cliff_period: 0, // in months
            // category_address: Some(category_address),
        };
    }

    #[test]
    fn test_vesting_at_tge() {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let spender_address = String::from("addr0001");
        let category_address = String::from("addr0002");

        let now = mock_env().block.time; // today
        let now =
            Timestamp::from_nanos(1635791182247090323u64).plus_seconds((30 * 24 * 60 * 60) + 650);
        let now = Timestamp::from_nanos(1635830827569351342u64).plus_seconds(850);
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
                assert_eq!(va.amount, Uint128::from(1234u128));
            }
            Err(e) => {
                println!("error = {:?}", e);
                assert_eq!(1, 0);
            }
        }
    }
    // #[test]
    // fn test_vesting_at_tge_with_initial_seed() {
    //     use std::time::{Duration, SystemTime, UNIX_EPOCH};
    //     let spender_address = String::from("addr0001");
    //     let category_address = String::from("addr0002");

    //     let now = mock_env().block.time; // today

    //     // vesting at TGE
    //     let mut vesting_details = get_vesting_details();
    //     vesting_details.initial_vesting_count = Uint128::from(1000u128);
    //     let vested_amount = calculate_tokens_for_this_period(
    //         Addr::unchecked(category_address.clone()),
    //         now,
    //         vesting_details,
    //     );
    //     match vested_amount {
    //         Ok(va) => {
    //             assert_eq!(va.amount, Uint128::from(1000u128));
    //         }
    //         Err(e) => {
    //             println!("error = {:?}", e);
    //             assert_eq!(1, 0);
    //         }
    //     }
    // }
    // #[test]
    // fn test_vesting_at_tge_no_initial_seed_first_interval() {
    //     use std::time::{Duration, SystemTime, UNIX_EPOCH};
    //     let spender_address = String::from("addr0001");
    //     let category_address = String::from("addr0002");

    //     let now = mock_env().block.time; // today

    //     // vesting at TGE
    //     let mut vesting_details = get_vesting_details();
    //     vesting_details.vesting_start_timestamp = vesting_details
    //         .vesting_start_timestamp
    //         .minus_seconds(vesting_details.vesting_periodicity);
    //     let vested_amount = calculate_tokens_for_this_period(
    //         Addr::unchecked(category_address.clone()),
    //         now,
    //         vesting_details,
    //     );
    //     match vested_amount {
    //         Ok(va) => {
    //             assert_eq!(va.amount, Uint128::from(10u128));
    //         }
    //         Err(e) => {
    //             println!("error = {:?}", e);
    //             assert_eq!(1, 0);
    //         }
    //     }
    // }

    // #[test]
    // fn test_vesting_at_tge_with_initial_seed_first_interval() {
    //     use std::time::{Duration, SystemTime, UNIX_EPOCH};
    //     let spender_address = String::from("addr0001");
    //     let category_address = String::from("addr0002");

    //     let now = mock_env().block.time; // today

    //     // vesting at TGE
    //     let mut vesting_details = get_vesting_details();
    //     vesting_details.initial_vesting_count = Uint128::from(1000u128);
    //     vesting_details.vesting_start_timestamp = vesting_details
    //         .vesting_start_timestamp
    //         .minus_seconds(vesting_details.vesting_periodicity);
    //     let vested_amount = calculate_tokens_for_this_period(
    //         Addr::unchecked(category_address.clone()),
    //         now,
    //         vesting_details,
    //     );
    //     match vested_amount {
    //         Ok(va) => {
    //             assert_eq!(va.amount, Uint128::from(1010u128));
    //         }
    //         Err(e) => {
    //             println!("error = {:?}", e);
    //             assert_eq!(1, 0);
    //         }
    //     }
    // }
    // #[test]
    // fn test_vesting_at_tge_no_initial_seed_2_uncalc_interval() {
    //     use std::time::{Duration, SystemTime, UNIX_EPOCH};
    //     let spender_address = String::from("addr0001");
    //     let category_address = String::from("addr0002");

    //     let now = mock_env().block.time; // today

    //     // vesting at TGE
    //     let mut vesting_details = get_vesting_details();
    //     vesting_details.vesting_start_timestamp = vesting_details
    //         .vesting_start_timestamp
    //         .minus_seconds(vesting_details.vesting_periodicity * 2);
    //     vesting_details.vesting_start_timestamp =
    //         vesting_details.vesting_start_timestamp.minus_seconds(5);
    //     let vested_amount = calculate_tokens_for_this_period(
    //         Addr::unchecked(category_address.clone()),
    //         now,
    //         vesting_details,
    //     );
    //     match vested_amount {
    //         Ok(va) => {
    //             assert_eq!(va.amount, Uint128::from(20u128));
    //         }
    //         Err(e) => {
    //             println!("error = {:?}", e);
    //             assert_eq!(1, 0);
    //         }
    //     }
    // }
    // #[test]
    // fn test_vesting_at_tge_with_initial_seed_2_uncalc_interval() {
    //     use std::time::{Duration, SystemTime, UNIX_EPOCH};
    //     let spender_address = String::from("addr0001");
    //     let category_address = String::from("addr0002");

    //     let now = mock_env().block.time; // today

    //     // vesting at TGE
    //     let mut vesting_details = get_vesting_details();
    //     vesting_details.initial_vesting_count = Uint128::from(1000u128);
    //     vesting_details.vesting_start_timestamp = vesting_details
    //         .vesting_start_timestamp
    //         .minus_seconds(vesting_details.vesting_periodicity * 2);
    //     vesting_details.vesting_start_timestamp =
    //         vesting_details.vesting_start_timestamp.minus_seconds(5);
    //     let vested_amount = calculate_tokens_for_this_period(
    //         Addr::unchecked(category_address.clone()),
    //         now,
    //         vesting_details,
    //     );
    //     match vested_amount {
    //         Ok(va) => {
    //             assert_eq!(va.amount, Uint128::from(1020u128));
    //         }
    //         Err(e) => {
    //             println!("error = {:?}", e);
    //             assert_eq!(1, 0);
    //         }
    //     }
    // }

    // #[test]
    // fn test_vesting_at_tge_no_initial_seed_1vested_1uncalc_interval() {
    //     use std::time::{Duration, SystemTime, UNIX_EPOCH};
    //     let spender_address = String::from("addr0001");
    //     let category_address = String::from("addr0002");

    //     let now = mock_env().block.time; // today

    //     let mut vesting_details = get_vesting_details();

    //     vesting_details.tokens_available_to_claim = Uint128::from(10u128);

    //     vesting_details.vesting_start_timestamp =
    //         now.minus_seconds(vesting_details.vesting_periodicity * 2);
    //     vesting_details.vesting_start_timestamp =
    //         vesting_details.vesting_start_timestamp.minus_seconds(5);

    //     vesting_details.last_vesting_timestamp =
    //         now.minus_seconds(vesting_details.vesting_periodicity);
    //     vesting_details.last_vesting_timestamp =
    //         vesting_details.last_vesting_timestamp.minus_seconds(5);

    //     let vested_amount = calculate_tokens_for_this_period(
    //         Addr::unchecked(category_address.clone()),
    //         now,
    //         vesting_details,
    //     );
    //     match vested_amount {
    //         Ok(va) => {
    //             assert_eq!(va.amount, Uint128::from(10u128));
    //         }
    //         Err(e) => {
    //             println!("error = {:?}", e);
    //             assert_eq!(1, 0);
    //         }
    //     }
    // }

    // #[test]
    // fn test_vesting_at_tge_no_initial_seed_1claimed_1uncalc_interval() {
    //     use std::time::{Duration, SystemTime, UNIX_EPOCH};
    //     let spender_address = String::from("addr0001");
    //     let category_address = String::from("addr0002");

    //     let now = mock_env().block.time; // today

    //     let mut vesting_details = get_vesting_details();

    //     vesting_details.total_claimed_tokens_till_now = Uint128::from(10u128);

    //     vesting_details.vesting_start_timestamp =
    //         now.minus_seconds(vesting_details.vesting_periodicity * 2);
    //     vesting_details.vesting_start_timestamp =
    //         vesting_details.vesting_start_timestamp.minus_seconds(5);

    //     vesting_details.last_vesting_timestamp =
    //         now.minus_seconds(vesting_details.vesting_periodicity);
    //     vesting_details.last_vesting_timestamp =
    //         vesting_details.last_vesting_timestamp.minus_seconds(5);

    //     let vested_amount = calculate_tokens_for_this_period(
    //         Addr::unchecked(category_address.clone()),
    //         now,
    //         vesting_details,
    //     );
    //     match vested_amount {
    //         Ok(va) => {
    //             assert_eq!(va.amount, Uint128::from(10u128));
    //         }
    //         Err(e) => {
    //             println!("error = {:?}", e);
    //             assert_eq!(1, 0);
    //         }
    //     }
    // }

    // #[test]
    // fn test_vesting_at_tge_no_initial_seed_1claimed_half_uncalc_interval() {
    //     use std::time::{Duration, SystemTime, UNIX_EPOCH};
    //     let spender_address = String::from("addr0001");
    //     let category_address = String::from("addr0002");

    //     let now = mock_env().block.time; // today

    //     let mut vesting_details = get_vesting_details();

    //     vesting_details.total_claimed_tokens_till_now = Uint128::from(10u128);

    //     vesting_details.vesting_start_timestamp =
    //         now.minus_seconds(vesting_details.vesting_periodicity * 2);
    //     vesting_details.vesting_start_timestamp =
    //         vesting_details.vesting_start_timestamp.plus_seconds(5);

    //     vesting_details.last_vesting_timestamp =
    //         now.minus_seconds(vesting_details.vesting_periodicity);
    //     vesting_details.last_vesting_timestamp =
    //         vesting_details.last_vesting_timestamp.plus_seconds(5);

    //     let vested_amount = calculate_tokens_for_this_period(
    //         Addr::unchecked(category_address.clone()),
    //         now,
    //         vesting_details,
    //     );
    //     match vested_amount {
    //         Ok(va) => {
    //             assert_eq!(va.amount, Uint128::from(10u128));
    //         }
    //         Err(e) => {
    //             println!("error = {:?}", e);
    //             assert_eq!(1, 0);
    //         }
    //     }
    // }

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

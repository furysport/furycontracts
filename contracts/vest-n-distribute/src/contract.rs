use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult, Uint128
};

use cw2::set_contract_version;

use cw20::{
    AllowanceResponse, BalanceResponse, Cw20Coin, Cw20ExecuteMsg, Cw20ReceiveMsg,
    DownloadLogoResponse, EmbeddedLogo, Expiration, Logo, LogoInfo, MarketingInfoResponse,
    MinterResponse, TokenInfoResponse,
};


use crate::error::ContractError;
use crate::msg::{InstantiateMsg, InstantiateVestingSchedulesInfo, QueryMsg};

use crate::state::{
    MinterData, TokenInfo, VestingDetails, ALLOWANCES, BALANCES, LOGO, MARKETING_INFO, TOKEN_INFO,
    VESTING_DETAILS,
};


const CONTRACT_NAME: &str = "crates.io:cw20-base";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const LOGO_SIZE_CAP: usize = 5 * 1024;

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
    _env: Env,
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

    instantiate_category_vesting_schedules(deps, env, msg.vesting)?;

    Ok(Response::default())
}

fn instantiate_category_vesting_schedules(
    deps: DepsMut,
    env: Env,
    vesting: Option<InstantiateVestingSchedulesInfo>,
) -> Result<Response, ContractError> {
    match vesting {
        Some(vesting_info) => {
            for schedule in vesting_info.vesting_schedules {
                let mut parent_cat_addr = None;
                if !schedule.parent_category_address.is_empty() {
                    parent_cat_addr = Some(schedule.parent_category_address);
                }
                let vesting_start_timestamp = env.block.time;
                let address = deps.api.addr_validate(schedule.address.as_str())?;
                let vesting_details = VestingDetails {
                    vesting_start_timestamp: vesting_start_timestamp,
                    initial_vesting_count: schedule.initial_vesting_count,
                    initial_vesting_consumed: Uint128::zero(),
                    vesting_periodicity: schedule.vesting_periodicity,
                    vesting_count_per_period: schedule.vesting_count_per_period,
                    total_vesting_token_count: schedule.total_vesting_token_count,
                    total_claimed_tokens_till_now: Uint128::zero(),
                    last_claimed_timestamp: None,
                    tokens_available_to_claim: Uint128::zero(),
                    last_vesting_timestamp: None,
                    cliff_period: schedule.cliff_period,
                    parent_category_address: parent_cat_addr,
                    should_transfer: schedule.should_transfer,
                };
                VESTING_DETAILS.save(deps.storage, &address, &vesting_details)?;
            }
            Ok(Response::default())
        }
        None => Ok(Response::default()),
    }
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
    msg: Cw20ExecuteMsg,
) -> Result<Response, ContractError> {
    Err(ContractError::Std(StdError::generic_err(String::from(
        "Not yet implemented",
    ))))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    to_binary(&some_query()?)
}

fn some_query() -> StdResult<String> {
    Err(StdError::not_found("Not yet implemented"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
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
        vesting: None,
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
        Cw20ExecuteMsg::UploadLogo(Logo::Embedded(EmbeddedLogo::Svg(img.into()))),
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


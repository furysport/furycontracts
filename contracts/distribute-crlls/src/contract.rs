#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Attribute, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo,
    OverflowError, OverflowOperation, QueryRequest, ReplyOn, Response, StdError,
    StdResult, SubMsg, Uint128, WasmMsg, WasmQuery,
};

use cw2::set_contract_version;
use cw20::{
    BalanceResponse, Cw20Coin, Cw20QueryMsg, Cw20ReceiveMsg, DownloadLogoResponse,
    EmbeddedLogo, Logo, MarketingInfoResponse, MinterResponse, TokenInfoResponse,
};

use crate::allowances::{execute_send_from, execute_transfer_from, query_allowance};
use crate::enumerable::{query_all_accounts, query_all_allowances};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{VestingDetails, BALANCES, LOGO, MARKETING_INFO, TOKEN_INFO, VESTING_DETAILS};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:distribute-crlls";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
extern crate serde;

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
    // Add the addresses which will be getting CRLL tokens as per vesting logic
    instantiate_vesting_details(&mut deps, _env)?;
    Ok(Response::default())
}

pub fn instantiate_vesting_details(deps: &mut DepsMut, env: Env) -> Result<Uint128, ContractError> {
    let purchased_till_date = Uint128::from(1050000000000u128);
    let now = env.block.time;
    // save details for angel invester wallet
    let gamified_vesting_details = VestingDetails {
        start_timestamp: now,
        vesting_periodicity: (5 * 60),
        periodic_vesting_token_count: Uint128::new(1234),
        total_vesting_token_count: Uint128::new(12345678910),
        total_purchased_tokens_till_now: purchased_till_date,
        cliff_period: 0,
    };
    let gamified_airdrop_address = deps
        .api
        .addr_validate("terra1hg3qhvhne3fmva7ty4fxrkwmk323vf9f6ueglx")?;
    VESTING_DETAILS.save(
        deps.storage,
        &gamified_airdrop_address,
        &gamified_vesting_details,
    )?;

    let mut total_supply = Uint128::zero();
    // let address = deps.api.addr_validate(&row.address)?;
    // VESTING_DETAILS.save(deps.storage, &address, &row.amount)?;
    // total_supply += row.amount;
    // for row in accounts {
    //     let address = deps.api.addr_validate(&row.address)?;
    //     VESTING_BALANCES.save(deps.storage, &address, &row.amount)?;
    //     total_supply += row.amount;
    // }
    Ok(total_supply)
}

pub fn distribute_to_accounts(
    deps: &mut DepsMut,
    env: &mut Env,
    info: &mut MessageInfo,
) -> Result<Response, ContractError> {
    // Contract address of cw20_base contract
    let cw20_base_contract_address = String::from("terra18vd8fpwxzck93qlwghaj6arh4p7c5n896xzem5");
    //Mint Wallet terra1rma8dw02n6luuqftuvfz8qwfl53fr8rv5cu43s
    let distribute_from = String::from("terra1qpq8u75z6cu23ryvdk4c7qmgh25ehzuzc7ecm4");

    // Fetch all tokens that can be distributed as per vesting logic
    let distribution_details = populate_distribution_details();
    // Calculate the total amount to be transferred
    let total_distribution_amount = calculate_total_distribution(&distribution_details);

    //Get the balance available in mint wallet
    let query = encode_smart_query(
        cw20_base_contract_address,
        Cw20QueryMsg::Balance {
            address: distribute_from.clone(),
        },
    )?;
    let balance_crlls: BalanceResponse = deps.querier.query(&query)?;
    let balance = balance_crlls.balance;

    //Check if there is sufficient balance with mint wallet
    // return error otherwise
    if balance < total_distribution_amount {
        return Err(ContractError::Std(StdError::overflow(OverflowError::new(
            OverflowOperation::Sub,
            balance,
            total_distribution_amount,
        ))));
    }

    //To collect attributes from all responses for transfer calls
    let mut attrs: Vec<String> = Vec::new();
    attrs.push(balance_crlls.balance.to_string());
    attrs.push(String::from("Kuchha to aaye!"));

    let mut attribs: Vec<Attribute> = Vec::new();

    for elem in distribution_details {
        //Proceed with the distribution
        let amount: Uint128 = elem.amount;
        let distribute_to = elem.address.clone();

        let res: Response<Empty> = Response::new()
            .add_submessage(SubMsg {
                id: 1234,
                msg: CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: String::from("terra1qpq8u75z6cu23ryvdk4c7qmgh25ehzuzc7ecm4"),
                    msg: to_binary(&cw20::Cw20ExecuteMsg::TransferFrom {
                        owner: distribute_from.clone(),
                        recipient: distribute_to,
                        amount: amount,
                    })?,
                    funds: vec![],
                }),
                gas_limit: None,
                reply_on: ReplyOn::Always,
            })
            .add_attributes(vec![
                ("action", "transfer vested tokens"),
                ("address", info.sender.as_str()),
            ]);

        for attrib in res.attributes {
            attribs.push(attrib);
        }
        attribs.push(Attribute::new("kuchha hua", "Pata nahi"));
    }
    Ok(Response::new().add_attributes(attribs))
}

fn encode_smart_query(addr: String, msg: Cw20QueryMsg) -> StdResult<QueryRequest<Empty>> {
    Ok(WasmQuery::Smart {
        contract_addr: addr.into(),
        msg: to_binary(&msg)?,
    }
    .into())
}

pub fn populate_distribution_details() -> Vec<Cw20Coin> {
    let mut distribution_details: Vec<Cw20Coin> = Vec::new();
    distribution_details.push(
        //Marketing wallet
        Cw20Coin {
            address: String::from("terra19snweulv8qgaw4wcs4sw09yms4929xccxvru75"),
            amount: Uint128::new(1000),
        },
    );
    distribution_details.push(
        //Gamified Airdrop wallet
        Cw20Coin {
            address: String::from("terra1hg3qhvhne3fmva7ty4fxrkwmk323vf9f6ueglx"),
            amount: Uint128::new(1000),
        },
    );
    distribution_details
}

pub fn calculate_total_distribution(distribution_details: &Vec<cw20::Cw20Coin>) -> Uint128 {
    let mut total = Uint128::zero();
    for elem in distribution_details {
        total += elem.amount;
    }
    return total;
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    mut deps: DepsMut,
    mut env: Env,
    mut info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Distribute {} => distribute_to_accounts(&mut deps, &mut env, &mut info),
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => execute_transfer_from(&mut deps, &mut env, &mut info, owner, recipient, amount),
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            msg,
        } => execute_send_from(&mut deps, &mut env, &mut info, owner, contract, amount, msg),
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
    }
}

pub fn query_balance(deps: Deps, address: String) -> StdResult<BalanceResponse> {
    let address = deps.api.addr_validate(&address)?;
    let balance = BALANCES
        .may_load(deps.storage, &address)?
        .unwrap_or_default();
    Ok(BalanceResponse { balance })
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

    fn get_balance<T: Into<String>>(deps: Deps, address: T) -> Uint128 {
        query_balance(deps, address.into()).unwrap().balance
    }
}

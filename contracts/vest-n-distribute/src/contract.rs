use cosmwasm_std::{
    entry_point, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult,
};

use crate::error::ContractError;
use crate::msg::{InstantiateMsg, MigrateMsg, QueryMsg, VnDExecuteMsg};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    Err(ContractError::Std(StdError::generic_err(String::from(
        "Not yet implemented",
    ))))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: VnDExecuteMsg,
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

use cosmwasm_std::{Deps, StdResult};

use cw20::{AllowanceResponse,};

use crate::state::ALLOWANCES;

pub fn query_allowance(deps: Deps, owner: String, spender: String) -> StdResult<AllowanceResponse> {
    let owner_addr = deps.api.addr_validate(&owner)?;
    let spender_addr = deps.api.addr_validate(&spender)?;
    let allowance = ALLOWANCES
        .may_load(deps.storage, (&owner_addr, &spender_addr))?
        .unwrap_or_default();
    Ok(allowance)
}

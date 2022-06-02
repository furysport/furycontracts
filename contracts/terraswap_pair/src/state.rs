use cosmwasm_std::Addr;
use cw_storage_plus::Item;
use terraswap::asset::PairInfoRaw;

pub const PAIR_INFO: Item<PairInfoRaw> = Item::new("pair_info");
pub const PROXY_ADDRESS: Item<Addr> = Item::new("proxy_address");

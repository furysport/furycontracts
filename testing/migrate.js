import {ClubStakingContractPath, mint_wallet,} from './constants.js';
import {storeCode} from "./utils.js";

let current_address = "terra1rws5tqe6fxl3hgmvywq76c6200rpqsy5tqvyuy"

export function sleep(time) {
    return new Promise((resolve) => setTimeout(resolve, time));
}

let fury_contract_address = ""
let proxy_contract_address = ""
let gaming_init = {
    "minting_contract_address": fury_contract_address,
    "admin_address": mint_wallet.key.accAddress,
    "platform_fee": "1",
    "transaction_fee": "1",
    "game_id": "Game001",
    "platform_fees_collector_wallet": mint_wallet.key.accAddress,
    "astro_proxy_address": proxy_contract_address,
}
let new_code_id = await storeCode(mint_wallet, ClubStakingContractPath);
console.log(new_code_id)
// await sleep(15000)
// let response = await instantiateContract(mint_wallet, new_code_id, gaming_init)
import {mint_wallet, MintingContractPath,} from './constants.js';
import {migrateContract, storeCode} from "./utils.js";

let current_address = "terra1rws5tqe6fxl3hgmvywq76c6200rpqsy5tqvyuy"

export function sleep(time) {
    return new Promise((resolve) => setTimeout(resolve, time));
}

let fury_contract_address = "terra1zjthyw8e8jayngkvg5kddccwa9v46s4w9sq2pq"

let new_code_id = await storeCode(mint_wallet, MintingContractPath);
console.log(new_code_id)
await sleep(15000)
await migrateContract(mint_wallet, fury_contract_address, new_code_id, {})
// let response =
// await instantiateContract(mint_wallet, new_code_id, gaming_init)
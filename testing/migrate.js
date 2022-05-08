import {GamingContractPath, terraClient} from './constants.js';
import {migrateContract} from "./utils.js";
import {MnemonicKey} from "@terra-money/terra.js";


const admin = new MnemonicKey({mnemonic: "rookie choose awake accident brisk day shoe fashion battle cost increase wrestle oyster drill mansion prevent top leader uncle again arctic carpet mention lend"});
export const admin_wallet = terraClient.wallet(admin);
console.log(GamingContractPath)
// let new_code_id = await storeCode(mint_wallet, GamingContractPath);
// console.log(new_code_id)
await migrateContract(admin_wallet, "terra1z0ufuvyzn6gcdrd8wq4sw7dvuul9pgzh3flslv", 69876, {})
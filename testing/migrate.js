import {ClubStakingContractPath, GamingContractPath, MintingContractPath, terraClient} from './constants.js';
import {migrateContract, storeCode} from "./utils.js";
import {MnemonicKey} from "@terra-money/terra.js";


const admin = new MnemonicKey({mnemonic: "rookie choose awake accident brisk day shoe fashion battle cost increase wrestle oyster drill mansion prevent top leader uncle again arctic carpet mention lend"});
export const admin_wallet = terraClient.wallet(admin);
console.log(GamingContractPath)
// let new_code_id = await storeCode(admin_wallet, GamingContractPath);
// console.log(new_code_id)
await migrateContract(admin_wallet,"terra1vcmultpz2e2jte9v0el6hae6gwwk427t2kkehs",69638,{})
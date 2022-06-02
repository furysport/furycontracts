import fs, {readFileSync, writeFileSync} from "fs";
import chalk from "chalk";
import path from 'path';
import {Wallet} from "./wallet";
import {cosmos} from "./constants";

export const ARTIFACTS_PATH = 'artifacts'

var gas_used = 0;


export function writeArtifact(data, name = 'artifact') {
    writeFileSync(path.join(ARTIFACTS_PATH, `${name}.json`), JSON.stringify(data, null, 2))
}


export function readArtifact(name = 'artifact') {
    try {
        const data = readFileSync(path.join(ARTIFACTS_PATH, `${name}.json`), 'utf8')
        return JSON.parse(data)
    } catch (e) {
        return {}
    }
}

/**
 * @notice Upload contract code to LocalTerra. Return code ID.
 */
export async function storeCode(deployerWallet, filepath) {
    deployerWallet.upload(filepath)
}

// export async function migrateContract(senderWallet, contractAddress, new_code_id, migrate_msg, verbose = false) {
//     let msg_list = [
//         new MsgMigrateContract(senderWallet.key.accAddress, contractAddress, new_code_id, migrate_msg),
//     ]
//     return await sendTransaction(senderWallet, msg_list, verbose);
// }

/**
 * @notice Execute a contract
 */
export async function executeContract(senderWallet, contractAddress, msg, coins, verbose = false) {
    return senderWallet.execute_contract(msg, contractAddress, coins)
}

/**
 * @notice Send a transaction. Return result if successful, throw error if failed.
 */


/**
 * @notice Instantiate a contract from an existing code ID. Return contract address.
 */
export async function uploadCodeId(deployer, path) {
    return deployer.upload(path)
}

export async function instantiateContract(deployer, codeId, instantiateMsg) {
    return deployer.init(codeId, instantiateMsg)
}

export async function queryContract(contractAddress, query) {
    cosmos.wasmQuery(
        contractAddress,
        JSON.stringify(query)
    ).then(json => {
        return json
    })
}

// TODO Need to fix a method in cosmos for it
export async function queryContractInfo(contractAddress) {
    const d = await terraClient.wasm.contractInfo(contractAddress);
    return d
}



// export async function get_server_epoch_seconds() {
//     const blockInfo = await cosmos.tendermint.blockInfo()
//     const time = blockInfo['block']['header']['time']
//
//     let dateObject = new Date(time);
//     return dateObject.getTime()
// }

export async function queryBankUusd(address) {
    let response = await terraClient.bank.balance(address)
    let value;
    try {
        value = Number(response[0]._coins.uusd.amount);
    } catch {
        value = 0;
    } finally {
        return value
    }
}


export async function queryTokenBalance(token_address, address) {
    let response = await queryContract(token_address, {
        balance: {address: address}
    });
    return Number(response.balance)
}

export async function transferToken(wallet_from, wallet_to_address, token_addres, token_amount) {
    let token_info = await queryContractInfo(token_addres)
    console.log(`Funding ${wallet_to_address} from ${wallet_from.key.accAddress} : ${token_amount} ${token_info.name}`);
    wallet_from.execute_contract({transfer: {recipient: wallet_to_address, amount: token_amount}},token_addres)
}

export async function bankTransferUusd(wallet_from, wallet_to_address, uusd_amount) {
    console.log(`Funding ${wallet_to_address} ${uusd_amount} uusd`);
    return wallet_from.send_funds(wallet_to_address, {"usdc": uusd_amount})
}

export async function bankTransferFund(wallet_from, wallet_to, uluna_amount, uusd_amount) {
    console.log(`Funding ${wallet_to.key.accAddress}`);
    let funds;
    if (uluna_amount == 0) {
        if (uusd_amount == 0) {
            return
        } else {
            funds = {uusd: uusd_amount}
        }
    } else {
        if (uusd_amount == 0) {
            funds = {uluna: uluna_amount}
        } else {
            funds = {uluna: uluna_amount, uusd: uusd_amount}
        }
    }


    return wallet_from.send_funds(wallet_to,funds)
}

export async function get_wallets(number_of_users) {
    let wallets_to_return = []
    for (let i = 0; i < number_of_users; i++) {
        wallets_to_return.push(cosmos.getRandomMnemonic())
    }
    return wallets_to_return
}
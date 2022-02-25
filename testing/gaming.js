import {
    mintInitMessage,
    MintingContractPath,
    VnDContractPath,
    walletTest1,
    walletTest2,
    walletTest3,
    gamified_airdrop_wallet,
    whitelist_airdrop_wallet,
    GamingContractPath,
    minting_wallet,
    treasury_wallet,
    marketing_wallet,
    terraClient,
    private_category_wallet,
    partnership_wallet, deployer
} from './constants.js';
import {
    storeCode,
    queryContract,
    executeContract,
    instantiateContract,
    sendTransaction,
    readArtifact,
    writeArtifact
} from "./utils.js";

import {primeAccountsWithFunds} from "./primeCustomAccounts.js";

import {promisify} from 'util';

import * as readline from 'node:readline';

import * as chai from 'chai';
import {Coin} from '@terra-money/terra.js';

const assert = chai.assert;

// Init and Vars
const sleep_time = 0
let gaming_contract_address = ""
const gaming_init = {
    "minting_contract_address": walletTest1.key.accAddress, //  This should be a contract But We passed wallet so it wont raise error on addr validate
    "admin_address": walletTest1.key.accAddress,
    "platform_fee": "300000",
    "game_id": "Game001",

}


// Helper Methods

function sleep(time) {
    return new Promise((resolve) => setTimeout(resolve, time));
}

const deploy_contract = async function (file, init) {
    const contractId = await storeCode(walletTest1, file,)
    await sleep(5000)
    const gamingInit = await instantiateContract(walletTest1, contractId, init)
    console.log(`New Contract Init Hash ${gamingInit.txhash}`)
    return gamingInit.logs[0].events[0].attributes[3].value; // Careful with order of argument
}


// Tests
let test_create_and_query_game = async function (time) {
    console.log("Testing Create and Query Game")
    gaming_contract_address = await deploy_contract(GamingContractPath, gaming_init)
    console.log(`Gaming Address:${gaming_contract_address}`)
    let query_resposne = await queryContract(gaming_contract_address, {
        game_details: {}
    })
    assert.isTrue(gaming_init['game_id'] === query_resposne['game_id'])
    assert.isTrue(1 === query_resposne['game_status'])
    console.log("Assert Success")
    sleep(time)
}

let test_create_and_query_pool = async function (time) {
    console.log("Testing Create and Query Pool")
    console.log("Create Pool")
    let response = await executeContract(walletTest1, gaming_contract_address, {
        create_pool: {
            "pool_type": "oneToOne"
        }
    })
    console.log(`Pool Create TX : ${response.txhash}`)
    let new_pool_id = response.logs[0].events[1].attributes[1].value
    console.log(`New Pool ID  ${new_pool_id}`)
    response = await queryContract(gaming_contract_address, {
        pool_details: {
            "pool_id": new_pool_id
        }
    })
    assert.isTrue(response['pool_id'] === new_pool_id)
    assert.isTrue(response['game_id'] === "Game001")
    assert.isTrue(response['pool_type'] === "oneToOne")
    assert.isTrue(response['current_teams_count'] === 0)
    assert.isTrue(response['rewards_distributed'] === false)
    console.log("Assert Success")
    sleep(time)
}


// let test_save_and_query_team_detail = async function (time) {
//     console.log("Testing Save and Query Team Details")
//     gaming_contract_address = await deploy_contract(GamingContractPath, gaming_init)
//     console.log(`Gaming Address:${gaming_contract_address}`)
//     let query_resposne = await queryContract(gaming_contract_address, {
//         game_details: {}
//     })
//     assert.isTrue(gaming_init['game_id'] === query_resposne['game_id'])
//     assert.isTrue(1 === query_resposne['game_status'])
//     console.log("Assert Success")
//     sleep(time)
// }

let test_get_team_count_for_user_in_pool_type = async function (time) {
    console.log("Test Get Team Count In Pool Type")
    executeContract(walletTest1, gaming_contract_address, {
        save_team_details: {
            'gamer': "Gamer001",
            'pool_id': "1",
            'team_id': "Team001",
            'game_id': "Game001",
            'pool_type': "oneToOne",
            'reward_amount': "144262",
            'claimed_reward': false,
            'refund_amount': "0",
            'claimed_refund': false,
            'team_points': 100,
            'team_rank': 2

        }
    })
    sleep(time)
    executeContract(walletTest1, gaming_contract_address, {
        save_team_details: {
            'gamer': "Gamer001",
            'pool_id': "1",
            'team_id': "Team002",
            'game_id': "Game001",
            'pool_type': "oneToOne",
            'reward_amount': "144262",
            'claimed_reward': false,
            'refund_amount': "0",
            'claimed_refund': false,
            'team_points': 100,
            'team_rank': 2

        }
    })
    sleep(time)
    executeContract(walletTest1, gaming_contract_address, {
        save_team_details: {
            'gamer': "Gamer001",
            'pool_id': "1",
            'team_id': "Team002",
            'game_id': "Game001",
            'pool_type': "oneToOne",
            'reward_amount': "144262",
            'claimed_reward': false,
            'refund_amount': "0",
            'claimed_refund': false,
            'team_points': 100,
            'team_rank': 2

        }
    })


    console.log("Assert Success")
    sleep(time)
}


await test_create_and_query_game(sleep_time)
await test_create_and_query_pool(sleep_time)
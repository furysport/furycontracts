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

import { primeAccountsWithFunds } from "./primeCustomAccounts.js";

import { promisify } from 'util';

import * as readline from 'node:readline';

import * as chai from 'chai';
import { Coin } from '@terra-money/terra.js';


const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
});
const question = promisify(rl.question).bind(rl);


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
    const gamingInit = await instantiateContract(walletTest1, contractId, init)
    console.log(`New Contract Init Hash ${gamingInit.txhash}`)
    return gamingInit.logs[0].events[0].attributes[3].value; // Careful with order of argument
}


function convertBinaryToObject(str) {
    var newBin = str.split(" ");
    var binCode = [];
    for (let i = 0; i < newBin.length; i++) {
        binCode.push(String.fromCharCode(parseInt(newBin[i], 2)));
    }
    let jsonString = binCode.join("");
    return JSON.parse(jsonString)
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
            "pool_type": "H2H"
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
    assert.isTrue(response['pool_type'] === "H2H")
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
    await executeContract(walletTest1, gaming_contract_address, {
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
    await executeContract(walletTest1, gaming_contract_address, {
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
    await executeContract(walletTest1, gaming_contract_address, {
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
    let team_count = await queryContract(gaming_contract_address, {
        get_team_count_for_user_in_pool_type: {
            "gamer": "Gamer001",
            "game_id": "Game001",
            "pool_type": "oneToOne"
        }
    })
    assert.isTrue(team_count === 3)
    console.log("Assert Success")

}

const set_pool_headers_for_H2H_pool_type = async function (time) {
    const response = await executeContract(walletTest1, gaming_contract_address, {
        set_pool_type_params: {
            'pool_type': "H2H",
            'pool_fee': "1000000",
            'min_teams_for_pool': 2,
            'max_teams_for_pool': 2,
            'max_teams_for_gamer': 2,
            'wallet_percentages': [
                {
                    "wallet_address": "terra1uyuy363rzun7e6txdjdemqj9zua9j7wxl2lp0m",
                    "wallet_name": "rake_1",
                    "percentage": 100,
                }
            ]
        }
    })
    console.log(response)
    console.log("Assert Success")
    if (time) sleep(time)
}

let test_game_pool_bid_submit_when_pool_team_in_range = async function (time) {
    console.log("Test game pool bid submit when pool team in range")
    set_pool_headers_for_H2H_pool_type();
    let msg = {
        game_pool_bid_submit_command: {
            'gamer': "terra1ttjw6nscdmkrx3zhxqx3md37phldgwhggm345k",
            'pool_type': "H2H",
            'pool_id': "1",
            'team_id': "1",
            poolFee: poolFee,
        }
    }

    let response = await executeContract(walletTest1, gaming_contract_address, {
        cw20_receive_msg: {
            sender: walletTest1.key.accAddress,
            amount: "10",
            msg: convertBinaryToObject(JSON.stringify(msg))
        }
    })
    console.log(response)
    console.log("Assert Success")
    sleep(time)
}

const test_game_lock_once_pool_is_closed = async function (time) {
    console.log("Testing game lock once pool is filled/closed.")

    let response = await executeContract(walletTest1, gaming_contract_address, {
        lock_game: {}
    })
    console.log(response)
    console.log("Assert Success")
    sleep(time)
}

const reward_distribution_for_locked_game = async function (time) {
    let response = await executeContract(walletTest1, gaming_contract_address, {
        "game_pool_reward_distribute": {
            "game_id": "Gamer001",
            "pool_id": "1",
            "game_winners":
                [
                    {
                        "gamer_address": "terra1ttjw6nscdmkrx3zhxqx3md37phldgwhggm345k",
                        "game_id": "Gamer001",
                        "team_id": "1",
                        "reward_amount": "2000000",
                        "refund_amount": "0",
                        "team_rank": 1,
                        "team_points": 150
                    },
                    {
                        "gamer_address": "terra1ttjw6nscdmkrx3zhxqx3md37phldgwhggm345k",
                        "game_id": "Gamer001",
                        "team_id": "2",
                        "reward_amount": "0",
                        "refund_amount": "0",
                        "team_rank": 2,
                        "team_points": 125
                    }
                ]
        }
    })
    console.log(response)
    console.log("Assert Success")
    sleep(time)
}
// let test_create_and_query_game = async function (time) {
//     console.log("Testing Create and Query Game")
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


await test_create_and_query_game(sleep_time)
await test_create_and_query_pool(sleep_time)
await test_get_team_count_for_user_in_pool_type(sleep_time)
await set_pool_headers_for_H2H_pool_type(sleep_time)
await test_game_pool_bid_submit_when_pool_team_in_range(sleep_time)
await test_game_lock_once_pool_is_closed(sleep_time)
await reward_distribution_for_locked_game(sleep_time)

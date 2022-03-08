import {
    GamingContractPath,
    mint_wallet,
    treasury_wallet,
    walletTest1,
} from './constants.js';
import {executeContract, instantiateContract, queryContract, storeCode, migrateContract} from "./utils.js";

import {promisify} from 'util';

import * as readline from 'node:readline';

import * as chai from 'chai';


const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
});
const question = promisify(rl.question).bind(rl);


const assert = chai.assert;
// Init and Vars
const sleep_time = 0
let gaming_contract_address = ""
let proxy_contract_address = "terra1ulwuw4etqty4n40f25css37jdtqu3z90rw0gxw"
let fury_contract_address = "terra12v7k5hmlqau8v69xurk4jdyaxz4y8s52ca8nyx"
const gamer = treasury_wallet
// const gamer_extra_1 = walletTest3.key.accAddress
// const gamer_extra_2 = walletTest4.key.accAddress

const gaming_init = {
    "minting_contract_address": fury_contract_address, //  This should be a contract But We passed wallet so it wont raise error on addr validate
    "admin_address": walletTest1.key.accAddress,
    "platform_fee": "1",
    "transaction_fee": "1",
    "game_id": "Game001",
    "platform_fees_collector_wallet": walletTest1.key.accAddress,
    "astro_proxy_address": proxy_contract_address,

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
            'gamer': gamer,
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
            'gamer': gamer,
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
            'gamer': gamer,
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
            "gamer": gamer,
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
            'pool_fee': "10000000",
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

async function transferFuryTokens(toAddress, amount) {
    let transferFuryToTreasuryMsg = {
        transfer: {
            recipient: toAddress.key.accAddress,
            amount: amount
        }
    };
    console.log(`transferFuryToTreasuryMsg = ${JSON.stringify(transferFuryToTreasuryMsg)}`);
    let response = await executeContract(mint_wallet, fury_contract_address, transferFuryToTreasuryMsg, {'uusd': 200000000});
    console.log(`transferFuryToTreasuryMsg Response - ${response['txhash']}`);
}


let test_game_pool_bid_submit_when_pool_team_in_range = async function (time) {
    console.log("Test game pool bid submit when pool team in range")
    set_pool_headers_for_H2H_pool_type();

    // Add method to provide the wallet one fury token
    console.log("Sending fury Tokens from Minter to wallet 1")
    let response = await transferFuryTokens(walletTest1, "5000000000")
    console.log(response)
    console.log("Getting Funds To Send In Fury")
    let funds_to_send_in_fury = await queryContract(proxy_contract_address,
        {
            get_fury_equivalent_to_ust: {
                "ust_count": "10000000"
            }
        });

    console.log(`Fees Received:${funds_to_send_in_fury}`)
    let increaseAllowanceMsg = {
        increase_allowance: {
            spender: gaming_contract_address,
            amount: `${funds_to_send_in_fury * 2}`
        }
    };
    console.log("Increasing Allowance For the Gaming Pool Contract ")
    let incrAllowResp = await executeContract(walletTest1, fury_contract_address, increaseAllowanceMsg);
    console.log(incrAllowResp)
    console.log("Submitting Game Pool Bid")
    response = await executeContract(walletTest1, gaming_contract_address, {
        game_pool_bid_submit_command: {
            gamer: gamer,
            pool_type: "H2H",
            pool_id: "1",
            team_id: "Team001",
            amount: `${funds_to_send_in_fury}`
        }
    }, {'uusd': 100000000})


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
const test_game_lock_once_pool_is_canceled = async function (time) {
    console.log("Testing game lock once pool is Cancelled.")

    let response = await executeContract(walletTest1, gaming_contract_address, {
        cancel_game: {}
    })
    console.log(response)
    console.log("Assert Success")
    sleep(time)
}
//     ExecuteMsg::ClaimReward { gamer } => claim_reward(deps, info, gamer, env),
//     ExecuteMsg::ClaimRefund { gamer } => claim_refund(deps, info, gamer, env),
const claim = async function (time) {
    let expected_reward = await queryContract(gaming_contract_address, {
            query_reward: {"gamer": gamer}
        }
    )
    console.log(`Expected Reward Amount  ${expected_reward}`)
    let response = await executeContract(walletTest1, gaming_contract_address, {
        claim_reward: {"gamer": walletTest1.key.accAddress}
    })
    console.log(response)
    console.log("Assert Success")
    sleep(time)
}
const reward_distribution_for_locked_game = async function (time) {
    console.log("Reward Distribution for locked game")
    let response = await executeContract(walletTest1, gaming_contract_address, {
        "game_pool_reward_distribute": {
            "game_id": "Gamer001",
            "pool_id": "1",
            "game_winners":
                [
                    {
                        "gamer_address": gamer,
                        "game_id": "Gamer001",
                        "team_id": "1",
                        "reward_amount": "5000000", // This will be in ufury
                        "refund_amount": "0",
                        "team_rank": 1,
                        "team_points": 150
                    },
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

async function test_migrate(time) {
    console.log("Testing Migrate")
    console.log("Uploading New Contract")
    let contract_id = await storeCode(walletTest1, GamingContractPath,) //  Uploading the new contract
    console.log("Executing the migrate")
    let r = await migrateContract(walletTest1, gaming_contract_address, contract_id, {})
    console.log(r)
    console.log("Success")
    sleep(time)


}

async function test_game_pool_reward_distribute(time) {
    console.log("Game Pool Reward Distribute")
    let game_winners = [
        {
            gamer_address: gamer,
            game_id: "Game001",
            team_id: "Team001",
            team_rank: 1,
            team_points: 100,
            reward_amount: 100,
            refund_amount: ""
        }, {
            gamer_address: walletTest1.key.accAddress,
            game_id: "Game001",
            team_id: "Team001",
            team_rank: 2,
            team_points: 200,
            reward_amount: 100,
            refund_amount: ""
        },
        {
            gamer_address: walletTest1.key.accAddress,
            game_id: "Game001",
            team_id: "Team001",
            team_rank: 2,
            team_points: 300,
            reward_amount: 100,
            refund_amount: ""
        }
    ]
    console.log("Executing Reward Distribute")

    let response = await executeContract(walletTest1, gaming_contract_address, {
        game_pool_reward_distribute: {
            pool_id: "1",
            game_winners: game_winners
        }
    })
    console.log(response)
}

await test_create_and_query_game(sleep_time)
await test_create_and_query_pool(sleep_time)
await test_get_team_count_for_user_in_pool_type(sleep_time)
await set_pool_headers_for_H2H_pool_type(sleep_time)
await test_game_pool_bid_submit_when_pool_team_in_range(sleep_time)
await test_game_lock_once_pool_is_closed(sleep_time)
await reward_distribution_for_locked_game(sleep_time)
await claim(sleep_time)
// // Claim
// await test_migrate(sleep_time)
// await test_create_and_query_game(sleep_time)
// await test_create_and_query_pool(sleep_time)
// await test_get_team_count_for_user_in_pool_type(sleep_time)
// await set_pool_headers_for_H2H_pool_type(sleep_time)
// await test_game_pool_bid_submit_when_pool_team_in_range(sleep_time)
// await test_game_lock_once_pool_is_closed(sleep_time)
// await test_game_lock_once_pool_is_canceled(sleep_time)
// // Refund
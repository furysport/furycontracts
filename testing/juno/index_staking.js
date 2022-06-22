import dotenv from "dotenv";
dotenv.config();
import * as readline from 'node:readline';
import { promisify } from 'util';
// import { ajay_wallet, ClubStakingContractPath, liquidity_wallet, marketing_wallet, MintingContractPath, mintInitMessage, mint_wallet, nitin_wallet, sameer_wallet, team_wallet, terraClient, treasury_wallet } from './constants.js';
import { ajay_wallet, ClubStakingContractPath, liquidity_wallet, marketing_wallet, MintingContractPath, mintInitMessage, mint_wallet, nitin_wallet, sameer_wallet, team_wallet, terraClient, treasury_wallet } from './constants.js';

import { primeAccountsWithFunds } from "./primeCustomAccounts.js";
import { executeContract, getGasUsed, instantiateContract, queryContract, readArtifact, storeCode, 
    writeArtifact, queryBankUusd, queryContractInfo, readDistantArtifact,
    queryTokenBalance,
    queryBankUusdContract,
    queryBankUusdNew
 } from './utils.js';

const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
});
const question = promisify(rl.question).bind(rl);

async function main() {
    try {
        // terraClient.chainID = "bombay-12";
        terraClient.chainID = "uni-3";
        console.log('terraClient.chainID : '+terraClient.chainID);
        let deploymentDetails = readArtifact(terraClient.chainID);
        // let skipSetup = await question('Do you jump to Repeatable Operations? (y/N) ');
        // if (skipSetup === 'Y' || skipSetup === 'y') {
        //     await cycleOperationsOnClubStaking(deploymentDetails);
        // } else {
            // const primeAccounts = await question('Do you want to preload custom accounts? (y/N) ');
            // if (primeAccounts === 'Y' || primeAccounts === 'y') {
            //     let txHash = await primeAccountsWithFunds();
            //     console.log(txHash);
            // }
            // const setupAccounts = await question('Do you want to setup staking contracts first? (y/N) ');
            // if (setupAccounts === 'Y' || setupAccounts === 'y') {
            await proceedToSetup(deploymentDetails);
            // }
            deploymentDetails = readArtifact(terraClient.chainID);

            await performOperationsOnClubStaking(deploymentDetails);
        // }

    } catch (error) {
        // console.log(error.response.data.message)
        // console.log(error.response.config.data)
        console.log(error);
    } finally {
        rl.close();
        console.log(`Total gas used = ${getGasUsed()}`);
    }
}

async function proceedToSetup(deploymentDetails) {
    const startFresh = await question('Do you want to upload and deploy fresh? (y/N)');
    if (startFresh === 'Y' || startFresh === 'y') {
        deploymentDetails = readArtifact(terraClient.chainID);
    }
    if (!deploymentDetails.adminWallet) {
        deploymentDetails.adminWallet = mint_wallet.wallet_address;
    }
    if (!deploymentDetails.teamWallet) {
        deploymentDetails.teamWallet = team_wallet.wallet_address;
    }
    if (!deploymentDetails.nitinWallet) {
        deploymentDetails.nitinWallet = nitin_wallet.wallet_address;
    }
    if (!deploymentDetails.ajayWallet) {
        deploymentDetails.ajayWallet = ajay_wallet.wallet_address;
    }
    if (!deploymentDetails.sameerWallet) {
        deploymentDetails.sameerWallet = sameer_wallet.wallet_address;
    }
    writeArtifact(deploymentDetails, terraClient.chainID);
    
    let result = await setAstroProxyContractAddress(deploymentDetails);

    if (result) {
        deploymentDetails = readArtifact(terraClient.chainID);
        await transferFuryToWallets(deploymentDetails);
        await uploadClubStaking(deploymentDetails);
        await instantiateClubStaking(deploymentDetails);
    }
    console.log("Leaving proceedToSetup");
}

async function setAstroProxyContractAddress(deploymentDetails) {
    if (!deploymentDetails.astroProxyContractAddress) {
        let astroProxyAddress = "";
        let distantDeploymentDetails = readDistantArtifact('.',terraClient.chainID); // Path : "../../astroport-core/testing/artifacts/juno.json"
        if (!distantDeploymentDetails.proxyContractAddress) {       
            astroProxyAddress = await question('Proxy not found, Please provide the astro proxy contract address? ');
            deploymentDetails.astroProxyContractAddress = astroProxyAddress;
        } else {
            deploymentDetails.astroProxyContractAddress = distantDeploymentDetails.proxyContractAddress;
            console.log(`Proxy found at : ${deploymentDetails.astroProxyContractAddress}`);
            astroProxyAddress = deploymentDetails.astroProxyContractAddress;
        }
//        const proxyInfo = await queryContractInfo(mint_wallet, astroProxyAddress); // TODO ?
//        console.log("proxy address details : " + proxyInfo)
//        deploymentDetails.furyContractAddress = proxyInfo.init_msg.custom_token_address;
//        const mintInfo = await queryContractInfo(deploymentDetails.furyContractAddress);
        // deploymentDetails.furyTokenCodeId = mintInfo.code_id;

        deploymentDetails.furyTokenCodeId = deploymentDetails.furyTokenCodeId;
        writeArtifact(deploymentDetails, terraClient.chainID);
        return true;
    }
}

async function uploadFuryTokenContract(deploymentDetails) {
    if (!deploymentDetails.furyTokenCodeId) {
        let deployFury = false;
        const answer = await question('Do you want to upload Fury Token Contract? (y/N) ');
        if (answer === 'Y' || answer === 'y') {
            deployFury = true;
        } else if (answer === 'N' || answer === 'n') {
            const codeId = await question('Please provide code id for Fury Token contract: ');
            if (isNaN(codeId)) {
                deployFury = true;
            } else {
                deploymentDetails.furyTokenCodeId = codeId;
                writeArtifact(deploymentDetails, terraClient.chainID);
                deployFury = false;
            }
        } else {
            console.log("Alright! Have fun!! :-)");
            return false;
        }
        if (deployFury) {
            console.log("Uploading Fury token contract");
            console.log(`mint_wallet = ${mint_wallet.wallet_address}`);
            let contractId = await storeCode(mint_wallet, MintingContractPath); // Getting the contract id from local terra
            console.log(`Fury Token Contract ID: ${contractId}`);
            deploymentDetails.furyTokenCodeId = contractId;
            writeArtifact(deploymentDetails, terraClient.chainID);
        }
    }
    return true;
}

async function transferFuryToWallets(deploymentDetails) {
    await transferFury(deploymentDetails,mint_wallet,treasury_wallet,"5000000000");
    await transferFury(deploymentDetails,mint_wallet,marketing_wallet,"50000000");
    await transferFury(deploymentDetails,mint_wallet,liquidity_wallet,"50000000");
    await transferFury(deploymentDetails,mint_wallet,nitin_wallet,"50000000");
    await transferFury(deploymentDetails,mint_wallet,ajay_wallet,"50000000");
    await transferFury(deploymentDetails,mint_wallet,sameer_wallet,"50000000");
}

async function transferFury(deploymentDetails,fromWallet,toWallet,ufury) {
    let transferFuryMsg = {
        transfer: {
            recipient: toWallet.wallet_address,
            amount: ufury
        }
    };
    console.log(`transferFuryMsg = ${JSON.stringify(transferFuryMsg)}`);
    let response = await executeContract(fromWallet, deploymentDetails.furyContractAddress, transferFuryMsg);
    console.log(`transferFury Response - ${response['transactionHash']}`);
}


async function uploadClubStaking(deploymentDetails) {
    if (!deploymentDetails.clubStakingId) {
        console.log("Uploading Club Staking...");
        let contractId = await storeCode(mint_wallet, ClubStakingContractPath); // Getting the contract id from local terra
        console.log(`Club Staking Contract ID: ${contractId}`);
        deploymentDetails.clubStakingId = contractId.codeId;
       // writeArtifact(deploymentDetails, terraClient.chainID);
    }
}

async function instantiateClubStaking(deploymentDetails) {
    if (!deploymentDetails.clubStakingAddress) {
        console.log("Instantiating Club Staking...");
        /*
        Club Price in this contract is 100000 (0.1 Fury) -  "club_price": "100000"
        Withdraw from a Club will mature after 2 minutes 120 seconds -  "bonding_duration": 120
        Also a repeat calculate_and_distribute_reward() 
            if called within 5 minutes shall fail - "reward_periodicity": 300
        */
        let clubStakingInitMessage = {
            admin_address: deploymentDetails.adminWallet,
            minting_contract_address: deploymentDetails.furyContractAddress,
            astro_proxy_address: deploymentDetails.astroProxyContractAddress,
            platform_fees_collector_wallet: deploymentDetails.adminWallet,
            club_fee_collector_wallet: deploymentDetails.teamWallet,
            club_reward_next_timestamp: "1640447808000000000",
            reward_periodicity: 300, 
            club_price: "100000",
            bonding_duration: 120,
            owner_release_locking_duration: 120,
            platform_fees: "100",
            transaction_fees: "30",
            control_fees: "50",
            max_bonding_limit_per_user: 100,
            usdc_ibc_symbol: "ujunox" //changed
        }
        console.log(JSON.stringify(clubStakingInitMessage, null, 2));
        let result = await instantiateContract(mint_wallet, deploymentDetails.clubStakingId, clubStakingInitMessage); // changed
       // let contractAddresses = result.logs[0].events[0].attributes.filter(element => element.key == 'contract_address').map(x => x.value);
       // deploymentDetails.clubStakingAddress = contractAddresses.shift();
        deploymentDetails.clubStakingAddress = result;
        console.log(`Club Staking Contract Address: ${deploymentDetails.clubStakingAddress}`);
        writeArtifact(deploymentDetails, terraClient.chainID);
    }
}

async function performOperationsOnClubStaking(deploymentDetails) {
    await showAllClubOwnerships(deploymentDetails);
    await showAllClubStakes(deploymentDetails);
    console.log("Balances of buyer before buy club");
    // await queryBalancesTemp(deploymentDetails, nitin_wallet, true);
    await queryBalances(deploymentDetails, nitin_wallet, true);
    console.log("Balances of club_fee_collector before buy club");
    await queryBalances(deploymentDetails, team_wallet, true);

    console.log("Balances of platform_fee_collector before buy club");  
    await queryBalancesForAddress(deploymentDetails, deploymentDetails.adminWallet, true);
    
    console.log("Buy club activity");
    await buyAClub(deploymentDetails);
    console.log("Balances of buyer after buy club");
    await queryBalances(deploymentDetails, nitin_wallet, true);
    console.log("Balances of club_fee_collector after buy club");
    await queryBalances(deploymentDetails, team_wallet, true);

    console.log("Balances of platform_fee_collector after buy club");
    await queryBalancesForAddress(deploymentDetails, deploymentDetails.adminWallet, true);
   
    await showAllClubOwnerships(deploymentDetails);

    console.log("Assign club activity");
    await assignAClub(deploymentDetails);
    console.log("Balances of owner after assign club");
    await queryBalances(deploymentDetails, nitin_wallet, true);

    await showAllClubOwnerships(deploymentDetails);
	await showAllClubStakes(deploymentDetails);
    
    console.log("Balances of admin before assign club stake");
    await queryBalancesForAddress(deploymentDetails, deploymentDetails.adminWallet, true);

    console.log("Assign Stake activity");
    await assignStakesToAClub(deploymentDetails);

    console.log("Balances of admin after assign club stake");
    await queryBalancesForAddress(deploymentDetails, deploymentDetails.adminWallet, true);

	await showAllClubStakes(deploymentDetails);

    console.log("Balances of staker before club stake");
    await queryBalances(deploymentDetails, sameer_wallet, true);

    console.log("Stake on a club activity");
    await stakeOnAClub(deploymentDetails);
    console.log("Balances of staker after club stake");
    await queryBalances(deploymentDetails, sameer_wallet, true);
    
    console.log("Balances of platform_fee_collector after club stake");
    await queryBalancesForAddress(deploymentDetails, deploymentDetails.adminWallet, true);
    
    console.log("Balances of contract after club stake");
    await queryBalancesForAddress(deploymentDetails, deploymentDetails.clubStakingAddress, true);

    console.log("Balances of contract after club stake");
	await showAllClubStakes(deploymentDetails);
    
    console.log("Reward activity");
	await distributeRewards(deploymentDetails);
	await showAllClubStakes(deploymentDetails);
	await showAllClubOwnerships(deploymentDetails);
	
    await claimRewards(deploymentDetails);
    console.log("Balances of staker after claim reward");
    await queryBalances(deploymentDetails, sameer_wallet, true);
    console.log("Withdraw Stake activity");
    
    await withdrawStakeFromAClub(deploymentDetails);
    console.log("Balances of staker after withdraw stake");
    await queryBalances(deploymentDetails, sameer_wallet, true);

    console.log("Balances of platform_fee_collector after withdraw stake");
    await queryBalancesForAddress(deploymentDetails, deploymentDetails.adminWallet, true);

    console.log("Balances of contract after withdraw stake");
    await queryBalancesForAddress(deploymentDetails, deploymentDetails.clubStakingAddress, true);
}


async function cycleOperationsOnClubStaking(deploymentDetails) {
    // await queryClubStakes(deploymentDetails,"ABC");
    await queryStepByStepClubStakes(deploymentDetails);
    // await showAllClubStakes(deploymentDetails);
    // await queryStakerStakes(deploymentDetails,sameer_wallet.key.accAddress);
    // await queryStakerStakes(deploymentDetails,nitin_wallet.key.accAddress);
    // let stakeResp = await queryStakerStakes(deploymentDetails,ajay_wallet.key.accAddress);
    // console.log(JSON.stringify(stakeResp));`
    // console.log("Balances of buyer before buy club");
    // await queryBalances(deploymentDetails, deploymentDetails.nitinWallet, true);
    // console.log("Balances of club_fee_collector before buy club");
    // await queryBalances(deploymentDetails, deploymentDetails.teamWallet, true);
    // console.log("Balances of platform_fee_collector before buy club");
    // await queryBalances(deploymentDetails, deploymentDetails.adminWallet, true);
    // await buyAClub(deploymentDetails);
    // console.log("Balances of buyer after buy club");
    // await queryBalances(deploymentDetails, deploymentDetails.nitinWallet, true);
    // console.log("Balances of club_fee_collector after buy club");
    // await queryBalances(deploymentDetails, deploymentDetails.teamWallet, true);
    // console.log("Balances of platform_fee_collector after buy club");
    // await queryBalances(deploymentDetails, deploymentDetails.adminWallet, true);
    // await showAllClubOwnerships(deploymentDetails);

    // await assignAClub(deploymentDetails);
    // console.log("Balances of owner after assign club");
    // await queryBalances(deploymentDetails, deploymentDetails.nitinWallet, true);
    // await showAllClubOwnerships(deploymentDetails);
    // await showAllClubStakes(deploymentDetails);
    // console.log("Balances of admin before assign club stake");
    // await queryBalances(deploymentDetails, deploymentDetails.adminWallet, true);
    // await assignStakesToAClub(deploymentDetails);
    // console.log("Balances of admin after assign club stake");
    // await queryBalances(deploymentDetails, deploymentDetails.adminWallet, true);
    // await showAllClubStakes(deploymentDetails);

    // console.log("Balances of staker before club stake");
    // await queryBalances(deploymentDetails, deploymentDetails.sameerWallet, true);
    // await stakeOnAClub(deploymentDetails);
    // console.log("Balances of staker after club stake");
    // await queryBalances(deploymentDetails, deploymentDetails.sameerWallet, true);
    // console.log("Balances of platform_fee_collector after club stake");
    // await queryBalances(deploymentDetails, deploymentDetails.adminWallet, true);
    // console.log("Balances of contract after club stake");
    // await queryBalances(deploymentDetails, deploymentDetails.clubStakingAddress, true);
    // console.log("Balances of contract after club stake");
    // await showAllClubStakes(deploymentDetails);
    // await distributeRewards(deploymentDetails);
    // await showAllClubStakes(deploymentDetails);
    // // await showAllClubOwnerships(deploymentDetails);
    // // await claimRewards(deploymentDetails);
    // console.log("Balances of staker after claim reward");
    // await queryBalances(deploymentDetails, deploymentDetails.sameerWallet, true);
    // await withdrawStakeFromAClub(deploymentDetails);
    // console.log("Balances of staker after withdraw stake");
    // await queryBalances(deploymentDetails, deploymentDetails.sameerWallet, true);
    // console.log("Balances of platform_fee_collector after withdraw stake");
    // await queryBalances(deploymentDetails, deploymentDetails.adminWallet, true);
    // console.log("Balances of contract after withdraw stake");
    // await queryBalances(deploymentDetails, deploymentDetails.clubStakingAddress, true);
}

async function showAllClubOwnerships(deploymentDetails) {
    //Fetch club ownership details for all clubs
    let coResponse = await queryContract(mint_wallet, deploymentDetails.clubStakingAddress, {
        all_club_ownership_details: {}
    });
    let club_string = "All clubs ownership \n";
    for (let i = 0 ; i < coResponse.length ; i++) {
        club_string += coResponse[i].owner_address + " " + coResponse[i].club_name 
            + " : " + coResponse[i].owner_released.toString();
        if (coResponse[i].owner_released) {
            club_string += " " + coResponse[i].locking_period +  " " + coResponse[i].start_timestamp + "\n";
        } else {
            club_string += "\n";
        }
    }
    console.log(club_string);
}

async function showAllClubStakes(deploymentDetails) {
    let coResponse = await queryContract(mint_wallet, deploymentDetails.clubStakingAddress, {
        all_stakes: {"user_address_list":[]}
    });
    // console.log(JSON.stringify(coResponse));
    let stake_string = "All stakes \n";
    for (let i = 0 ; i < coResponse.length ; i++) {
        stake_string += coResponse[i].staker_address + " " + coResponse[i].club_name 
            + " " + coResponse[i].staked_amount  
            + " " + coResponse[i].auto_stake.toString();
        if (coResponse[i].auto_stake == false) {
            stake_string += " " + coResponse[i].reward_amount + "\n";
        } else {
            stake_string += "\n";
        }
    }
    console.log(stake_string);
}

async function queryStakerStakes(deploymentDetails,staker) {
    //Fetch club Stakes details for all clubs
    let coResponse = await queryContract(mint_wallet,deploymentDetails.clubStakingAddress, {
        all_stakes_for_user: {
            user_address: staker
        }
    });
    let stake_string = "Stakes for " + staker + "\n";
    for (let i = 0 ; i < coResponse.length ; i++) {
        stake_string += coResponse[i].club_name 
            + " " + coResponse[i].staked_amount  
            + " " + coResponse[i].auto_stake.toString();
        if (coResponse[i].auto_stake == false) {
            stake_string += " " + coResponse[i].reward_amount + "\n";
        } else {
            stake_string += "\n";
        }
    }
    console.log(stake_string);
    return coResponse;
}

async function queryClubStakes(deploymentDetails,club_name) {
    //Fetch club Stakes details for all clubs
    let coResponse = await queryContract(mint_wallet,deploymentDetails.clubStakingAddress, {
        club_staking_details: {
            club_name: club_name
        }
    });
    let stake_string = "Stakes for " + club_name + "\n";
    for (let i = 0 ; i < coResponse.length; i++) {  
        stake_string += coResponse[i].club_name 
            + " " + coResponse[i].staker_address 
            + " " + coResponse[i].staked_amount  
            + " " + coResponse[i].auto_stake.toString();
        if (coResponse[i].auto_stake == false) {
            stake_string += " " + coResponse[i].reward_amount + "\n";
        } else {
            stake_string += "\n";
        }
    }
    console.log(stake_string);
    // return coResponse;
}

async function queryStepByStepClubStakes(deploymentDetails) {
    let clubResponse = await queryContract(mint_wallet, deploymentDetails.clubStakingAddress, {
        all_club_ownership_details: {}
    });
    console.log("Total Clubs : " + clubResponse.length.toString());
    for (let i = 0 ; i < clubResponse.length ; i++) {
        try {
            let coResponse = await queryContract(mint_wallet, deploymentDetails.clubStakingAddress, {
                club_staking_details: {
                    club_name: clubResponse[i].club_name
                }
            });
            //console.log("Stakes for " + clubResponse[i].club_name + " count " + coResponse.length.toString() + "\n");
            let stake_string = "";
            for (let i = 0 ; i < coResponse.length; i++) {  // coResponse.length 
                stake_string += coResponse[i].staker_address 
                    + " " + coResponse[i].staked_amount  
                    + " " + coResponse[i].auto_stake.toString() + " " + coResponse[i].club_name.split(" ").join("_");
                if (coResponse[i].auto_stake == false) {
                    stake_string += " " + coResponse[i].reward_amount + "\n";
                } else {
                    stake_string += "\n";
                }
            }
            console.log(stake_string);
        } catch (error) {
            console.log(clubResponse[i].club_name + " Error");
        } finally {
            continue;
        }
    }
}


async function buyAClub(deploymentDetails) {
    if (!deploymentDetails.clubBought) {
        //let Nitin buy a club
        // first increase allowance for club staking contract on nitin wallet to let it move fury
        let increaseAllowanceMsg = {
            increase_allowance: {
                spender: deploymentDetails.clubStakingAddress,
                amount: "500000"
            }
        };
        let incrAllowResp = await executeContract(nitin_wallet, deploymentDetails.furyContractAddress, increaseAllowanceMsg);
        console.log(`Increase allowance response : ${JSON.stringify(incrAllowResp)}`)
        console.log(`Increase allowance response hash = ${incrAllowResp['transactionHash']}`);

        let bacRequest = {
            buy_a_club: {
                buyer: nitin_wallet.wallet_address,
                club_name: "ClubB",
                auto_stake: true
            }
        };
       // let platformFees = await queryContract(nitin_wallet, deploymentDetails.clubStakingAddress, { query_platform_fees: { msg: Buffer.from(JSON.stringify(bacRequest)).toString('base64') } });
       // console.log(`platformFees = ${JSON.stringify(platformFees)}`);
      //  let bacResponse = await executeContract(nitin_wallet, deploymentDetails.clubStakingAddress, bacRequest, { 'uusd': Number(platformFees) });
        console.log("nitin .. " + JSON.stringify(nitin_wallet))
        let bacResponse = await executeContract(nitin_wallet, deploymentDetails.clubStakingAddress, bacRequest, {"denom":"ujunox","amount":"130"});
        console.log(`bacResponse : ${JSON.stringify(bacResponse)}`)
        console.log("Buy a club transaction hash = " + bacResponse['transactionHash']);
        deploymentDetails.clubBought = true;
		writeArtifact(deploymentDetails, terraClient.chainID);
    }
}

async function assignAClub(deploymentDetails) {
	//let Admin assign a club to Sameer
	let aacRequest = {
		assign_a_club: {
			buyer: sameer_wallet.wallet_address,
			club_name: "ClubD",
			auto_stake: true
		}
	};
	let aacResponse = await executeContract(mint_wallet, deploymentDetails.clubStakingAddress, aacRequest);
	console.log("Assign a club transaction hash = " + aacResponse['transactionHash']);
}

async function stakeOnAClub(deploymentDetails) {
    //let Sameer stakeOn a club
    // first increase allowance for club staking contract on Sameer wallet to let it move fury
    let increaseAllowanceMsg = {
        increase_allowance: {
            spender: deploymentDetails.clubStakingAddress,
            amount: "100000"
        }
    };
    let incrAllowResp = await executeContract(sameer_wallet, deploymentDetails.furyContractAddress, increaseAllowanceMsg);
    console.log(`Increase allowance response : ${JSON.stringify(incrAllowResp)}`)
    console.log(`Increase allowance response hash = ${incrAllowResp['transactionHash']}`);

    let soacRequest = {
        stake_on_a_club: {
            staker: sameer_wallet.wallet_address,
            club_name: "ClubB",
            amount: "100000",
            auto_stake: false
        }
    };
//    let platformFees = await queryContract(mint_wallet, deploymentDetails.clubStakingAddress, { query_platform_fees: { msg: Buffer.from(JSON.stringify(soacRequest)).toString('base64') } });
//    console.log(`platformFees = ${JSON.stringify(platformFees)}`);
    // let soacResponse = await executeContract(sameer_wallet, deploymentDetails.clubStakingAddress, soacRequest, { 'uusd': Number(platformFees) });
    let soacResponse = await executeContract(sameer_wallet, deploymentDetails.clubStakingAddress, soacRequest,  {"denom":"ujunox","amount":"180"});
    console.log(`Increase allowance response : ${JSON.stringify(soacResponse)}`)
    console.log("Stake on a club transaction hash = " + soacResponse['transactionHash']);
}

async function assignStakesToAClub(deploymentDetails) {
    //let Admin assign stakeTo a club to Ajay
    // first increase allowance for club staking contract on Admin wallet to let it move fury
    let increaseAllowanceMsg = {
        increase_allowance: {
            spender: deploymentDetails.clubStakingAddress,
            amount: "1010000"
        }
    };
    let incrAllowResp = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, increaseAllowanceMsg);
    console.log(`Increase allowance response hash = ${incrAllowResp['transactionHash']}`);

	//let currTime = 10000000000;
    let soacRequest = {
        assign_stakes_to_a_club: {
            stake_list: 
			[
				{
					club_name: "ClubD",
					staker_address: ajay_wallet.wallet_address,
					staking_start_timestamp: "1640447808000000000",
					staked_amount: "1010000",
					staking_duration: 0,
					reward_amount: "0",
					auto_stake: true
				}
			],
			club_name: "ClubD"
        }
    };
    let soacResponse = await executeContract(mint_wallet, deploymentDetails.clubStakingAddress, soacRequest);
    console.log("Assign Stakes to a club transaction hash = " + soacResponse['transactionHash']);
}

async function withdrawStakeFromAClub(deploymentDetails) {
    let wsfacRequest = {
        stake_withdraw_from_a_club: {
            staker: sameer_wallet.wallet_address,
            club_name: "ClubB",
            amount: "10000",
            immediate_withdrawal: false
        }
    };
    // let platformFees = await queryContract(mint_wallet, deploymentDetails.clubStakingAddress, { query_platform_fees: { msg: Buffer.from(JSON.stringify(wsfacRequest)).toString('base64') } });
    // console.log(`platformFees = ${JSON.stringify(platformFees)}`);

//    let wsfacResponse = await executeContract(sameer_wallet, deploymentDetails.clubStakingAddress, wsfacRequest, { 'uusd': Number(platformFees) });
    let wsfacResponse = await executeContract(sameer_wallet, deploymentDetails.clubStakingAddress, wsfacRequest, {"denom":"ujunox","amount":"130"});
    console.log("Withdraw Stake on a club transaction hash = " + wsfacResponse['transactionHash']);

    console.log("Waiting for 30sec to try early Withdraw - would fail");
    //ADD DELAY small to check failure of quick withdraw - 30sec
    await new Promise(resolve => setTimeout(resolve, 30000));

    wsfacRequest = {
        stake_withdraw_from_a_club: {
            staker: sameer_wallet.wallet_address,
            club_name: "ClubB",
            amount: "10000",
            immediate_withdrawal: true
        }
    };
    try {
        wsfacResponse = await executeContract(sameer_wallet, deploymentDetails.clubStakingAddress, wsfacRequest,{"denom":"ujunox","amount":"130"});
        console.log("Not expected to reach here");
        console.log("Withdraw Stake on a club transaction hash = " + wsfacResponse['transactionHash']);
    } catch (error) {
        console.log("Failure as expected");
        console.log("Waiting for 100sec to try Withdraw after bonding period 2min- should pass");
        //ADD DELAY to reach beyond the bonding duration - 2min
        await new Promise(resolve => setTimeout(resolve, 100000));

        wsfacResponse = await executeContract(sameer_wallet, deploymentDetails.clubStakingAddress, wsfacRequest, {"denom":"ujunox","amount":"130"});
        console.log("Withdraw Stake on a club transaction hash = " + wsfacResponse['transactionHash']);
    } finally {
        console.log("Withdraw Complete");
    }
}

async function claimRewards(deploymentDetails) {
    let wsfacRequest = {
        claim_staker_rewards: {
            staker: sameer_wallet.wallet_address,
            club_name: "ClubB",
        }
    };
    // let platformFees = await queryContract(mint_wallet, deploymentDetails.clubStakingAddress, { query_platform_fees: { msg: Buffer.from(JSON.stringify(wsfacRequest)).toString('base64') } });
    // if (platformFees == 0) {
    //     platformFees = 1;
    // }
    
    // console.log(`platformFees = ${JSON.stringify(platformFees)}`);
    //let wsfacResponse = await executeContract(sameer_wallet, deploymentDetails.clubStakingAddress, wsfacRequest, { 'uusd': Number(platformFees) });
    let wsfacResponse = await executeContract(sameer_wallet, deploymentDetails.clubStakingAddress, wsfacRequest, {"denom":"ujunox","amount":"130"});
    console.log("Claim Rewards Platform Fees transaction hash = " + wsfacResponse['transactionHash']);
}

async function distributeRewards(deploymentDetails) {
    let iraRequest = {  
        increase_reward_amount: {
            reward_from: mint_wallet.wallet_address,
            amount: "100000"
        }
    };
	let msgString = Buffer.from(JSON.stringify(iraRequest)).toString('base64');
            
	let viaMsg = {
		send : {
			contract: deploymentDetails.clubStakingAddress, 
			amount: "100000",
			msg: msgString
		}
	};

   //  let iraResponse = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, viaMsg); //changed

    let iraResponse = await executeContract(mint_wallet, deploymentDetails.clubStakingAddress, iraRequest);

    //ADD DELAY small to check failure of quick withdraw - 30sec
    // await new Promise(resolve => setTimeout(resolve, 30000));

    let cadrRequestClubA = {
        calculate_and_distribute_rewards: {
            staker_list: 
			[
				ajay_wallet.wallet_address,
				nitin_wallet.wallet_address,
				sameer_wallet.wallet_address,
			],
			club_name: "ClubD",
			is_first_batch: true,
			is_final_batch: false,
        }
    };

	let cadrResponseClubA = await executeContract(mint_wallet, deploymentDetails.clubStakingAddress, cadrRequestClubA);
	console.log("distribute reward Club A transaction hash = " + cadrResponseClubA['transactionHash']);

    let cadrRequestClubB = {
        calculate_and_distribute_rewards: {
            staker_list: 
			[
				ajay_wallet.wallet_address,
				nitin_wallet.wallet_address,
				sameer_wallet.wallet_address,
			],
			club_name: "ClubB",
			is_first_batch: false,
			is_final_batch: true,
        }
    };

	let cadrResponseClubB = await executeContract(mint_wallet, deploymentDetails.clubStakingAddress, cadrRequestClubB);
	console.log("distribute reward Club B transaction hash = " + cadrResponseClubB['transactionHash']);
}

async function queryBalances(deploymentDetails, wallet, print) {
    let uusd_balance = await queryBankUusdNew(wallet, "ujunox");
    let fury_balance = await queryTokenBalance(mint_wallet, deploymentDetails.furyContractAddress,wallet.wallet_address);
    if (print) {console.log("wallet: " + wallet.wallet_address + " uusd: " + uusd_balance + " uFury: " + fury_balance)}
    return(fury_balance, uusd_balance)
}

async function queryBalancesForAddress(deploymentDetails, cAddress, print) {
    let uusd_balance = await queryBankUusdContract(mint_wallet, cAddress, "ujunox");
    let fury_balance = await queryTokenBalance(mint_wallet, deploymentDetails.furyContractAddress , cAddress);
    if (print) {console.log("wallet: " + mint_wallet.wallet_address + " uusd: " + uusd_balance + " uFury: " + fury_balance)}
    return(fury_balance, uusd_balance)
}

main();

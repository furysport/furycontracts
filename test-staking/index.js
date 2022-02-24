import * as readline from 'node:readline';
import { promisify } from 'util';
import { ajay_wallet, ClubStakingContractPath, liquidity_wallet, marketing_wallet, MintingContractPath, mintInitMessage, mint_wallet, nitin_wallet, sameer_wallet, team_wallet, terraClient, treasury_wallet } from './constants.js';
import { primeAccountsWithFunds } from "./primeCustomAccounts.js";
import { executeContract, instantiateContract, queryContract, readArtifact, storeCode, writeArtifact } from './utils.js';

const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
});
const question = promisify(rl.question).bind(rl);

async function main() {
    try {
        terraClient.chainID = "localterra";
        let deploymentDetails = readArtifact(terraClient.chainID);
        const primeAccounts = await question('Do you want to preload custom accounts? (y/N) ');
        if (primeAccounts === 'Y' || primeAccounts === 'y') {
            let txHash = await primeAccountsWithFunds();
            console.log(txHash);
            await proceedToSetup(deploymentDetails);
            deploymentDetails = readArtifact(terraClient.chainID);
            await performOperationsOnClubStaking(deploymentDetails);
        } else {
            await proceedToSetup(deploymentDetails);
            deploymentDetails = readArtifact(terraClient.chainID);
            await performOperationsOnClubStaking(deploymentDetails);
        }
    } catch (error) {
        console.log(error);
    } finally {
        rl.close();
    }
}

async function proceedToSetup(deploymentDetails) {
    const startFresh = await question('Do you want to upload and deploy fresh? (y/N)');
    if (startFresh === 'Y' || startFresh === 'y') {
        deploymentDetails = {};
    }
    if (!deploymentDetails.adminWallet) {
        deploymentDetails.adminWallet = mint_wallet.key.accAddress;
    }
    if (!deploymentDetails.teamWallet) {
        deploymentDetails.teamWallet = team_wallet.key.accAddress;
    }
    if (!deploymentDetails.nitinWallet) {
        deploymentDetails.nitinWallet = nitin_wallet.key.accAddress;
    }
    if (!deploymentDetails.ajayWallet) {
        deploymentDetails.ajayWallet = ajay_wallet.key.accAddress;
    }
    if (!deploymentDetails.sameerWallet) {
        deploymentDetails.sameerWallet = sameer_wallet.key.accAddress;
    }
    writeArtifact(deploymentDetails, terraClient.chainID);
    let result = await uploadFuryTokenContract(deploymentDetails);
    if (result) {
        result = await instantiateFuryTokenContract(deploymentDetails);
        if (result) {
            await setAstroProxyContractAddress(deploymentDetails);
            deploymentDetails = readArtifact(terraClient.chainID);
            await transferFuryToWallets(deploymentDetails);
            await uploadClubStaking(deploymentDetails);
            await instantiateClubStaking(deploymentDetails);
        }
    }
    console.log("Leaving proceedToSetup");
}

async function setAstroProxyContractAddress(deploymentDetails) {
    if (!deploymentDetails.astroProxyContractAddress) {
        const setAstroProxy = await question('Do you want to set the astro proxy contract address? (y/N)');
        if (setAstroProxy === 'Y' || setAstroProxy === 'y') {
            const astroProxyAddress = await question('Please provide the astro proxy contract address? ');
            deploymentDetails.astroProxyContractAddress = astroProxyAddress;
        }
        writeArtifact(deploymentDetails, terraClient.chainID);
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
                deployFury = false;
            }
        } else {
            console.log("Alright! Have fun!! :-)");
            return false;
        }
        if (deployFury) {
            console.log("Uploading Fury token contract");
            console.log(`mint_wallet = ${mint_wallet.key.accAddress}`);
            let contractId = await storeCode(mint_wallet, MintingContractPath); // Getting the contract id from local terra
            console.log(`Fury Token Contract ID: ${contractId}`);
            deploymentDetails.furyTokenCodeId = contractId;
            writeArtifact(deploymentDetails, terraClient.chainID);
        }
    }
    return true;
}

async function instantiateFuryTokenContract(deploymentDetails) {
    if (!deploymentDetails.furyContractAddress) {
        let instantiateFury = false;
        const answer = await question('Do you want to instantiate Fury Token Contract? (y/N) ');
        if (answer === 'Y' || answer === 'y') {
            instantiateFury = true;
        } else if (answer === 'N' || answer === 'n') {
            const contractAddress = await question('Please provide contract address for Fury Token contract: ');
            deploymentDetails.furyContractAddress = contractAddress;
            instantiateFury = false;
        } else {
            console.log("Alright! Have fun!! :-)");
            return false;
        }
        if (instantiateFury) {
            console.log("Instantiating Fury token contract");
            let initiate = await instantiateContract(mint_wallet, deploymentDetails.furyTokenCodeId, mintInitMessage);
            // The order is very imp
            let contractAddress = initiate.logs[0].events[0].attributes[3].value;
            console.log(`Fury Token Contract ID: ${contractAddress}`);
            deploymentDetails.furyContractAddress = contractAddress;
        }
        writeArtifact(deploymentDetails, terraClient.chainID);
    }
    return true;
}

async function transferFuryToWallets(deploymentDetails) {
    await transferFuryToTreasury(deploymentDetails);
    await transferFuryToLiquidity(deploymentDetails);
    await transferFuryToMarketing(deploymentDetails);
    await transferFuryToNitin(deploymentDetails);
    await transferFuryToAjay(deploymentDetails);
    await transferFuryToSameer(deploymentDetails);
}

async function transferFuryToTreasury(deploymentDetails) {
    let transferFuryToTreasuryMsg = {
        transfer: {
            recipient: treasury_wallet.key.accAddress,
            amount: "5000000000"
        }
    };
    console.log(`transferFuryToTreasuryMsg = ${JSON.stringify(transferFuryToTreasuryMsg)}`);
    let response = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, transferFuryToTreasuryMsg);
    console.log(`transferFuryToTreasuryMsg Response - ${response['txhash']}`);
}

async function transferFuryToMarketing(deploymentDetails) {
    let transferFuryToMarketingMsg = {
        transfer: {
            recipient: marketing_wallet.key.accAddress,
            amount: "50000000"
        }
    };
    console.log(`transferFuryToMarketingMsg = ${JSON.stringify(transferFuryToMarketingMsg)}`);
    let response = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, transferFuryToMarketingMsg);
    console.log(`transferFuryToMarketingMsg Response - ${response['txhash']}`);
}

async function transferFuryToLiquidity(deploymentDetails) {
    let transferFuryToLiquidityMsg = {
        transfer: {
            recipient: liquidity_wallet.key.accAddress,
            amount: "50000000"
        }
    };
    console.log(`transferFuryToLiquidityMsg = ${JSON.stringify(transferFuryToLiquidityMsg)}`);
    let response = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, transferFuryToLiquidityMsg);
    console.log(`transferFuryToLiquidityMsg Response - ${response['txhash']}`);
}

async function transferFuryToNitin(deploymentDetails) {
    let transferFuryToNitinMsg = {
        transfer: {
            recipient: nitin_wallet.key.accAddress,
            amount: "50000000"
        }
    };
    console.log(`transferFuryToNitinMsg = ${JSON.stringify(transferFuryToNitinMsg)}`);
    let response = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, transferFuryToNitinMsg);
    console.log(`transferFuryToNitinMsg Response - ${response['txhash']}`);
}

async function transferFuryToAjay(deploymentDetails) {
    let transferFuryToAjayMsg = {
        transfer: {
            recipient: ajay_wallet.key.accAddress,
            amount: "50000000"
        }
    };
    console.log(`transferFuryToAjayMsg = ${JSON.stringify(transferFuryToAjayMsg)}`);
    let response = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, transferFuryToAjayMsg);
    console.log(`transferFuryToAjayMsg Response - ${response['txhash']}`);
}

async function transferFuryToSameer(deploymentDetails) {
    let transferFuryToSameerMsg = {
        transfer: {
            recipient: sameer_wallet.key.accAddress,
            amount: "50000000"
        }
    };
    console.log(`transferFuryToSameerMsg = ${JSON.stringify(transferFuryToSameerMsg)}`);
    let response = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, transferFuryToSameerMsg);
    console.log(`transferFuryToSameerMsg Response - ${response['txhash']}`);
}

async function uploadClubStaking(deploymentDetails) {
    if (!deploymentDetails.clubStakingId) {
        console.log("Uploading Club Staking...");
        let contractId = await storeCode(mint_wallet, ClubStakingContractPath); // Getting the contract id from local terra
        console.log(`Club Staking Contract ID: ${contractId}`);
        deploymentDetails.clubStakingId = contractId;
        writeArtifact(deploymentDetails, terraClient.chainID);
    }
}

async function instantiateClubStaking(deploymentDetails) {
    if (!deploymentDetails.clubStakingAddress) {
        console.log("Instantiating Club Staking...");
        /*
        Club Price in this contract is 100000 (0.1 Fury) -  "club_price": "100000"
        Withdraw from a Club will mature after 5 minutes 300 seconds -  "bonding_duration": 300
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
            bonding_duration: 300,
            platform_fees: "100",
            transaction_fees: "30",
            control_fees: "50",
        }
        console.log(JSON.stringify(clubStakingInitMessage, null, 2));
        let result = await instantiateContract(mint_wallet, deploymentDetails.clubStakingId, clubStakingInitMessage);
        let contractAddresses = result.logs[0].events[0].attributes.filter(element => element.key == 'contract_address').map(x => x.value);
        deploymentDetails.clubStakingAddress = contractAddresses.shift();
        console.log(`Club Staking Contract Address: ${deploymentDetails.clubStakingAddress}`);
        writeArtifact(deploymentDetails, terraClient.chainID);
    }
}

async function performOperationsOnClubStaking(deploymentDetails) {
    await queryAllClubOwnerships(deploymentDetails);
    console.log("Balances of buyer");
    await queryBalances(deploymentDetails, deploymentDetails.nitinWallet);
    console.log("Balances of club_fee_collector");
    await queryBalances(deploymentDetails, deploymentDetails.teamWallet);
    console.log("Balances of platform_fee_collector");
    await queryBalances(deploymentDetails, deploymentDetails.adminWallet);
    await buyAClub(deploymentDetails);
    console.log("Balances of buyer");
    await queryBalances(deploymentDetails, deploymentDetails.nitinWallet);
    console.log("Balances of club_fee_collector");
    await queryBalances(deploymentDetails, deploymentDetails.teamWallet);
    console.log("Balances of platform_fee_collector");
    await queryBalances(deploymentDetails, deploymentDetails.adminWallet);
    await queryAllClubOwnerships(deploymentDetails);
    console.log("Balances of staker");
    await queryBalances(deploymentDetails, deploymentDetails.sameerWallet);
    await stakeOnAClub(deploymentDetails);
    console.log("Balances of staker");
    await queryBalances(deploymentDetails, deploymentDetails.sameerWallet);
    console.log("Balances of platform_fee_collector");
    await queryBalances(deploymentDetails, deploymentDetails.adminWallet);
}

async function queryAllClubOwnerships(deploymentDetails) {
    //Fetch club ownership details for all clubs
    let coResponse = await queryContract(deploymentDetails.clubStakingAddress, {
        all_club_ownership_details: {}
    });
    console.log("All clubs ownership = " + JSON.stringify(coResponse));

}

async function queryAllClubStakes(deploymentDetails) {
    //Fetch club Stakes details for all clubs
    let csResponse = await queryContract(deploymentDetails.clubStakingAddress, {
        club_staking_details: {
            club_name: "ClubB"
        }
    });
    console.log("All clubs stakes = " + JSON.stringify(csResponse));

}

async function buyAClub(deploymentDetails) {
    if (!deploymentDetails.clubBought) {
        //let Nitin buy a club
        // first increase allowance for club staking contract on nitin wallet to let it move fury
        let increaseAllowanceMsg = {
            increase_allowance: {
                spender: deploymentDetails.clubStakingAddress,
                amount: "100000"
            }
        };
        let incrAllowResp = await executeContract(nitin_wallet, deploymentDetails.furyContractAddress, increaseAllowanceMsg);
        console.log(`Increase allowance response hash = ${incrAllowResp['txhash']}`);

        let bacRequest = {
            buy_a_club: {
                buyer: nitin_wallet.key.accAddress,
                club_name: "ClubB"
            }
        };
        let platformFees = await queryContract(deploymentDetails.clubStakingAddress, { query_platform_fees: { msg: Buffer.from(JSON.stringify(bacRequest)).toString('base64') } });
        console.log(`platformFees = ${JSON.stringify(platformFees)}`);
        let bacResponse = await executeContract(nitin_wallet, deploymentDetails.clubStakingAddress, bacRequest, { 'uusd': Number(platformFees) });
        console.log("Buy a club transaction hash = " + bacResponse['txhash']);
        deploymentDetails.clubBought = true;
    }
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
    console.log(`Increase allowance response hash = ${incrAllowResp['txhash']}`);

    let soacRequest = {
        stake_on_a_club: {
            staker: sameer_wallet.key.accAddress,
            club_name: "ClubB",
            amount: "100000"
        }
    };
    let platformFees = await queryContract(deploymentDetails.clubStakingAddress, { query_platform_fees: { msg: Buffer.from(JSON.stringify(soacRequest)).toString('base64') } });
    console.log(`platformFees = ${JSON.stringify(platformFees)}`);
    let soacResponse = await executeContract(sameer_wallet, deploymentDetails.clubStakingAddress, soacRequest, { 'uusd': Number(platformFees) });
    console.log("Stake on a club transaction hash = " + soacResponse['txhash']);
}

async function queryBalances(deploymentDetails, accAddress) {
    let bankBalances = await terraClient.bank.balance(accAddress);
    console.log(JSON.stringify(bankBalances));
    let furyBalance = await queryContract(deploymentDetails.furyContractAddress, { balance: { address: accAddress } });
    console.log(JSON.stringify(furyBalance));
}
main();

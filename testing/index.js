import {
    mintInitMessage,
    MintingContractPath,
    VnDContractPath,
    walletTest1,
    walletTest2,
    walletTest3,
    gamified_airdrop_wallet,
    whitelist_airdrop_wallet,

    minting_wallet,
    treasury_wallet,
    marketing_wallet,
    terraClient,
    private_category_wallet,
    partnership_wallet
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
const assert = chai.assert;

const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
});
const question = promisify(rl.question).bind(rl);


const main = async () => {
    try {
        terraClient.chainID = "bombay-12";
        let deploymentDetails = readArtifact(terraClient.chainID);
        const primeAccounts = await question('Do you want to preload custom accounts? (y/N) ');
        if (primeAccounts === 'Y' || primeAccounts === 'y') {
            primeAccountsWithFunds().then((txHash) => {
                console.log(txHash);
                proceedToSetup(deploymentDetails);
            });
        } else {
            proceedToSetup(deploymentDetails);
        }
    } catch (error) {
        console.log(error);
    }
}

const proceedToSetup = async (deploymentDetails) => {
    const startFresh = await question('Do you want to upload and deploy fresh? (y/N)');
    if (startFresh === 'Y' || startFresh === 'y') {
        deploymentDetails = {};
    }
    if (!deploymentDetails.adminWallet) {
        deploymentDetails.adminWallet = minting_wallet.key.accAddress;
    }
    const sleep_time = 31000;
    uploadFuryTokenContract(deploymentDetails).then(() => {
        setTimeout(() => {
            instantiateFuryTokenContract(deploymentDetails).then(() => {
                setTimeout(() => {
                    uploadVestingNDistributionContract(deploymentDetails).then(() => {
                        setTimeout(() => {
                            instantiateVnD(deploymentDetails).then(() => {
                                setTimeout(() => {
                                    setAllowancesForVnDContract(deploymentDetails).then(() => {
                                        setTimeout(() => {
                                            console.log("deploymentDetails = " + JSON.stringify(deploymentDetails, null, ' '));
                                            rl.close();
                                            queryVestingDetailsForGaming(deploymentDetails).then(() => {
                                                setTimeout(() => {
                                                    performPeriodicDistribution(deploymentDetails).then(() => {
                                                        setTimeout(() => {
                                                            performPeriodicVesting(deploymentDetails).then(() => {
                                                                setTimeout(() => {
                                                                    claimVestedTokens(deploymentDetails).then(() => {
                                                                        setTimeout(() => {
                                                                            console.log("Finished!");
                                                                        }, sleep_time);
                                                                    });
                                                                }, sleep_time);
                                                            });
                                                        }, sleep_time);
                                                    });
                                                }, sleep_time);
                                            });
                                        }, sleep_time);
                                    });
                                }, sleep_time);
                            });
                        }, sleep_time);
                    });
                }, sleep_time);
            });
        }, sleep_time);
    });
}

const uploadFuryTokenContract = async (deploymentDetails) => {
    console.log(`terraClient.chainID = ${terraClient.chainID}`);
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
        }
        if (deployFury) {
            console.log("Uploading Fury token contract");
            console.log(`minting_wallet = ${minting_wallet.key}`);
            let contractId = await storeCode(minting_wallet, MintingContractPath); // Getting the contract id from local terra
            console.log(`Fury Token Contract ID: ${contractId}`);
            deploymentDetails.furyTokenCodeId = contractId;
            writeArtifact(deploymentDetails, terraClient.chainID);
        }
    }
}

const instantiateFuryTokenContract = async (deploymentDetails) => {
    if (!deploymentDetails.furyContractAddress) {
        let instantiateFury = false;
        const answer = await question('Do you want to instantiate Fury Token Contract? (y/N) ');
        if (answer === 'Y' || answer === 'y') {
            instantiateFury = true;
        } else if (answer === 'N' || answer === 'n') {
            const contractAddress = await question('Please provide contract address for Fury Token contract: ');
            deploymentDetails.furyContractAddress = contractAddress;
            instantiateFury = false;
        }
        if (instantiateFury) {
            console.log("Instantiating Fury token contract");
            let initiate = await instantiateContract(minting_wallet, deploymentDetails.furyTokenCodeId, mintInitMessage)
            // The order is very imp
            let contractAddress = initiate.logs[0].events[0].attributes[3].value;
            console.log(`Fury Token Contract ID: ${contractAddress}`)
            deploymentDetails.furyContractAddress = contractAddress;
            writeArtifact(deploymentDetails, terraClient.chainID);
        }
    }
}

const periodically_transfer_to_categories = async (deploymentDetails) => {

}

const transferFuryToTreasury = async (deploymentDetails) => {
    let transferFuryToTreasuryMsg = {
        transfer: {
            recipient: treasury_wallet.key.accAddress,
            amount: "5000000000"
        }
    };
    console.log(`transferFuryToTreasuryMsg = ${JSON.stringify(transferFuryToTreasuryMsg)}`);
    let response = await executeContract(minting_wallet, deploymentDetails.furyContractAddress, transferFuryToTreasuryMsg);
    console.log(`transferFuryToTreasuryMsg Response - ${response['txhash']}`);
}

const transferFuryToMarketing = async (deploymentDetails) => {
    let transferFuryToMarketingMsg = {
        transfer: {
            recipient: marketing_wallet.key.accAddress,
            amount: "50000000"
        }
    };
    console.log(`transferFuryToMarketingMsg = ${JSON.stringify(transferFuryToMarketingMsg)}`);
    let response = await executeContract(minting_wallet, deploymentDetails.furyContractAddress, transferFuryToMarketingMsg);
    console.log(`transferFuryToMarketingMsg Response - ${response['txhash']}`);
}

const uploadVestingNDistributionContract = async (deploymentDetails) => {
    if (!deploymentDetails.vndCodeId) {
        console.log("Uploading Vesting and Distribution contract");
        let contractId = await storeCode(minting_wallet, VnDContractPath); // Getting the contract id from local terra
        console.log(`VestNDistrib Contract ID: ${contractId}`);
        deploymentDetails.vndCodeId = contractId;
        writeArtifact(deploymentDetails, terraClient.chainID);
    }
}

const instantiateVnD = async (deploymentDetails) => {
    if (!deploymentDetails.vndAddress) {
        console.log("Instantiating Vesting and Distribute contract");
        let vndInitMessage = {
            admin_wallet: deploymentDetails.adminWallet,
            fury_token_contract: deploymentDetails.furyContractAddress,
            vesting: {
                vesting_schedules: [
                    {
                        address: gamified_airdrop_wallet.key.accAddress,
                        cliff_period: 0,
                        initial_vesting_count: "3950000000000",
                        parent_category_address: "",
                        should_transfer: true,
                        total_vesting_token_count: "79000000000000",
                        vesting_count_per_period: "69490740000",
                        vesting_periodicity: 20,
                    },
                    {
                        address: marketing_wallet.key.accAddress,
                        cliff_period: 0,
                        initial_vesting_count: "4200000000000",
                        parent_category_address: "",
                        should_transfer: false,
                        total_vesting_token_count: "42000000000000",
                        vesting_count_per_period: "21000000000",
                        vesting_periodicity: 10
                    },
                    {
                        address: partnership_wallet.key.accAddress,
                        cliff_period: 0,
                        initial_vesting_count: "1680000000000",
                        parent_category_address: "",
                        should_transfer: false,
                        total_vesting_token_count: "16800000000000",
                        vesting_count_per_period: "8400000000",
                        vesting_periodicity: 10
                    },
                    {
                        address: treasury_wallet.key.accAddress,
                        cliff_period: 0,
                        initial_vesting_count: "1260000000000",
                        parent_category_address: "",
                        should_transfer: false,
                        total_vesting_token_count: "12600000000000",
                        vesting_count_per_period: "6300000000",
                        vesting_periodicity: 10
                    },
                    {
                        address: private_category_wallet.key.accAddress,
                        cliff_period: 0,
                        initial_vesting_count: "1260000000000",
                        parent_category_address: minting_wallet.key.accAddress,
                        should_transfer: false,
                        total_vesting_token_count: "12600000000000",
                        vesting_count_per_period: "6300000000",
                        vesting_periodicity: 10
                    }
                ]
            }
        }

        let result = await instantiateContract(minting_wallet, deploymentDetails.vndCodeId, vndInitMessage)
        let contractAddress = result.logs[0].events[0].attributes.filter(element => element.key == 'contract_address').map(x => x.value);
        deploymentDetails.vndAddress = contractAddress.shift()
        writeArtifact(deploymentDetails, terraClient.chainID);
    }
}

const setAllowancesForVnDContract = async (deploymentDetails) => {
    if (!deploymentDetails.setAllowanceForVnD) {
        console.log(`Setting allowances for VnD contract ${deploymentDetails.vndAddress} on admin wallet ${deploymentDetails.adminWallet}`);
        let balanceResponse = await queryContract(deploymentDetails.furyContractAddress, {
            balance: { address: deploymentDetails.adminWallet }
        });
        console.log(`Balance for ${deploymentDetails.adminWallet} = ${JSON.stringify(balanceResponse)}`);
        let increaseAllowanceMsg = {
            increase_allowance: {
                spender: deploymentDetails.vndAddress,
                amount: balanceResponse.balance
            }
        };
        let incrAllowResp = await executeContract(minting_wallet, deploymentDetails.furyContractAddress, increaseAllowanceMsg);
        console.log(incrAllowResp['txhash']);
        deploymentDetails.setAllowanceForVnD = true;
        writeArtifact(deploymentDetails, terraClient.chainID);
    }
}

const performPeriodicDistribution = async (deploymentDetails) => {
    console.log("Performing periodic distribution");
    let periodicDistributionMsg = { periodically_transfer_to_categories: {} }
    let periodicDistributionResp = await executeContract(minting_wallet, deploymentDetails.vndAddress, periodicDistributionMsg);
    console.log(periodicDistributionResp['txhash']);
}

const performPeriodicVesting = async (deploymentDetails) => {
    console.log("Performing periodic vesting");
    let periodicVestingMsg = { periodically_calculate_vesting: {} };
    let periodicVestingResp = await executeContract(minting_wallet, deploymentDetails.vndAddress, periodicVestingMsg);
    console.log(periodicVestingResp['txhash']);
}

const claimVestedTokens = async (deploymentDetails) => {
    //Get balance of private_category_wallet
    console.log(`Claiming vested tokens for ${private_category_wallet.key.accAddress}`);
    let vesting_details = await queryContract(deploymentDetails.vndAddress, {
        vesting_details: { address: private_category_wallet.key.accAddress }
    });
    console.log(`vesting details of ${private_category_wallet.key.accAddress} : ${JSON.stringify(vesting_details)}`);
    let vestable = vesting_details['tokens_available_to_claim']
    if (vestable > 0) {
        let claimVestedTokensMsg = { claim_vested_tokens: { amount: vestable } };
        let claimVestingResp = await executeContract(private_category_wallet, deploymentDetails.vndAddress, claimVestedTokensMsg);
        console.log(claimVestingResp['txhash']);
    } else {
        console.log("Number of tokens available for claiming = " + vestable);
    }
    //Get balance of private_category_wallet
}

const queryVestingDetailsForGaming = async (deploymentDetails) => {
    let result = await queryContract(deploymentDetails.vndAddress, {
        vesting_details: { address: gamified_airdrop_wallet.key.accAddress }
    });
    console.log(`vesting details of ${gamified_airdrop_wallet.key.accAddress} : ${JSON.stringify(result)}`);
}


const queryPool = async (deploymentDetails) => {
    console.log("querying pool details");
    let poolDetails = await queryContract(deploymentDetails.proxyContractAddress, {
        pool: {}
    });
    console.log(JSON.stringify(poolDetails));
}


main()
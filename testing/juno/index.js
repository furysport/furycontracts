import dotenv from "dotenv";
import {
    bonded_lp_reward_wallet,
    FactoryContractPath,
    liquidity_wallet,
    marketing_wallet,
    mint_wallet,
    MintingContractPath,
    mintInitMessage,
    nitin_wallet,
    PairContractPath,
    ProxyContractPath,
    StakingContractPath,
    terraClient,
    treasury_wallet,
    VnDContractPath
} from './constants.js';
import {executeContract, instantiateContract, queryContract, readArtifact, storeCode, writeArtifact} from "./utils.js";

import {primeAccountsWithFunds} from "./primeCustomAccounts.js";

import * as readline from 'node:readline';

import * as chai from 'chai';

dotenv.config();
const assert = chai.assert;

const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout
});

// const question = promisify(rl.question).bind(rl);
function question(query) {
    return new Promise(resolve => {
        rl.question(query, resolve);
    })
}

let configResponseReceived;

const main = async () => {
    try {
        let deploymentDetails = readArtifact(terraClient.chainId);
        let primeAccounts = 'N';
        if (process.env.TERRA_CLIENT === "testing") {
            primeAccounts = await question('Do you want to preload custom accounts? (y/N) ');
        }
        if (primeAccounts === 'Y' || primeAccounts === 'y') {
            let txHash = await primeAccountsWithFunds();
            console.log(txHash);
            await proceedToSetup(deploymentDetails);
        } else {
            await proceedToSetup(deploymentDetails);
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
        deploymentDetails.adminWallet = mint_wallet.wallet_address;
    }
    if (!deploymentDetails.authLiquidityProvider) {
        deploymentDetails.authLiquidityProvider = treasury_wallet.wallet_address;
    }
    if (!deploymentDetails.defaultLPTokenHolder) {
        deploymentDetails.defaultLPTokenHolder = liquidity_wallet.wallet_address;
    }
    const sleep_time = (process.env.TERRA_CLIENT === "testing") ? 31 : 150;

    await uploadFuryTokenContract(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await instantiateFuryTokenContract(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await uploadVnDContract(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await instantiateVnDContract(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await VnDIncreaseAllowance(deploymentDetails)
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await performPeriodicDistribution(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await performPeriodicVesting(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await claimVestedFury(deploymentDetails, marketing_wallet);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await transferFuryToTreasury(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await transferFuryToMarketing(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await transferFuryToLiquidity(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await uploadProxyContract(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await instantiateProxyContract(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await uploadPairContract(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await uploadFactoryContract(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await instantiateFactory(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await transferFuryToFactory(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await transferNativeToFactory(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await queryProxyConfiguration(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await createPoolPairs(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await savePairAddressToProxy(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await queryProxyConfiguration(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    console.log(`TGE Vesting and Distribution`);
    await VnDPeriodic(deploymentDetails)

    console.log("deploymentDetails = " + JSON.stringify(deploymentDetails, null, ' '));
    rl.close();
    let response = await executeContract(mint_wallet, deploymentDetails.proxyContractAddress, {
        hello_sub: {}
    })
    console.log(`Response From Proxy HELLO SUB ${response['transactionHash']}`)
    await performOperations(deploymentDetails);
}

async function uploadFuryTokenContract(deploymentDetails) {
    console.log(`terraClient.chainId = ${terraClient.chainId}`);
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
            console.log(`mint_wallet = ${mint_wallet.wallet_address}`);
            console.log(JSON.stringify(mint_wallet.wallet_address));
            const uploadReciept = await storeCode(mint_wallet, MintingContractPath); // Getting the contract id from local terra
            const contractId = uploadReciept.codeId
            console.log(`Fury Token Contract ID: ${contractId}`);
            deploymentDetails.furyTokenCodeId = contractId;
            writeArtifact(deploymentDetails, terraClient.chainId);
        }
    }
}

async function instantiateFuryTokenContract(deploymentDetails) {
    if (!deploymentDetails.furyContractAddress) {
        let instantiateFury = false;
        const answer = await question('Do you want to instantiate Fury Token Contract? (y/N) ');
        if (answer === 'Y' || answer === 'y') {
            instantiateFury = true;
        } else if (answer === 'N' || answer === 'n') {
            deploymentDetails.furyContractAddress = await question('Please provide contract address for Fury Token contract: ');
            instantiateFury = false;
        }
        if (instantiateFury) {
            console.log("Instantiating Fury token contract");
            // let contractAddress = await instantiateContract(mint_wallet, deploymentDetails.furyTokenCodeId, mintInitMessage);
            let contractAddress = await mint_wallet.init(deploymentDetails.furyTokenCodeId, mintInitMessage)
            // The order is very imp
            console.log(`Fury Token Contract address: ${contractAddress}`);
            deploymentDetails.furyContractAddress = contractAddress;
            writeArtifact(deploymentDetails, terraClient.chainId);
        }
    }
}

async function uploadVnDContract(deploymentDetails) {
    console.log(`terraClient.chainId = ${terraClient.chainId}`);
    if (!deploymentDetails.VnDCodeId) {
        let deployVnD = false;
        const answer = await question('Do you want to upload Vesting and Distribution Contract? (y/N) ');
        if (answer === 'Y' || answer === 'y') {
            deployVnD = true;
        } else if (answer === 'N' || answer === 'n') {
            const codeId = await question('Please provide code id for Vesting and Distribution contract: ');
            if (isNaN(codeId)) {
                deployVnD = true;
            } else {
                deploymentDetails.VnDCodeId = codeId;
                deployVnD = false;
            }
        } else {
            console.log("Alright! Have fun!! :-)");
        }
        if (deployVnD) {
            console.log("Uploading VnD contract");
            console.log(`mint_wallet = ${mint_wallet.wallet_address}`);
            const uploadReciept = await storeCode(mint_wallet, VnDContractPath); // Getting the contract id from local terra
            const contractId = uploadReciept.codeId
            console.log(`VnD Contract ID: ${contractId}`);
            deploymentDetails.VnDCodeId = contractId;
            writeArtifact(deploymentDetails, terraClient.chainId);
        }
    }
}

async function instantiateVnDContract(deploymentDetails) {
    if (!deploymentDetails.VnDContractAddress) {
        let instantiateVnD = false;
        const answer = await question('Do you want to instantiate VnD Token Contract? (y/N) ');
        if (answer === 'Y' || answer === 'y') {
            instantiateVnD = true;
        } else if (answer === 'N' || answer === 'n') {
            const contractAddress = await question('Please provide contract address for VnD Token contract: ');
            deploymentDetails.VnDContractAddress = contractAddress;
            instantiateVnD = false;
        }
        if (instantiateVnD) {
            let VnDInitMessage = {
                admin_wallet: mint_wallet.wallet_address,
                fury_token_contract: deploymentDetails.furyContractAddress,
                vesting: {
                    vesting_schedules: [
                        {
                            address: treasury_wallet.wallet_address,
                            cliff_period: 0,
                            initial_vesting_count: "0000000",
                            parent_category_address: treasury_wallet.wallet_address,
                            should_transfer: true,
                            total_vesting_token_count: "42000000000000",
                            vesting_count_per_period: "69490740000",
                            vesting_periodicity: 10
                        },
                        {
                            address: liquidity_wallet.wallet_address,
                            cliff_period: 0,
                            initial_vesting_count: "7000000000000",
                            parent_category_address: liquidity_wallet.wallet_address,
                            should_transfer: true,
                            total_vesting_token_count: "21000000000000",
                            vesting_count_per_period: "140000000000",
                            vesting_periodicity: 120
                        },
                        {
                            address: bonded_lp_reward_wallet.wallet_address,
                            cliff_period: 0,
                            initial_vesting_count: "3150000000000",
                            parent_category_address: bonded_lp_reward_wallet.wallet_address,
                            should_transfer: true,
                            total_vesting_token_count: "31500000000000",
                            vesting_count_per_period: "630000000000",
                            vesting_periodicity: 60
                        },
                        {
                            address: marketing_wallet.wallet_address,
                            cliff_period: 0,
                            initial_vesting_count: "4000000",
                            parent_category_address: marketing_wallet.wallet_address,
                            should_transfer: false,
                            total_vesting_token_count: "40000004000000",
                            vesting_count_per_period: "20000000000",
                            vesting_periodicity: 30
                        },
                        {
                            address: nitin_wallet.wallet_address,
                            cliff_period: 0,
                            initial_vesting_count: "0",
                            parent_category_address: marketing_wallet.wallet_address,
                            should_transfer: false,
                            total_vesting_token_count: "1000000000",
                            vesting_count_per_period: "10000000",
                            vesting_periodicity: 2
                        }
                    ]
                }
            }
            // Last Category with cliff = 1 will vest after 1 week (clock time)
            // TOTAL total_vesting_token_count = 42000000000000+21000000000000+31500000000000+40000004000000+1000000000 = 134501004000000 

            console.log("Instantiating VnD token contract");
            let contractAddress = await mint_wallet.init(deploymentDetails.VnDCodeId, VnDInitMessage)
            // The order is very imp
            console.log(`VnD Token Contract address: ${contractAddress}`);
            deploymentDetails.VnDContractAddress = contractAddress;
            let response = await executeContract(mint_wallet, deploymentDetails.VnDContractAddress, {
                hello_sub: {}
            })
            console.log(`RESPONSE FROM TEST EXECUTE ${response['transactionHash']}`)
            writeArtifact(deploymentDetails, terraClient.chainId);
        }
    }
}

async function VnDIncreaseAllowance(deploymentDetails) {
    // TOTAL total_vesting_token_count = 42000000000000+21000000000000+31500000000000+40000004000000+1000000000 = 134501004000000 
    let increaseAllowanceMsg = {
        increase_allowance:
            {
                owner: mint_wallet.wallet_address,
                spender: deploymentDetails.VnDContractAddress,
                amount: "134501004000000"
            }
    };
    console.log(`Increasing the allowance for Vnd from fury_wallet`);
    let response = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, increaseAllowanceMsg);
    console.log(`Increased allowance for Vnd from fury_wallet - ${response['transactionHash']}`);
}

async function VnDPeriodic(deploymentDetails) {
    let VnDTransfer = {periodically_calculate_vesting: {}};
    let response = await executeContract(mint_wallet, deploymentDetails.VnDContractAddress, VnDTransfer);
    console.log(`periodically_calculate_vesting Response - ${response['transactionHash']}`);
    let VnDVest = {periodically_transfer_to_categories: {}};
    response = await executeContract(mint_wallet, deploymentDetails.VnDContractAddress, VnDVest);
    console.log(`periodically_transfer_to_categories Response - ${response['transactionHash']}`);
    await increasePOLRewardAllowance(deploymentDetails, bonded_lp_reward_wallet);
    await increasePOLRewardAllowance(deploymentDetails, liquidity_wallet);
}

async function transferFuryToTreasury(deploymentDetails) {
    let transferFuryToTreasuryMsg = {
        transfer: {
            recipient: treasury_wallet.wallet_address,
            amount: "5000000000"
        }
    };
    console.log(`transferFuryToTreasuryMsg = ${JSON.stringify(transferFuryToTreasuryMsg)}`);
    let response = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, transferFuryToTreasuryMsg);
    console.log(`transferFuryToTreasuryMsg Response - ${response['transactionHash']}`);
}

async function transferFuryTokens(deploymentDetails, toAddress, amount) {
    let transferFuryToTreasuryMsg = {
        transfer: {
            recipient: toAddress.wallet_address,
            amount: amount
        }
    };
    console.log(`transferFuryToTreasuryMsg = ${JSON.stringify(transferFuryToTreasuryMsg)}`);
    let response = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, transferFuryToTreasuryMsg);
    console.log(`transferFuryToTreasuryMsg Response - ${response['transactionHash']}`);
}

async function transferFuryToMarketing(deploymentDetails) {
    let transferFuryToMarketingMsg = {
        transfer: {
            recipient: marketing_wallet.wallet_address,
            amount: "50000000"
        }
    };
    console.log(`transferFuryToMarketingMsg = ${JSON.stringify(transferFuryToMarketingMsg)}`);
    let response = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, transferFuryToMarketingMsg);
    console.log(`transferFuryToMarketingMsg Response - ${response['transactionHash']}`);
}

async function transferFuryToLiquidity(deploymentDetails) {
    let transferFuryToLiquidityMsg = {
        transfer: {
            recipient: liquidity_wallet.wallet_address,
            amount: "50000000"
        }
    };
    console.log(`transferFuryToLiquidityMsg = ${JSON.stringify(transferFuryToLiquidityMsg)}`);
    let response = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, transferFuryToLiquidityMsg);
    console.log(`transferFuryToLiquidityMsg Response - ${response['transactionHash']}`);
}

async function uploadPairContract(deploymentDetails) {
    if (!deploymentDetails.pairCodeId) {
        console.log("Uploading pair contract (xyk)");
        let resp = await storeCode(mint_wallet, PairContractPath); // Getting the contract id from local terra
        console.log(`Pair Contract ID: ${resp.codeId}`);
        deploymentDetails.pairCodeId = resp.codeId;
        writeArtifact(deploymentDetails, terraClient.chainId);
    }
}


async function uploadWhiteListContract(deploymentDetails) {
    if (!deploymentDetails.whitelistCodeId) {
        console.log("Uploading whitelist contract");
        let resp = await storeCode(mint_wallet, StakingContractPath); // Getting the contract id from local terra
        console.log(`Whitelist Contract ID: ${resp.codeId}`);
        deploymentDetails.whitelistCodeId = resp.codeId;
        writeArtifact(deploymentDetails, terraClient.chainId);
    }
}

async function uploadFactoryContract(deploymentDetails) {
    if (!deploymentDetails.factoryCodeId) {
        console.log("Uploading factory contract");
        let resp = await storeCode(mint_wallet, FactoryContractPath); // Getting the contract id from local terra
        console.log(`Factory Contract ID: ${resp.codeId}`);
        deploymentDetails.factoryCodeId = resp.codeId;
        writeArtifact(deploymentDetails, terraClient.chainId);
    }
}

async function instantiateFactory(deploymentDetails) {
    if (!deploymentDetails.factoryAddress) {
        console.log("Instantiating factory contract");
        let factoryInitMessage = {
            "pair_code_id": deploymentDetails.pairCodeId,
            "token_code_id": deploymentDetails.furyTokenCodeId,
            "proxy_contract_addr": deploymentDetails.proxyContractAddress
        };
        console.log(JSON.stringify(factoryInitMessage, null, 2));
        let contractAddresses = await instantiateContract(mint_wallet, deploymentDetails.factoryCodeId, factoryInitMessage);
        deploymentDetails.factoryAddress = contractAddresses;
        writeArtifact(deploymentDetails, terraClient.chainId);
    }
}

async function uploadProxyContract(deploymentDetails) {
    if (!deploymentDetails.proxyCodeId) {
        console.log("Uploading proxy contract");
        let response = await storeCode(mint_wallet, ProxyContractPath); // Getting the contract id from local terra
        console.log(`Proxy Contract ID: ${response.codeId}`);
        deploymentDetails.proxyCodeId = response.codeId;
        writeArtifact(deploymentDetails, terraClient.chainId);
    }
}

async function instantiateProxyContract(deploymentDetails) {
    if (!deploymentDetails.proxyContractAddress) {
        console.log("Instantiating proxy contract");
        let proxyInitMessage = {
            /// admin address for configuration activities
            admin_address: mint_wallet.wallet_address,
            /// contract address of Fury token
            custom_token_address: deploymentDetails.furyContractAddress,

            /// discount_rate when fury and UST are both provided
            pair_discount_rate: 700,
            /// bonding period when fury and UST are both provided TODO 7*24*60*60
            pair_bonding_period_in_sec: 2 * 60,
            /// Fury tokens for balanced investment will be fetched from this wallet
            pair_fury_reward_wallet: liquidity_wallet.wallet_address,
            /// The LP tokens for all liquidity providers except
            /// authorised_liquidity_provider will be stored to this address
            /// The LPTokens for balanced investment are delivered to this wallet
            pair_lp_tokens_holder: liquidity_wallet.wallet_address,

            /// discount_rate when only UST are both provided
            native_discount_rate: 500,
            /// bonding period when only UST provided TODO 5*24*60*60
            native_bonding_period_in_sec: 3 * 60,
            /// Fury tokens for native(UST only) investment will be fetched from this wallet
            //TODO: Change to Bonded Rewards Wallet == (old name)community/LP incentives Wallet
            native_investment_reward_wallet: bonded_lp_reward_wallet.wallet_address,
            /// The native(UST only) investment will be stored into this wallet
            native_investment_receive_wallet: treasury_wallet.wallet_address,

            /// This address has the authority to pump in liquidity
            /// The LP tokens for this address will be returned to this address
            authorized_liquidity_provider: deploymentDetails.authLiquidityProvider,
            ///Time in nano seconds since EPOC when the swapping will be enabled
            swap_opening_date: "1644734115627110528",

            /// Pool pair contract address of astroport
            pool_pair_address: deploymentDetails.poolPairContractAddress,

            platform_fees_collector_wallet: mint_wallet.wallet_address,
            ///Specified in percentage multiplied by 100, i.e. 100% = 10000 and 0.01% = 1
            platform_fees: "100",
            ///Specified in percentage multiplied by 100, i.e. 100% = 10000 and 0.01% = 1
            transaction_fees: "30",
            ///Specified in percentage multiplied by 100, i.e. 100% = 10000 and 0.01% = 1
            swap_fees: "0",
            max_bonding_limit_per_user: 100,
            usdc_ibc_symbol: "ujunox"
        };
        console.log(JSON.stringify(proxyInitMessage, null, 2));
        deploymentDetails.proxyContractAddress = await instantiateContract(mint_wallet, deploymentDetails.proxyCodeId, proxyInitMessage);
        writeArtifact(deploymentDetails, terraClient.chainId);
    }
}

async function queryProxyConfiguration(deploymentDetails) {
    //Fetch configuration
    let configResponse = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {
        configuration: {}
    });
    configResponseReceived = configResponse;
    console.log(JSON.stringify(configResponseReceived));
}

async function transferNativeToFactory(deploymentDetails) {
    console.log(`Funding ${deploymentDetails.factoryAddress}`);
    const resp = await mint_wallet.send_funds(deploymentDetails.factoryAddress, "1", "ujunox");
    console.log(`Response from transferNativeToFactory : ${JSON.stringify(resp)}`);
    return resp;
}

async function transferFuryToFactory(deploymentDetails) {
    let transferFuryToLiquidityMsg = {
        transfer: {
            recipient: deploymentDetails.factoryAddress,
            amount: "50000000"
        }
    };
    console.log(`transferFuryToFactoryMsg = ${JSON.stringify(transferFuryToLiquidityMsg)}`);
    let response = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, transferFuryToLiquidityMsg);
    console.log(`transferFuryToLiquidityMsg Response - ${response['transactionHash']}`);
}


async function createPoolPairs(deploymentDetails) {
    if (!deploymentDetails.poolPairContractAddress) {
        let init_param = {proxy: deploymentDetails.proxyContractAddress};
        console.log(`init_param = ${JSON.stringify(init_param)}`);
        console.log(Buffer.from(JSON.stringify(init_param)).toString('base64'));
        let executeMsg = {
            create_pair: {
                asset_infos: [
                    {
                        token: {
                            contract_addr: deploymentDetails.furyContractAddress
                        }
                    },
                    {
                        native_token: {denom: "ujunox"}
                    }
                ],
                init_params: Buffer.from(JSON.stringify(init_param)).toString('base64')
            }
        };
        console.log(`executeMsg = ${JSON.stringify(executeMsg)}`);
        let response = await executeContract(mint_wallet, deploymentDetails.factoryAddress, executeMsg);

        console.log(`Create pair call response is: ${JSON.stringify(response)}`)

        const raw_log = JSON.parse(response.rawLog);

        console.log(`Raw_log is: ${raw_log}`)

        console.log(`Raw_log[0] is: ${JSON.stringify(raw_log[0])}`)

        const events = raw_log[0].events

        console.log(`Events is: ${events[1]}`)

        const attributes = events[1].attributes[0]

        console.log(`Attributes is: ${JSON.stringify(attributes)}`)

        deploymentDetails.poolPairContractAddress = attributes.value;

        console.log(`Pool pair contract address is: ${deploymentDetails.poolPairContractAddress}`);
        //FIXME This query contract needs to be fixed. 
        let pool_info = await queryContract(mint_wallet, deploymentDetails.poolPairContractAddress, {
            pair: {}
        });
        console.log("pool_info: " + JSON.stringify(pool_info));
        deploymentDetails.poolLpTokenAddress = pool_info.liquidity_token;
        writeArtifact(deploymentDetails, terraClient.chainId);
    }
}

async function savePairAddressToProxy(deploymentDetails) {
    if (!deploymentDetails.poolpairSavedToProxy) {
        //Fetch configuration
        //let configResponse = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {
        //    configuration: {}
        //});

        let configResponse = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {
            configuration: {}
        })
        configResponse.pool_pair_address = deploymentDetails.poolPairContractAddress;
        configResponse.liquidity_token = deploymentDetails.poolLpTokenAddress;
        console.log(`Configuration = ${JSON.stringify(configResponse)}`);
        let executeMsg = {
            configure: configResponse
        };
        console.log(`executeMsg = ${JSON.stringify(executeMsg, null, 2)}`);
        let response = await executeContract(mint_wallet, deploymentDetails.proxyContractAddress, executeMsg);
        console.log(`Save Response - ${response['transactionHash']}`);
        deploymentDetails.poolpairSavedToProxy = true;
        writeArtifact(deploymentDetails, terraClient.chainId);
    }
}

async function performOperations(deploymentDetails) {
    console.log("Performance Operations")
    const sleep_time = (process.env.TERRA_CLIENT === "testing") ? 31 : 15000;
    await checkLPTokenDetails(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await checkLPTokenBalances(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await transferFuryToTreasury(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await transferFuryTokens(deploymentDetails, bonded_lp_reward_wallet, "5000000000");
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    // TODO CURRENT ERROR IS HERE
    await provideLiquidityAuthorised(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await checkLPTokenBalances(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await queryPool(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await performSimulation(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await getFuryEquivalentToUST(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await buyFuryTokens(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await getUSTEquivalentToFury(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await sellFuryTokens(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await withdrawLiquidityAutorized(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await checkLPTokenBalances(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await provideNativeForRewards(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await providePairForReward(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await checkLPTokenBalances(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await queryInvestmentReward(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    await claimInvestmentReward(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    console.log(`Second Vesting and Distribution`);
    await VnDPeriodic(deploymentDetails)
    await new Promise(resolve => setTimeout(resolve, sleep_time));

    //await claimVestedFury(deploymentDetails, marketing_wallet);

    console.log("Finished operations");
}

async function checkLPTokenDetails(deploymentDetails) {
    let lpTokenDetails = await queryContract(mint_wallet, deploymentDetails.poolLpTokenAddress, {
        token_info: {}
    });
    console.log(JSON.stringify(lpTokenDetails));
    //assert.equal(lpTokenDetails['name'], "FURY-UUSD-LP");
    assert.equal(lpTokenDetails['name'], "terraswap liquidity token");
}

async function checkLPTokenBalances(deploymentDetails) {
    console.log("Getting LPToken balances");
    await queryContract(mint_wallet, deploymentDetails.poolLpTokenAddress, {
        all_accounts: {}
    }).then((allAccounts) => {
        console.log(JSON.stringify(allAccounts.accounts));
        allAccounts.accounts.forEach((account) => {
            queryContract(mint_wallet, deploymentDetails.poolLpTokenAddress, {
                balance: {address: account}
            }).then((balance) => {
                console.log(`Balance of ${account} : ${JSON.stringify(balance)}`);
            });
        });
    });
}

async function provideLiquidityAuthorised(deploymentDetails) {
    //First increase allowance for proxy to spend from mint_wallet wallet
    let increaseAllowanceMsg = {
        increase_allowance: {
            spender: deploymentDetails.proxyContractAddress,
            amount: "5000000000"
        }
    };
    let incrAllowResp = await executeContract(treasury_wallet, deploymentDetails.furyContractAddress, increaseAllowanceMsg);
    console.log(`Increase allowance response hash = ${incrAllowResp['transactionHash']}`);
    let executeMsg = {
        provide_liquidity: {
            assets: [
                {
                    info: {
                        native_token: {
                            denom: "ujunox"
                        }
                    },
                    amount: "500000000"
                },
                {
                    info: {
                        token: {
                            contract_addr: deploymentDetails.furyContractAddress
                        }
                    },
                    amount: "5000000000"
                }
            ]
        }
    };
    let funds = Number(500000000);
    console.log(`funds = ${funds}`);
    //let response = await executeContract(treasury_wallet, deploymentDetails.proxyContractAddress, executeMsg, {'uusdc': funds});
    let response = await executeContract(treasury_wallet, deploymentDetails.proxyContractAddress, executeMsg, {'ujonox': funds});
    console.log(`Provide Liquidity (from treasury) Response - ${response['transactionHash']}`);
}

async function withdrawLiquidityAutorized(deploymentDetails) {
    console.log(`withdraw liquidity using lptokens = 1000000000`);
    let withdrawMsg = {
        withdraw_liquidity: {
            sender: deploymentDetails.authLiquidityProvider,
            amount: "1000000000"
        }
    };
    let base64Msg = Buffer.from(JSON.stringify(withdrawMsg)).toString('base64');
    let executeMsg = {
        send: {
            contract: deploymentDetails.proxyContractAddress,
            amount: "1000000000",
            msg: base64Msg,
        }
    };
    let qResp = await executeContract(treasury_wallet, deploymentDetails.poolLpTokenAddress, executeMsg);
    console.log(`withdraw Liquidity (from treasury) Response - ${qResp['transactionHash']}`);
}

async function provideLiquidityGeneral(deploymentDetails) {
    //First increase allowance for proxy to spend from marketing_wallet wallet
    let increaseAllowanceMsg = {
        increase_allowance: {
            spender: deploymentDetails.proxyContractAddress,
            amount: "50000000"
        }
    };
    let incrAllowResp = await executeContract(marketing_wallet, deploymentDetails.furyContractAddress, increaseAllowanceMsg);
    console.log(`Increase allowance response hash = ${incrAllowResp['transactionHash']}`);
    let executeMsg = {
        provide_liquidity: {
            assets: [
                {
                    info: {
                        native_token: {
                            denom: "ujunox"
                        }
                    },
                    amount: "5000000"
                },
                {
                    info: {
                        token: {
                            contract_addr: deploymentDetails.furyContractAddress
                        }
                    },
                    amount: "50000000"
                }
            ]
        }
    };
    let tax = 0;
    console.log(`tax = ${tax}`);
    let funds = Number(5000000);
    funds = funds + Number(tax.amount);
    console.log(`funds = ${funds}`);
    let response = await executeContract(marketing_wallet, deploymentDetails.proxyContractAddress, executeMsg, {'ujunox': funds});
    console.log(`Provide Liquidity (from marketing) Response - ${response['transactionHash']}`);
}

async function providePairForReward(deploymentDetails) {
    //Get the pool details
    let ufuryCount;
    let uustCount;
    let poolDetails = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {
        pool: {}
    });
    poolDetails.assets.forEach(asset => {
        console.log(`asset = ${JSON.stringify(asset)}`);
        if (asset.info.native_token) {
            uustCount = asset.amount;
            console.log("Native Tokens = " + uustCount + asset.info.native_token.denom);
        }
        if (asset.info.token) {
            ufuryCount = asset.amount;
            console.log("Fury Tokens = " + ufuryCount + "uFury");
        }
    });

    let hundredPercent = Number(10000);
    let rate = hundredPercent - configResponseReceived.pair_discount_rate;
    let baseUstAmount = Number(5000);
    let furyForBaseUst = parseInt(baseUstAmount * Number(ufuryCount) / Number(uustCount));
    let totalFuryAmount = furyForBaseUst * Number(2);
    let incrAllowLW = parseInt(totalFuryAmount * hundredPercent / rate);
    console.log(`Increase allowance for liquidity by = ${incrAllowLW}`);
    //First increase allowance for proxy to spend from liquidity wallet
    let increaseAllowanceMsgLW = {
        increase_allowance: {
            spender: deploymentDetails.proxyContractAddress,
            amount: incrAllowLW.toString()
        }
    };
    let incrAllowRespLW = await executeContract(liquidity_wallet, deploymentDetails.furyContractAddress, increaseAllowanceMsgLW);
    console.log(`Increase allowance response hash = ${incrAllowRespLW['transactionHash']}`);

    //First increase allowance for proxy to spend from marketing_wallet wallet
    let increaseAllowanceMsg = {
        increase_allowance: {
            spender: deploymentDetails.proxyContractAddress,
            amount: furyForBaseUst.toString()
        }
    };
    let incrAllowResp = await executeContract(marketing_wallet, deploymentDetails.furyContractAddress, increaseAllowanceMsg);
    console.log(`Increase allowance response hash = ${incrAllowResp['transactionHash']}`);
    let executeMsg = {
        provide_pair_for_reward: {
            assets: [
                {
                    info: {
                        native_token: {
                            denom: "ujunox"
                        }
                    },
                    amount: baseUstAmount.toString()
                },
                {
                    info: {
                        token: {
                            contract_addr: deploymentDetails.furyContractAddress
                        }
                    },
                    amount: furyForBaseUst.toString()
                }
            ]
        }
    };
    let tax = 0;
    console.log(`tax = ${tax}`);
    let funds = baseUstAmount + Number(tax.amount);
    console.log(`funds + tax = ${funds}`);

    let platformFees = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {query_platform_fees: {msg: Buffer.from(JSON.stringify(executeMsg)).toString('base64')}});
    console.log(`platformFees = ${JSON.stringify(platformFees)}`);
    funds = funds + Number(platformFees);
    console.log(`funds + tax + platform fees = ${funds}`);

    let response = await executeContract(marketing_wallet, deploymentDetails.proxyContractAddress, executeMsg, {'ujunox': funds});
    console.log(`Provide Pair for Liquidity (from marketing) Response - ${response['transactionHash']}`);
}

async function claimInvestmentReward(deploymentDetails) {
    let qRes = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {
        get_bonding_details: {
            user_address: marketing_wallet.wallet_address
        }
    });

    let rewardClaimMsg = {
        reward_claim: {
            receiver: marketing_wallet.wallet_address,
            withdrawal_amount: "105298",
        }
    };

    console.log("Waiting for 1sec to try early Claim - would fail");
    //ADD DELAY small to check failure of quick withdraw - 1sec
    await new Promise(resolve => setTimeout(resolve, 1000));

    let platformFees = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {query_platform_fees: {msg: Buffer.from(JSON.stringify(rewardClaimMsg)).toString('base64')}});
    console.log(`platformFees = ${JSON.stringify(platformFees)}`);

    let response;

    try {
        console.log(`rewardClaimMsg = ${JSON.stringify(rewardClaimMsg)}`);
        console.log("Trying to Claim Pair Reward before Maturity");
        response = await executeContract(marketing_wallet, deploymentDetails.proxyContractAddress, rewardClaimMsg, {'uusdc': Number(platformFees)});
        console.log("Not expected to reach here");
        console.log(`Reward Claim Response - ${response['transactionHash']}`);
    } catch (error) {
        console.log("Failure as expected");
        console.log("Waiting for 120sec to try Claim after bonding period 2min- should pass");
        //ADD DELAY to reach beyond the bonding duration - 2min
        await new Promise(resolve => setTimeout(resolve, 120000));

        response = await executeContract(marketing_wallet, deploymentDetails.proxyContractAddress, rewardClaimMsg, {'ujunox': Number(platformFees)});
        console.log("Withdraw Reward transaction hash = " + response['transactionHash']);

        rewardClaimMsg = {
            reward_claim: {
                receiver: marketing_wallet.wallet_address,
                withdrawal_amount: "53781",
            }
        };
        await queryInvestmentReward(deploymentDetails);
        console.log("Waiting for 60sec more to try Claim Native Reward after bonding period 3min- should pass");
        console.log(`rewardClaimMsg = ${JSON.stringify(rewardClaimMsg)}`);
        //ADD DELAY small to check failure of quick withdraw - 60sec
        await new Promise(resolve => setTimeout(resolve, 60000));

        response = await executeContract(marketing_wallet, deploymentDetails.proxyContractAddress, rewardClaimMsg, {'ujunox': Number(platformFees)});
        console.log("Withdraw Reward transaction hash = " + response['transactionHash']);

    } finally {
        console.log("Withdraw Complete");
    }
}

async function provideNativeForRewards(deploymentDetails) {
    //Get the pool details
    let ufuryCount;
    let uustCount;
    let poolDetails = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {
        pool: {}
    });
    poolDetails.assets.forEach(asset => {
        console.log(`asset = ${JSON.stringify(asset)}`);
        if (asset.info.native_token) {
            uustCount = asset.amount;
            console.log("Native Tokens = " + uustCount + asset.info.native_token.denom);
        }
        if (asset.info.token) {
            ufuryCount = asset.amount;
            console.log("Fury Tokens = " + ufuryCount + "uFury");
        }
    });

    let hundredPercent = Number(10000);
    let rate = hundredPercent - configResponseReceived.native_discount_rate;
    let baseUstAmount = Number(5000);
    let furyForBaseUst = baseUstAmount * Number(ufuryCount) / Number(uustCount);
    console.log(`for ${baseUstAmount} the equivalent furys are ${furyForBaseUst}`);
    // let ustFuryEquivAmount = baseUstAmount * Number(10); // 10x of ust is fury and then total = fury + ust
    // let totalFuryAmount = ustFuryEquivAmount;
    let incrAllowLW = parseInt(furyForBaseUst * hundredPercent / rate);
    console.log(`Increase allowance for treasury by = ${incrAllowLW}`);
    //First increase allowance for proxy to spend from bonded_and_lp_rewards wallet
    let increaseAllowanceMsgLW = {
        increase_allowance: {
            spender: deploymentDetails.proxyContractAddress,
            amount: incrAllowLW.toString()
        }
    };
    let incrAllowRespLW = await executeContract(bonded_lp_reward_wallet, deploymentDetails.furyContractAddress, increaseAllowanceMsgLW);
    console.log(`Increase allowance response hash = ${incrAllowRespLW['transactionHash']}`);

    let executeMsg = {
        provide_native_for_reward: {
            asset: {
                info: {
                    native_token: {
                        denom: "ujunox"
                    }
                },
                amount: baseUstAmount.toString()
            }
        }
    };
    let tax = 0;
    console.log(`tax = ${tax}`);
    let funds = baseUstAmount + Number(tax.amount);
    console.log(`funds + tax = ${funds}`);

    let platformFees = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {query_platform_fees: {msg: Buffer.from(JSON.stringify(executeMsg)).toString('base64')}});
    console.log(`platformFees = ${JSON.stringify(platformFees)}`);
    funds = funds + Number(platformFees);
    console.log(`funds + tax + platform fees = ${funds}`);

    let response = await executeContract(marketing_wallet, deploymentDetails.proxyContractAddress, executeMsg, {'ujunox': funds});
    console.log(`Provide Native for Liquidity (from marketing) Response - ${response['transactionHash']}`);
}

async function queryPool(deploymentDetails) {
    console.log("querying pool details");
    let poolDetails = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {
        pool: {}
    });
    console.log(JSON.stringify(poolDetails));
}

async function performSimulation(deploymentDetails) {
    const sleep_time = (process.env.TERRA_CLIENT === "testing") ? 31 : 15000;
    await simulationOfferNative(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await simulationOfferFury(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await reverseSimulationAskNative(deploymentDetails);
    await new Promise(resolve => setTimeout(resolve, sleep_time));
    await reverseSimulationAskFury(deploymentDetails);
}

async function getFuryEquivalentToUST(deploymentDetails) {
    let ustCount = "10000";
    let furyCount = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {
        get_fury_equivalent_to_ust: {
            ust_count: ustCount
        }
    });
    console.log(`${ustCount} ujunox = ${furyCount} uFury`);
}

async function buyFuryTokens(deploymentDetails) {
    let buyFuryMsg = {
        swap: {
            to: mint_wallet.wallet_address,
            offer_asset: {
                info: {
                    native_token: {
                        denom: "ujunox"
                    }
                },
                amount: "10000"
            },
        }
    };
    let tax = 0;
    console.log(`tax = ${tax}`);
    let funds = 10000 + Number(tax.amount);
    console.log(`funds + tax = ${funds}`);

    let platformFees = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {query_platform_fees: {msg: Buffer.from(JSON.stringify(buyFuryMsg)).toString('base64')}});
    console.log(`platformFees = ${JSON.stringify(platformFees)}`);
    funds = funds + Number(platformFees);
    console.log(`funds + tax + platform fees = ${funds}`);

    let buyFuryResp = await executeContract(mint_wallet, deploymentDetails.proxyContractAddress, buyFuryMsg, {'ujunox': funds});
    console.log(`Buy Fury swap response tx hash = ${buyFuryResp['transactionHash']}`);
}

async function getUSTEquivalentToFury(deploymentDetails) {
    let furyCount = "1000000";
    let ustCount = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {
        get_ust_equivalent_to_fury: {
            fury_count: furyCount
        }
    });
    console.log(`${furyCount} uFury = ${ustCount} ujunox`);
}

async function sellFuryTokens(deploymentDetails) {
    let increaseAllowanceMsg = {
        increase_allowance: {
            spender: deploymentDetails.proxyContractAddress,
            amount: "1000000"
        }
    };
    let incrAllowResp = await executeContract(mint_wallet, deploymentDetails.furyContractAddress, increaseAllowanceMsg);
    console.log("increase allowance resp tx = " + incrAllowResp['transactionHash']);
    let sellFuryMsg = {
        swap: {
            to: mint_wallet.wallet_address,
            offer_asset: {
                info: {
                    token: {
                        contract_addr: deploymentDetails.furyContractAddress
                    }
                },
                amount: "1000000"
            }
        }
    };
    let platformFees = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {query_platform_fees: {msg: Buffer.from(JSON.stringify(sellFuryMsg)).toString('base64')}});
    console.log(`platformFees = ${JSON.stringify(platformFees)}`);
    let funds = Number(platformFees);
    console.log(`funds + platform fees = ${funds}`);

    let sellFuryResp = await executeContract(mint_wallet, deploymentDetails.proxyContractAddress, sellFuryMsg, {'ujunox': funds});
    console.log(`Sell Fury swap response tx hash = ${sellFuryResp['transactionHash']}`);
}

async function simulationOfferNative(deploymentDetails) {
    console.log("performing simulation for offering native coins");
    let simulationResult = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {
        simulation: {
            offer_asset: {
                info: {
                    native_token: {
                        denom: "ujunox"
                    }
                },
                amount: "100000000"
            }
        }
    });
    console.log(JSON.stringify(simulationResult));
}

async function simulationOfferFury(deploymentDetails) {
    console.log("performing simulation for offering Fury tokens");
    let simulationResult = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {
        simulation: {
            offer_asset: {
                info: {
                    token: {
                        contract_addr: deploymentDetails.furyContractAddress
                    }
                },
                amount: "100000000"
            }
        }
    });
    console.log(JSON.stringify(simulationResult));
}

async function reverseSimulationAskNative(deploymentDetails) {
    console.log("performing reverse simulation asking for native coins");
    let simulationResult = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {
        reverse_simulation: {
            ask_asset: {
                info: {
                    native_token: {
                        denom: "ujunox"
                    }
                },
                amount: "1000000"
            }
        }
    });
    console.log(JSON.stringify(simulationResult));
}

async function reverseSimulationAskFury(deploymentDetails) {
    console.log("performing reverse simulation asking for Fury tokens");
    let simulationResult = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {
        reverse_simulation: {
            ask_asset: {
                info: {
                    token: {
                        contract_addr: deploymentDetails.furyContractAddress
                    }
                },
                amount: "1000000"
            }
        }
    });
    console.log(JSON.stringify(simulationResult));
}

async function queryInvestmentReward(deploymentDetails) {
    let qRes = await queryContract(mint_wallet, deploymentDetails.proxyContractAddress, {
        get_bonding_details: {
            user_address: marketing_wallet.wallet_address
        }
    });
    console.log(`bonded reward query response ${JSON.stringify(qRes)}`);
}

const increasePOLRewardAllowance = async (deploymentDetails, wallet) => {
    let response = await queryContract(mint_wallet, deploymentDetails.furyContractAddress, {
        balance: {address: wallet.wallet_address}
    });
    let respBalance = Number(response.balance);
    response = await queryContract(mint_wallet, deploymentDetails.furyContractAddress, {
        allowance: {
            owner: wallet.wallet_address,
            spender: deploymentDetails.proxyContractAddress
        }
    });
    let respAllowance = Number(response.allowance);
    console.log(`fury : existing balance ${respBalance}, existing allowance ${respAllowance}, increase allowance by ${respBalance - respAllowance}`);
    if (respBalance > respAllowance) {
        let increase_amount = respBalance - respAllowance;
        let execMsg = {
            increase_allowance: {
                spender: deploymentDetails.proxyContractAddress,
                amount: increase_amount.toString()
            }
        };
        let execResponse = await executeContract(wallet, deploymentDetails.furyContractAddress, execMsg);
        console.log(`POL increase allowance by ${increase_amount} uFury for proxy in wallet ${wallet.wallet_address}, transactionHash ${execResponse['transactionHash']}`);
    }
}

const claimVestedFury = async (deploymentDetails, wallet) => {
    //console.log(JSON.stringify(wallet));
    let response = await queryContract(mint_wallet, deploymentDetails.VnDContractAddress, {
        vesting_details: {address: wallet.wallet_address}
    });
    console.log(`Query response: ${JSON.stringify(response)}`)
    let respBalance = Number(response.tokens_available_to_claim);
    let execMsg = {claim_vested_tokens: {amount: respBalance.toString()}};
    let execResponse = await executeContract(wallet, deploymentDetails.VnDContractAddress, execMsg);
    console.log(`Claim all Vested Tokens ${respBalance} uFury for wallet ${wallet.wallet_address}, transactionHash ${execResponse['transactionHash']}`);
}

const performPeriodicDistribution = async (deploymentDetails) => {
    console.log("Performing periodic distribution");
    let periodicDistributionMsg = {periodically_transfer_to_categories: {}}
    let periodicDistributionResp = await executeContract(mint_wallet, deploymentDetails.VnDContractAddress, periodicDistributionMsg);
    console.log(periodicDistributionResp['transactionHash']);
}

const performPeriodicVesting = async (deploymentDetails) => {
    console.log("Performing periodic vesting");
    let periodicVestingMsg = {periodically_calculate_vesting: {}};
    let periodicVestingResp = await executeContract(mint_wallet, deploymentDetails.VnDContractAddress, periodicVestingMsg);
    console.log(periodicVestingResp['transactionHash']);
}


main()

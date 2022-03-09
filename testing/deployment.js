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


import {
    vesting_and_distribution
} from "./index.js";
import {
    astroport_setup
} from "./astroport.js";

const sleep_time = 11000

function sleep(time) {
    return new Promise((resolve) => setTimeout(resolve, time));
}


const upload_contract = async function (file) {
    const contractId = await storeCode(walletTest1, file,)
    console.log(`New Contract Id For Gaming ${contractId}`)
}
console.log("Initiating Total Deployment")
await vesting_and_distribution()
await sleep(sleep_time)
await astroport_setup()
await sleep(sleep_time)
await upload_contract(GamingContractPath)
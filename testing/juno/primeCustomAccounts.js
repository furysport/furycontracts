import {
    ajay_wallet,
    liquidity_wallet,
    marketing_wallet,
    mint_wallet,
    nitin_wallet,
    sameer_wallet,
    treasury_wallet,
    walletTest1,
    walletTest2,
    walletTest3,
    walletTest4,
    walletTest5,
    walletTest6,
    walletTest7
} from './constants.js';


export const primeAccountsWithFunds = async () => {
    var txHash = [];
    txHash.push(await fundMintingWallet());
    txHash.push(await fundTreasuryWallet());
    txHash.push(await fundLiquidityWallet());
    txHash.push(await fundMarketingWallet());
    txHash.push(await fundNitinWallet());
    txHash.push(await fundAjayWallet());
    txHash.push(await fundSameerWallet());
    console.log("leaving primeCustomAccounts");
    return txHash;
}

async function fundMintingWallet() {
    console.log(`Funding ${treasury_wallet.publicKey}`);
    return await walletTest1.send_funds(treasury_wallet, {"10000": "usdc"})

}

async function fundTreasuryWallet() {
    console.log(`Funding ${treasury_wallet.publicKey}`);
    return await walletTest2.send_funds(mint_wallet, {"100000": "usdc"})

}

async function fundLiquidityWallet() {
    console.log(`Funding ${liquidity_wallet.publicKey}`);
    return await walletTest3.send_funds(liquidity_wallet, {"100000": "usdc"})

}

async function fundMarketingWallet() {
    console.log(`Funding ${marketing_wallet.publicKey}`);
    return await walletTest4.send_funds(marketing_wallet, {"100000": "usdc"})

}

async function fundNitinWallet() {
    console.log(`Funding ${nitin_wallet.publicKey}`);
    return await walletTest5.send_funds(nitin_wallet, {"100000": "usdc"})

}

async function fundAjayWallet() {
    console.log(`Funding ${ajay_wallet.publicKey}`);
    return await walletTest6.send_funds(ajay_wallet, {"100000": "usdc"})

}

async function fundSameerWallet() {
    console.log(`Funding ${sameer_wallet.publicKey}`);
    return await walletTest7.send_funds(sameer_wallet, {"100000": "usdc"})

}
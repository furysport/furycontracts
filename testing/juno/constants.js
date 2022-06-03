import {cosmos, mnemonic, Wallet} from "./wallet.js";

/*
docker run -it --name juno_node_1 -p 26656:26656 -p 26657:26657 -p 1317:1317 -e STAKE_TOKEN=ujunox -e UNSAFE_CORS=true ghcr.io/cosmoscontracts/juno:v5.0.1 ./setup_and_run.sh juno16g2rahf5846rxzp3fwlswy08fz8ccuwk03k57y
Use This command To Up the Local JUNO
* */

//-------------------------------
export const MintingContractPath = "../../artifacts/cw20_base.wasm"
export const VnDContractPath = "../../artifacts/vest_n_distribute.wasm"
export const PairContractPath = "../../artifacts/terraswap_pair.wasm"
// export const StakingContractPath = "../artifacts/astroport_staking.wasm"
// export const WhitelistContractPath = "../artifacts/astroport_whitelist.wasm"
export const FactoryContractPath = ".../../artifacts/terraswap_factory.wasm"
export const ProxyContractPath = "../../artifacts/terra_swap_proxy.wasm"
export const StakingContractPath = "../../artifacts/club_staking.wasm"

// Wallets
export const mint_wallet = new Wallet(mnemonic)
export const treasury_wallet = new Wallet(mnemonic)
export const liquidity_wallet = new Wallet(mnemonic)
export const marketing_wallet = new Wallet(mnemonic)
export const team_wallet = new Wallet(mnemonic)
export const nitin_wallet = new Wallet(mnemonic)
export const ajay_wallet = new Wallet(mnemonic)
export const sameer_wallet = new Wallet(mnemonic)
export const bonded_lp_reward_wallet = new Wallet(mnemonic)
export const walletTest1 = new Wallet(mnemonic)
export const walletTest2 = new Wallet(mnemonic)
export const walletTest3 = new Wallet(mnemonic)
export const walletTest4 = new Wallet(mnemonic)
export const walletTest5 = new Wallet(mnemonic)
export const walletTest6 = new Wallet(mnemonic)
export const walletTest7 = new Wallet(mnemonic)
export const deployer = new Wallet(mnemonic)
// Init
export const mintInitMessage = {
    name: "Fury",
    symbol: "FURY",
    decimals: 6,
    initial_balances: [
        {address: "juno1y7yph5knactz5vdj5gz5rynahqzfdtsyvvntpe", amount: "420000000000000"},
    ],
    mint: {
        minter: "juno1y7yph5knactz5vdj5gz5rynahqzfdtsyvvntpe",
        cap: "420000000000000"
    },
    marketing: {
        project: "crypto11.me",
        description: "This token in meant to be used for playing gamesin crypto11 world",
        marketing: "juno1y7yph5knactz5vdj5gz5rynahqzfdtsyvvntpe"
    },
}


export const terraClient = cosmos
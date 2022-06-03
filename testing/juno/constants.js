import {Cosmos} from "@cosmostation/cosmosjs";
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
        {address: "juno1ttjw6nscdmkrx3zhxqx3md37phldgwhggm345k", amount: "410000000000000"},
        {address: "juno1m46vy0jk9wck6r9mg2n8jnxw0y4g4xgl3csh9h", amount: "0"},
        {address: "juno1k20rlfj3ea47zjr2sp672qqscck5k5mf3uersq", amount: "0"},
        {address: "juno1wjq02nwcv6rq4zutq9rpsyq9k08rj30rhzgvt4", amount: "0"},
        {address: "juno19rgzfvlvq0f82zyy4k7whrur8x9wnpfcj5j9g7", amount: "0"},
        {address: "juno12g4sj6euv68kgx40k7mxu5xlm5sfat806umek7", amount: "0"},
        {address: deployer.publicKey, amount: "010000000000000"},
    ],
    mint: {
        minter: "juno1ttjw6nscdmkrx3zhxqx3md37phldgwhggm345k",
        cap: "420000000000000"
    },
    marketing: {
        project: "crypto11.me",
        description: "This token in meant to be used for playing gamesin crypto11 world",
        marketing: "juno1wjq02nwcv6rq4zutq9rpsyq9k08rj30rhzgvt4"
    },
}


export const terraClient = cosmos
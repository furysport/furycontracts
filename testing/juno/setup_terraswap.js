import {Wallet} from "./wallet.js";

// Setup
const mnemonic = "clip hire initial neck maid actor venue client foam budget lock catalog sweet steak waste crater broccoli pipe steak sister coyote moment obvious choose"
let wallet = new Wallet(mnemonic)
const sleepTime = 1500
const basePath = "../../artifacts/"
const factoryPath = basePath + "terraswap_factory.wasm"
const terraSwapTokenPath = basePath + "terraswap_token.wasm"
const swapPairPath = basePath + "terraswap_pair.wasm"
// Upload Terra Swap
let factory_code_id = await wallet.upload(factoryPath)
console.log(factory_code_id)
await wallet.sleep(sleepTime)
let token_code_id = await wallet.upload(terraSwapTokenPath)
console.log(token_code_id)
await wallet.sleep(sleepTime)
let pair_code_id = await wallet.upload(swapPairPath)
console.log(pair_code_id)
await wallet.sleep(sleepTime)
// Init Factory
let factory_init_response = await wallet.init(factory_code_id, {
    "pair_code_id": pair_code_id,
    "token_code_id": token_code_id,
    "proxy_contract_addr": ""
})
console.log(factory_init_response)

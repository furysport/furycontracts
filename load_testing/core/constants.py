import json

with open('json/localterra.json', 'r') as f:
    deployment_details = json.load(f)
GAMING_CONTRACT_PATH = "../artifacts/gaming_pool.wasm"
FURY_CONTRACT_ADDRESS = deployment_details.get("furyContractAddress")
PROXY_CONTRACT_ADDRESS = deployment_details.get("proxyContractAddress")
GAMING_INIT = {
    "minting_contract_address": FURY_CONTRACT_ADDRESS,
    "admin_address": "",
    "platform_fee": "100",
    "transaction_fee": "30",
    "game_id": "Game001",
    "platform_fees_collector_wallet": PROXY_CONTRACT_ADDRESS,
    "astro_proxy_address": PROXY_CONTRACT_ADDRESS,
}
MINTING_WALLET_MEMONIC = "awesome festival volume rifle diagram suffer rhythm knock unlock reveal marine transfer lumber faint walnut love hover beach amazing robust oppose moon west will"

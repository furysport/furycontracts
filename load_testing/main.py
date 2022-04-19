import logging
import sys
from pprint import pprint

import self as self

from core.constants import PROXY_CONTRACT_ADDRESS, LIQUIDITY_PROVIDER
from core.engine import Engine
from core.gaming import GamingTestEngine

debug = True
# This is the wallet with the most number of funds and so use it to seed and fund other wallets
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    handlers={
        logging.FileHandler("staking.log"),
        logging.StreamHandler(sys.stdout)
    }
)
# GamingTestEngine(debug).run_test_1(100)
# StakingTestEngine(debug).run_test_1(5000)
#
# with ThreadPoolExecutor(max_workers=10) as executor:
#     for i in range(1, 10):
#         future = executor.submit(GamingTestEngine(debug=debug, admin_wallet_memonic=None, admin_shift=i).run_test_1, 20)


# Swap Test

# engine = Engine(debug).seed_liquidity(LIQUIDITY_PROVIDER)

engine = GamingTestEngine(debug)
# engine.seed_liquidity(LIQUIDITY_PROVIDER)
# engine.load_ust(engine.minting_wallet.key.acc_address, "10000000000")
# pprint(engine.get_max_spread(50000000))
swap = {
    "swap": {
        "to": engine.gaming_contract_address,
        "offer_asset": {
            "info": {
                "native_token": {
                    "denom": "uusd"
                }
            },
            "amount": "100000000",
        },
        "max_spread": "0.05"
    }
}
response = engine.execute(engine.terra.wallets["test6"], PROXY_CONTRACT_ADDRESS, swap, {
    "uusd": "90000000"
})
pprint(response)

import logging
import sys

from core.gaming import GamingTestEngine

debug = True
# This is the wallet with the most number of funds and so use it to seed and fund other wallets
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    handlers={
        logging.FileHandler("debug.log"),
        logging.StreamHandler(sys.stdout)
    }
)
GamingTestEngine(debug).run_test_1(100)
#
# with ThreadPoolExecutor(max_workers=10) as executor:
#     for i in range(1, 10):
#         future = executor.submit(GamingTestEngine(debug=debug, admin_wallet_memonic=None, admin_shift=i).run_test_1, 20)

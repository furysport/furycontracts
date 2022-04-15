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

engine = GamingTestEngine(debug)
engine.run_test_1(20)

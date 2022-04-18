import logging

from terra_sdk.client.lcd import Wallet

from load_testing.core.constants import CLUB_STAKING_CONTRACT_PATH, CLUB_STAKING_INIT
from load_testing.core.engine import Engine

"""
Notes
Currently the testing on local_terra gets limited due the shared accounts and their execution limits,
in order to avoid this its always a good practice to create tests that work within segregated accounts.
Use 10 admins and all the other roles should come with 
"""
logger = logging.getLogger(__name__)


class StakingTestEngine(Engine):
    def __init__(self, debug, admin_wallet_memonic=None, admin_shift=None):
        super().__init__(debug, admin_wallet_memonic, admin_shift)
        logger.info("Staking Test Instantiated, Setting new Club owners")
        self.club_owners = self.generate_wallets(2)
        self.auto_stake = True
        self.amount_to_stake_per_club = "100000"
        [self.fund_wallet(owner) for owner in self.club_owners]
        self.contract_id = self.upload_wasm(self.admin_wallet, CLUB_STAKING_CONTRACT_PATH)
        CLUB_STAKING_INIT['admin_address'] = self.admin_wallet.key.acc_address
        CLUB_STAKING_INIT['platform_fees_collector_wallet'] = self.admin_wallet.key.acc_address
        CLUB_STAKING_INIT['club_fee_collector_wallet'] = self.admin_wallet.key.acc_address
        self.club_staking_address = self.instantiate(self.admin_wallet, str(self.contract_id), CLUB_STAKING_INIT)

    @staticmethod
    def get_club_name(owner: Wallet):
        return f"Club_{owner.key.acc_address}"

    def buy_club(self, wallet: Wallet):
        self.increase_allowance(wallet, self.club_staking_address, "100000")
        buy_a_club_request = {
            'buyer': wallet.key.acc_address,
            'club_name': self.get_club_name(wallet),
            'auto_stake': self.auto_stake
        }
        logger.info("Getting Platform Fees for the Purchase")
        platform_fees = self.query_contract(self.club_staking_address, {
            "query_platform_fees": {
                "msg": self.base64_encode_dict(buy_a_club_request)
            }
        })
        logger.info(f"Platform Fee For The Purchase {platform_fees}")
        logger.info(f"Buying Club {self.get_club_name(wallet)} With {wallet.key.acc_address}")
        response = self.sign_and_execute_contract(wallet, self.club_staking_address, {"uusd": str(platform_fees)})
        logger.info(f"Buy a club response: {response.txhash}")

    def setup_clubs(self):
        for owner in self.club_owners:
            self.buy_club(owner)

    def stake_to_club(self, wallet: Wallet, club_name: str):
        logger.info(f"Initiating Staaking for {wallet.key.acc_address} On Club {club_name}")
        self.increase_allowance(wallet, self.club_staking_address, self.amount_to_stake_per_club)
        logger.info("Getting Platform Fees For Staking On The Club")
        stake_on_a_club_request = {
            'stake_on_a_club': {
                'staker': wallet.key.acc_address,
                'club_name': club_name,
                'amount': self.amount_to_stake_per_club,
                'auto_stake': self.auto_stake,
            }
        }
        platform_fees = self.query_contract(self.club_staking_address, {
            "query_platform_fees": {
                "msg": self.base64_encode_dict(stake_on_a_club_request)
            }
        })
        logger.info(f"Response Of Platform Fees {platform_fees}")
        logger.info("Executing Stake On a Club")
        response = self.sign_and_execute_contract(
            wallet,
            self.club_staking_address,
            stake_on_a_club_request,
            {"uusd": platform_fees}
        )
        logger.info(f"Staking On a Club TX Hash {response.txhash}")

    def run_test_1(self, number_of_users):
        self.setup_clubs()
        logger.info(f"Loading {number_of_users} Users for Test")
        wallets_for_test = self.generate_wallets(number_of_users)
        for wallet in wallets_for_test:
            self.fund_wallet(wallet)
            for owner in self.club_owners:
                self.stake_to_club(wallet, self.get_club_name(owner))

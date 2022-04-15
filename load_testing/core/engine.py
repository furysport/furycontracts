import base64
import datetime
import json
import logging
from time import sleep
from typing import Optional

import requests
from core.constants import MINTING_WALLET_MEMONIC, PROXY_CONTRACT_ADDRESS, FURY_CONTRACT_ADDRESS
from terra_sdk.client.lcd import LCDClient, Wallet
from terra_sdk.client.localterra import LocalTerra
from terra_sdk.core import Coins
from terra_sdk.core.bank import MsgSend
from terra_sdk.core.broadcast import BlockTxBroadcastResult
from terra_sdk.core.wasm import MsgInstantiateContract, MsgExecuteContract, MsgStoreCode
from terra_sdk.key.mnemonic import MnemonicKey
from terra_sdk.util.contract import get_code_id, read_file_as_b64, get_contract_address

logger = logging.getLogger(__name__)


class Engine(object):
    """
    Engine is a base for any test process and is meant to be inherited from.
    Methods from engine include handler for init,execute and query.
    Along with this Engine also has short hands for commonly used methods.
    NOTE:
    Admin shift is only applicable for LOCAL TERRA Use, we can use it to shift the admin to any from test_1 to test_10
    """

    def __init__(
            self,
            debug,
            admin_wallet_memonic=None,
            admin_shift=None
    ):
        self.debug = debug
        self.config = {}
        logger.info(f"NEW TEST RUN AT {datetime.datetime.now()}")
        if not self.debug:
            logger.info("Test-Net")
            self.sleep_time = 31000
            res = requests.get("https://fcd.terra.dev/v1/txs/gas_prices")
            self.terra = LCDClient(
                chain_id="bombay-12",
                url="https://bombay-lcd.terra.dev",
                gas_prices=Coins(res.json()),
                gas_adjustment="1.4")
        else:
            self.sleep_time = 0
            self.terra = LocalTerra()
        if admin_wallet_memonic:
            self.admin_wallet = self.generate_wallet(admin_wallet_memonic)
        else:
            if admin_shift:
                self.admin_wallet = self.terra.wallets[f"test{admin_shift}"]
            else:
                self.admin_wallet = self.terra.wallets["test1"]

        self.minting_wallet = self.generate_wallet(MINTING_WALLET_MEMONIC)
        logger.info(f"Current Admin Address:{self.admin_wallet.key.acc_address}")

    def execute(
            self,
            wallet: Wallet,
            contract_address: str,
            execute_msg: dict,
            coins: dict = None
    ) -> BlockTxBroadcastResult:
        instantiate = MsgExecuteContract(
            sender=wallet.key.acc_address,
            contract=contract_address,
            execute_msg=execute_msg,
            coins=coins,
        )
        fee = self.estimate_fee([instantiate], wallet)
        execute_tx = wallet.create_and_sign_tx(msgs=[instantiate], fee=fee, fee_denoms=['uusd'])
        execute_tx_result = self.terra.tx.broadcast(execute_tx)
        return execute_tx_result

    def sign_and_execute_contract(self, wallet, contract, messages, fee=None, ):
        message_list = []

        for message in messages:
            message_list.append(
                MsgExecuteContract(sender=wallet.key.acc_address, contract=contract, execute_msg=message)
            )
        if not fee:
            fee = self.estimate_fee(message_list, wallet)
        signed_message = wallet.create_and_sign_tx(
            msgs=message_list,
            fee=fee,

        )
        return self.terra.tx.broadcast(signed_message)

    def instantiate(
            self,
            wallet: Wallet,
            code_id: str,
            init_msg: dict,
            init_coins: dict = None
    ) -> str:
        instantiate = MsgInstantiateContract(
            wallet.key.acc_address,
            wallet.key.acc_address,
            code_id,
            init_msg,
            init_coins,
        )
        fee = self.estimate_fee([instantiate], wallet)
        instantiate_tx = wallet.create_and_sign_tx(msgs=[instantiate], fee=fee)
        instantiate_tx_result = self.terra.tx.broadcast(instantiate_tx)
        contract_address = get_contract_address(instantiate_tx_result)
        return contract_address

    def upload_wasm(
            self,
            wallet: Wallet,
            artifact_path: str,
    ) -> int:
        file_bytes = read_file_as_b64(artifact_path)
        store_code = MsgStoreCode(wallet.key.acc_address, file_bytes)
        fee = self.estimate_fee([store_code], wallet)
        store_code_tx = wallet.create_and_sign_tx(msgs=[store_code], fee=fee)
        store_code_tx_result = self.terra.tx.broadcast(store_code_tx)
        code_id = int(get_code_id(store_code_tx_result))
        logger.info("New Code ID:%s", code_id)
        return code_id

    def query_contract(self, contract_address: str, query_msg: dict):
        return self.terra.wasm.contract_query(contract_address, query_msg)

    def generate_wallet(self, mnemonic: Optional[str] = None) -> Wallet:
        mk = MnemonicKey(mnemonic=mnemonic)
        wallet = self.terra.wallet(mk)
        return wallet

    def generate_wallets(self, number):
        return [self.generate_wallet() for _ in range(0, number)]

    def sleep(self):
        """
        This method will sleep for the default time set with respect to debug
        :return:
        """
        sleep(self.sleep_time)

    def load_fury(self, to_address, amount: str):
        """
        This method accepts address and amount,
        Since the amount is Uint128 Format we encode it as str in the msg.
        """
        logger.info(f"Loading Fury Balance of {amount} to {to_address}")
        msg = MsgExecuteContract(
            sender=self.minting_wallet.key.acc_address,
            contract=FURY_CONTRACT_ADDRESS,
            execute_msg={
                "transfer": {
                    "amount": str(amount),
                    "recipient": to_address
                }
            })
        fee = self.estimate_fee([msg], self.minting_wallet)
        execute_tx = self.minting_wallet.create_and_sign_tx([msg], fee=fee)
        response = self.terra.tx.broadcast(execute_tx)
        logger.info(f"Load Fury Balance Response Hash {response.txhash}")

    def get_fury_equivalent_to_ust(self, ust_count):
        return self.query_contract(PROXY_CONTRACT_ADDRESS, {
            "get_fury_equivalent_to_ust": {
                "ust_count": ust_count
            }
        })

    def get_ust_equivalent_to_fury(self, fury_count):
        return self.query_contract(PROXY_CONTRACT_ADDRESS, {
            "get_ust_equivalent_to_fury": {
                "fury_count": fury_count
            }
        })

    def load_ust(self, to_address, amount):
        logger.info(f"Loading UST Balance of {amount} to {to_address}")
        msg = MsgSend(
            self.admin_wallet.key.acc_address,
            to_address,
            {"uusd": amount, "uluna": "100000000"}
        )
        fee = self.estimate_fee([msg], self.admin_wallet)
        execute_tx = self.admin_wallet.create_and_sign_tx([msg], fee=fee)
        response = self.terra.tx.broadcast(execute_tx)
        logger.info(f"Response Hash From UST Trasfer:{response.txhash}")

    def fund_wallet(self, wallet: Wallet):
        """
        This method will Fund any provided wallet with LUNA AND UST COINS and AlSO FURY TOKENS
        :param wallet:
        :return:
        """
        address = wallet.key.acc_address if type(wallet) != str else wallet
        logger.info(f"Funding Wallet {address}")
        self.load_fury(address, "100000000")
        self.load_ust(address, 50000000)

    def estimate_fee(self, message_list, wallet):
        estimate_fee = self.terra.tx.estimate_fee(
            sender=wallet.key.acc_address,
            gas_adjustment=2,
            gas_prices=Coins.from_str('0.013uluna'),
            msgs=message_list,
        )
        return estimate_fee

    @staticmethod
    def divide_to_batches(principal_list, chunk_size):
        """
        This method will auto-split the given list into chunks to make it easier to batchsize
        :param principal_list: list
        :param chunk_size: int
        :return:[[1,2,3],[4,5,6],...]
        """
        for i in range(0, len(principal_list), chunk_size):
            yield principal_list[i:i + chunk_size]

    def get_wallet_from_addr(self, address):
        return {
            "key": {
                "acc_address": address
            }
        }

    def increase_allowance(self, sender: Wallet, spender: str, amount: str):
        logger.info(f"Performing Increase Allowance From {sender.key.acc_address} to {spender} for {amount} $FURY")
        response = self.sign_and_execute_contract(sender, FURY_CONTRACT_ADDRESS, {
            "increase_allowance": {
                "spender": "club_staking_address",
                "amount": "100000"
            }
        })
        logger.info(f"Increase Allowance Response Hash :{response.txhash}")

    @staticmethod
    def base64_encode_dict(dict_: dict):
        return base64.urlsafe_b64encode(json.dumps(dict_).encode()).decode()

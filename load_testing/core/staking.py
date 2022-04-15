from load_testing.core.engine import Engine


class StakingTestEngine(Engine):
    def __init__(self, debug, admin_wallet_memonic=None, admin_shift=None):
        super().__init__(debug, admin_wallet_memonic, admin_shift)

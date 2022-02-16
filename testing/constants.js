import {LocalTerra} from "@terra-money/terra.js";
import {get_server_epoch_seconds} from "./utils.js";
import {MnemonicKey} from '@terra-money/terra.js';

// Contracts
export const MintingContractPath = "../artifacts/cw20_base.wasm"
export const VnDContractPath = "../artifacts/vest_n_distribute.wasm"
export const StakingContractPath = "../artifacts/astroport_staking.wasm"
export const WhitelistContractPath = "../artifacts/astroport_whitelist.wasm"
export const FactoryContractPath = "../artifacts/astroport_factory.wasm"
export const ProxyContractPath = "../artifacts/astroport_proxy.wasm"

export const terraClient = new LocalTerra();

// Accounts
export const deployer = terraClient.wallets.test1; // used as operator on all contracts
// These can be the client wallets to interact
export const walletTest1 = terraClient.wallets.test1;
export const walletTest2 = terraClient.wallets.test2;
export const walletTest3 = terraClient.wallets.test3;
export const walletTest4 = terraClient.wallets.test4;
export const walletTest5 = terraClient.wallets.test5;
export const walletTest10 = terraClient.wallets.test10;
// export const mint_wallet = "terra1ttjw6nscdmkrx3zhxqx3md37phldgwhggm345k";
// export const gamifiedairdrop = "terra1m46vy0jk9wck6r9mg2n8jnxw0y4g4xgl3csh9h";
// export const privatecategory = "terra1k20rlfj3ea47zjr2sp672qqscck5k5mf3uersq";
// export const marketing = "terra1wjq02nwcv6rq4zutq9rpsyq9k08rj30rhzgvt4";
// export const advisory = "terra19rgzfvlvq0f82zyy4k7whrur8x9wnpfcj5j9g7";
// export const sameerkey = "terra12g4sj6euv68kgx40k7mxu5xlm5sfat806umek7";
const mkga = new MnemonicKey({mnemonic: "guess couch drip increase gossip juice bachelor wood pilot host wire august morning advice property odor book august force oak exclude craft soda bag",});
export const gamified_airdrop_wallet = terraClient.wallet(mkga);

const mkwa = new MnemonicKey({mnemonic: "runway now diesel vibrant suspect light love exhibit skull right promote voyage develop broom roast soup habit snap pupil liberty man warrior stone state",});
export const whitelist_airdrop_wallet = terraClient.wallet(mkwa);

const mksti = new MnemonicKey({mnemonic: "garage solar dinner lawn upset february clarify cage drip jewel inherit member omit nurse pulse forest flush cannon penalty rib ladder slush element joy",});
export const star_terra_ido_wallet = terraClient.wallet(mksti);

const mklpi = new MnemonicKey({mnemonic: "kiwi bunker found artist script slim trade away sport manage manual receive obscure leader defense void bench mobile cricket naive surge pipe dream attend",});
export const lp_incentive_wallet = terraClient.wallet(mklpi);

const mkac = new MnemonicKey({mnemonic: "code tenant find country possible pulp cream away poet flee ugly galaxy brick mean label armor fee auction guess utility luxury clump exile occur",});
export const angel_category_wallet = terraClient.wallet(mkac);

const mksc = new MnemonicKey({mnemonic: "humor shoulder differ flame aisle ski noodle undo ghost solution calm crowd finish diesel correct mountain vote dirt hollow frost apple chronic opera soft",});
export const seed_category_wallet = terraClient.wallet(mksc);

const mkpc = new MnemonicKey({mnemonic: "clean antique turtle hill confirm skirt swim leader gaze replace evoke height tent olive key argue fall stool milk seed run visit eight foil",});
export const private_category_wallet = terraClient.wallet(mkpc);

const mkpp = new MnemonicKey({mnemonic: "common rare fitness goose spatial embody average half kind party gauge fee raise depend canvas sugar click pudding wrong purpose mango tonight suit tragic",});
export const pylon_public_wallet = terraClient.wallet(mkpp);

const mktsp = new MnemonicKey({mnemonic: "size decade collect shop burger among castle jelly skill witness void stomach engine charge enroll laugh appear quality renew razor pass rescue else dry",});
export const terraswap_public_wallet = terraClient.wallet(mktsp);

const mkmarketing = new MnemonicKey({mnemonic: "bread profit three cabbage guitar butter super firm more state lonely plunge grit august grid laundry discover trade dragon hazard badge journey news say",});
export const marketing_wallet = terraClient.wallet(mkmarketing);

const mkbonus = new MnemonicKey({mnemonic: "hello clutch disorder turkey want shuffle you seven across kid around sniff kiwi toddler shallow cattle library jaguar claw side credit intact bleak security",});
export const bonus_wallet = terraClient.wallet(mkbonus);

const mkpartnership = new MnemonicKey({mnemonic: "document valve inform type cradle prison road cherry swamp shiver vital labor vehicle wide bag oak poem airport must garden solid detail engine spread",});
export const partnership_wallet = terraClient.wallet(mkpartnership);

const mkadvisory = new MnemonicKey({mnemonic: "limit start minor rule harsh family turtle morning salmon voyage profit smart route shiver boil weird sand soccer horn assume blood robust wrist north",});
export const advisory_wallet = terraClient.wallet(mkadvisory);

const mktm = new MnemonicKey({mnemonic: "clarify hen fashion future amateur civil apart unaware entire pass arena walk vanish step uniform apple teach calm middle smart all grief action slot",});
export const team_money_wallet = terraClient.wallet(mktm);

const mktreasury = new MnemonicKey({mnemonic: "wait tribe hard proud lyrics oblige enough assume tag appear breeze hint faculty tomato famous quarter elbow random across marine physical depart infant hobby",});
export const treasury_wallet = terraClient.wallet(mktreasury);

const mkecosystem = new MnemonicKey({mnemonic: "minute better actor exchange mom tool man suffer upgrade cargo radar dizzy alone spatial cinnamon nuclear height genuine orient blossom wing scatter middle furnace",});
export const ecosystem_wallet = terraClient.wallet(mkecosystem);

const mkliquidity = new MnemonicKey({mnemonic: "priority rough worth change shop adapt ritual trap palm trust worth hidden shaft speak common parent armor fantasy artist retreat derive jeans remove glove",});
export const liquidity_wallet = terraClient.wallet(mkliquidity);

const mkminting = new MnemonicKey({mnemonic: "awesome festival volume rifle diagram suffer rhythm knock unlock reveal marine transfer lumber faint walnut love hover beach amazing robust oppose moon west will",});
export const minting_wallet = terraClient.wallet(mkminting);

const mkgasfee = new MnemonicKey({mnemonic: "crew final success notable steel harbor bicycle maze open donkey off cloth adult spread kit only increase muffin alter drink caution rare garage hazard",});
export const gasfee_wallet = terraClient.wallet(mkgasfee);

const mktransaction = new MnemonicKey({mnemonic: "brand relax chest wolf announce humble awful leave reopen guess scout off never captain rookie dad jaguar wrestle security detail panda athlete fork upgrade",});
export const transaction_wallet = terraClient.wallet(mktransaction);

const mkrake_return = new MnemonicKey({mnemonic: "royal steel thought shift curve beach reward radar okay butter ceiling detail bamboo asset busy knock kit oxygen jar under remove advance state silver",});
export const rake_return_wallet = terraClient.wallet(mkrake_return);

export const swapinitMessage = {
    pair_code_id: 321,
    token_code_id: 123
}

export const mintInitMessage = {
    name: "Fury",
    symbol: "FURY",
    decimals: 6,
    initial_balances: [
        {address: "terra1ttjw6nscdmkrx3zhxqx3md37phldgwhggm345k",amount: "410000000000000"},
        {address: "terra1m46vy0jk9wck6r9mg2n8jnxw0y4g4xgl3csh9h",amount: "0"},
        {address: "terra1k20rlfj3ea47zjr2sp672qqscck5k5mf3uersq",amount: "0"},
        {address: "terra1wjq02nwcv6rq4zutq9rpsyq9k08rj30rhzgvt4",amount: "0"},
        {address: "terra19rgzfvlvq0f82zyy4k7whrur8x9wnpfcj5j9g7",amount: "0"},
        {address: "terra12g4sj6euv68kgx40k7mxu5xlm5sfat806umek7",amount: "0"},
        {address: deployer.key.accAddress, amount: "010000000000000"},
        ],
    mint: {
        minter: "terra1ttjw6nscdmkrx3zhxqx3md37phldgwhggm345k",
        cap: "420000000000000"
    },
    marketing: {
        project: "crypto11.me",
        description: "This token in meant to be used for playing gamesin crypto11 world",
        marketing: "terra1wjq02nwcv6rq4zutq9rpsyq9k08rj30rhzgvt4"
    },
}
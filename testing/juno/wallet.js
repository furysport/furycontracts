//import message from "@cosmostation/cosmosjs/src/messages/proto.js";
import fs from "fs";
import fetch from "node-fetch";
//import {Cosmos} from "@cosmostation/cosmosjs";
import {SigningCosmWasmClient} from "@cosmjs/cosmwasm-stargate";
import {DirectSecp256k1HdWallet} from "@cosmjs/proto-signing";
import {calculateFee, GasPrice} from "@cosmjs/stargate";
import wasmTxType from "cosmjs-types/cosmwasm/wasm/v1/tx.js";
import {toUtf8} from "@cosmjs/encoding";

const {MsgExecuteContract, MsgSend} = wasmTxType;

const debug = false
//const debug = true

const chainId = "juno"
//const lcdUrl = "http://localhost:1317"
//const endpoint = "http://localhost:26657";
const endpoint = "https://rpc.uni.juno.deuslabs.fi"

//const chainIdTestNet = "uni-3"
//const lcdUrlTestNet = "https://uni-api.blockpane.com"
const testnetMemonic = "patch rookie cupboard salon powder depend grass account crawl raise cigar swim sunny van monster fatal system edge loop matter course muffin rigid ill"
// juno1lm3y9pyznfdmdl8kj3rgj3afkm0xh6p7deh6wc
// Copy Memonic from the Terminal in which the Juno Node contrainer was upped
export const mnemonic = (debug) ? "pony candy predict vote pride yard ecology burden record very fever blush still good pull swarm iron face before crunch liquid steel upper spare" : testnetMemonic;


//export const cosmos = (debug) ? new Cosmos(lcdUrl, chainId) : new Cosmos(lcdUrlTestNet, chainIdTestNet);
//cosmos.setBech32MainPrefix("juno")
//console.log(cosmos.bech32MainPrefix)


export class Wallet {
    wallet_address;
    publicKey;
    privateKey;
    client;
    gasPrice;
    memonic;

    constructor(memonic) {

        this.memonic = memonic;
    }

    async initialize() {
        const wallet = await DirectSecp256k1HdWallet.fromMnemonic(this.memonic, {prefix: "juno"});
        const account = await wallet.getAccounts();
        this.wallet_address = account[0].address;
        this.client = await SigningCosmWasmClient.connectWithSigner(endpoint, wallet, {gasPrice: GasPrice.fromString("1ujunox")});
    }

    async sign_and_broadcast(messages) {

        //FIXME: Need to sign and broadcast messgaes
        const memo = "sign_and_broadcast_memo";
        return this.client.signAndBroadcast(this.wallet_address, messages, "auto", memo)
    }

    async send_funds(to_address, amount, denom) {

        return this.sign_and_broadcast([{
            typeUrl: "/cosmos.bank.v1beta1.MsgSend",
            value: {
                fromAddress: this.wallet_address,
                toAddress: to_address,
                amount: [{amount: amount, denom: denom}]
            }
        }
        ])
    }

    async execute_contract(msg, contractAddress, coins) {
        let msg_list = []
        if (Array.isArray(msg)) {
            msg.forEach((msg) => {
                msg_list.push(this.get_execute(msg, contractAddress, coins))
            })

        } else {
            msg_list = [
                this.get_execute(msg, contractAddress, coins)
            ]
        }
        console.log("execute_contract is called")
        console.log(JSON.stringify(msg_list))
        let response = await this.sign_and_broadcast(msg_list)
        console.log(response)
        return response
    }

    get_execute(msg, contract, coins) {

        if (typeof coins === "object") {
            coins = [coins]
        } else {
            coins = []
        }

        const executeContractMsg = {
            typeUrl: "/cosmwasm.wasm.v1.MsgExecuteContract",
            value: MsgExecuteContract.fromPartial({
                sender: this.wallet_address,
                contract: contract,
                msg: (0, toUtf8)(JSON.stringify(msg)),
                funds: coins,
            }),
        };
        return executeContractMsg;
    }

    query(address, query) {

        return this.client.queryContractSmart(address, JSON.stringify(query))
    }


    async upload(file) {
        const code = fs.readFileSync(file);
        const uploadReceipt = await this.client.upload(
            this.wallet_address,
            code,
            "auto",
            "Uploading contract",
        );
        console.info(`Upload succeeded. Receipt: ${JSON.stringify(uploadReceipt)}`);
        return uploadReceipt
    }

    async init(code_id, contract_init) {

        const instantiateFee = calculateFee(500, GasPrice.fromString("0.0001ujunox"));
        const {contractAddress} = await this.client.instantiate(
            this.wallet_address,
            code_id,
            contract_init,
            "some_label",
            "auto",
            {
                memo: `Create a instance of contract`,
                admin: this.wallet_address,
            },
        );
        console.info(`Contract instantiated at ${contractAddress}`);
        return contractAddress

    }


    sleep(time) {
        return new Promise((resolve) => setTimeout(resolve, time));
    }

    queryBankUusd(address) {
        let api = "/cosmos/bank/1beta1/balances/";
        return fetch(this.url + api + address).then(response => response.json())
    }


}

//let wallet = new Wallet(mnemonic)
//await wallet.initialize();
// let response = await wallet.upload("../../artifacts/vest_n_distribute.wasm")
// console.log(response)
// wallet.send_funds("juno1gcxq5hzxgwf23paxld5c9z0derc9ac4m5g63xa", {denom: "ujunox", amount: String(100)})

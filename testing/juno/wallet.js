//import message from "@cosmostation/cosmosjs/src/messages/proto.js";
import fs from "fs";
import fetch from "node-fetch";
//import {Cosmos} from "@cosmostation/cosmosjs";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate"
import { DirectSecp256k1HdWallet } from"@cosmjs/proto-signing"
import { calculateFee, GasPrice } from "@cosmjs/stargate"

const debug = false

const chainId = "juno"
//const lcdUrl = "http://localhost:26657"
//const endpoint = "http://localhost:26657";
const endpoint = "https://uni-api.blockpane.com"

//const chainIdTestNet = "uni-3"
//const lcdUrlTestNet = "https://uni-api.blockpane.com"
const testnetMemonic = "patch rookie cupboard salon powder depend grass account crawl raise cigar swim sunny van monster fatal system edge loop matter course muffin rigid ill"
// juno1lm3y9pyznfdmdl8kj3rgj3afkm0xh6p7deh6wc
// Copy Memonic from the Terminal in which the Juno Node contrainer was upped
export const mnemonic = (debug) ? "bind scout grass sport note hero marine float deliver shrimp lunar owner gym mixed march glass swear asthma pass segment grant history flock trend" : testnetMemonic;

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
        /*
	this.privateKey = cosmos.getECPairPriv(memonic);
        this.publicKey = cosmos.getPubKeyAny(this.privateKey);
        this.wallet_address = cosmos.getAddress(memonic);
        this.url = cosmos.url
        this.feeValue = new message.cosmos.tx.v1beta1.Fee({
            amount: [{denom: "ujunox", amount: String(45000000)}],
            gas_limit: 1000000000
        });
	*/
	this.memonic = memonic;
	//this.gasPrice = GasPrice.fromString("0.0025ujunox");
    }

	async initialize() {
    		const wallet = await DirectSecp256k1HdWallet.fromMnemonic(this.memonic, { prefix: "juno" });
	        const account =  await wallet.getAccounts();
        	this.wallet_address = account[0].address;
        	this.client = await SigningCosmWasmClient.connectWithSigner(endpoint, wallet, {gasPrice: GasPrice.fromString("1ujunox")});
  	}

    async sign_and_broadcast(messages) {
	/*
        return cosmos.getAccounts(this.wallet_address).then(async data => {
            let signerInfo = new message.cosmos.tx.v1beta1.SignerInfo({
                public_key: this.publicKey,
                mode_info: {single: {mode: message.cosmos.tx.signing.v1beta1.SignMode.SIGN_MODE_DIRECT}},
                sequence: data.account.sequence
            });
            const txBody = new message.cosmos.tx.v1beta1.TxBody({messages: messages, memo: ""});
            const authInfo = new message.cosmos.tx.v1beta1.AuthInfo({signer_infos: [signerInfo], fee: this.feeValue});
            const signedTxBytes = cosmos.sign(txBody, authInfo, data.account.account_number, this.privateKey);
            return cosmos.broadcast(signedTxBytes, "BROADCAST_MODE_BLOCK")
        })
	*/
	//const fee = calculateFee(1, GasPrice.fromString("0.0001ujunox"));
        const memo = "memo_for_sign_and_broadcast";
	//FIXME: Need to sign and broadcast messgaes
        //return this.client.signAndBroadcast(this.wallet_address, messages, "auto", memo)
	return 
    }

    async send_funds(to_address, coins) {
	/*    
        const msgSend = new message.cosmos.bank.v1beta1.MsgSend({
            from_address: this.wallet_address,
            to_address: to_address,
            amount: [coins]
        });

        return this.sign_and_broadcast([{
            type_url: "/cosmos.bank.v1beta1.MsgSend",
            value: message.cosmos.bank.v1beta1.MsgSend.encode(msgSend).finish()
        }])
	*/
	//const fee = calculateFee(100, GasPrice.fromString("0.0001ujunox"));
  	const memo = "memo_for_send_fund";
  	const sendResult = await this.client.sendTokens(this.wallet_address, to_address, coins, "auto", memo);
	const response = await this.sign_and_broadcast(sendResult)
        console.log(response)
        return response
    }

    async execute_contract(msg, contractAddress, coins) {
        let msg_list = []
        if (Array.isArray(msg)) {
            msg.forEach((msg) => {
                msg_list.push(this.get_execute(msg, contractAddress, coins))
            })

        } else {
            msg_list = [
                this.get_execute(msg, contractAddress)
            ]
        }
        let response = await this.sign_and_broadcast(msg_list)
        console.log(response)
        return response
    }

    get_execute(msg, contract, coins) {
	/*
        let transferBytes = new Buffer.from(JSON.stringify(msg));
        const msgExecuteContract = new message.cosmwasm.wasm.v1.MsgExecuteContract({
            sender: this.wallet_address,
            contract: contract,
            msg: transferBytes,
            funds: coins
        });
        return new message.google.protobuf.Any({
            type_url: "/cosmwasm.wasm.v1.MsgExecuteContract",
            value: message.cosmwasm.wasm.v1.MsgExecuteContract.encode(msgExecuteContract).finish()
        })
	*/
	//const fee = calculateFee(100, GasPrice.fromString("0.0001ujunox"));
	return this.client.execute(this.wallet_address, contract, msg, "auto", "", coins)
    }

    query(address, query) {
	/*
        cosmos.wasmQuery(
            address,
            JSON.stringify(query)
        ).then(json => {
            return json
        })
	*/
	return this.client.queryContractSmart(address, JSON.stringify(query))
    }

    async upload(file) {
	/*
        const code = fs.readFileSync(file).toString("base64");
        const msgStoreCode = new message.cosmwasm.wasm.v1.MsgStoreCode({
            sender: this.wallet_address,
            wasm_byte_code: code,
        });
        let response = await this.sign_and_broadcast([{
            type_url: "/cosmwasm.wasm.v1.MsgStoreCode",
            value: message.cosmwasm.wasm.v1.MsgStoreCode.encode(msgStoreCode).finish()
        }])
        console.log(response)
        console.log(response.tx_response.raw_log)
        let j = JSON.parse(response.tx_response.raw_log)
        return parseInt(j[0].events[1].attributes[0].value)
	*/
	const code = fs.readFileSync(file);
  	//const uploadFee = calculateFee(10000, GasPrice.fromString("0.0001ujunox"));
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
	/*
        let transferBytes = new Buffer.from(JSON.stringify(contract_init));
        const msgInit = new message.cosmwasm.wasm.v1.MsgInstantiateContract({
            sender: this.wallet_address,
            admin: this.wallet_address,
            code_id: parseInt(code_id),
            msg: transferBytes,
            label: "some",
            initFunds: []
        });
        let response = await this.sign_and_broadcast([{
            type_url: "/cosmwasm.wasm.v1.MsgInstantiateContract",
            value: message.cosmwasm.wasm.v1.MsgInstantiateContract.encode(msgInit).finish()
        }])

        let address = Buffer.from(response.tx_response.events[response.tx_response.events.length - 1].attributes[0].value, "base64").toString()
        if (address.includes("juno")) {
            return address
        }
        throw new Error("Error Instantiating the contract, please check the init message and try again...")
	*/
	const instantiateFee = calculateFee(500, GasPrice.fromString("0.0001ujunox"));
        const { contractAddress } = await this.client.instantiate(
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

import {queryContract} from "./utils.js";

let query_resposne = await queryContract("terra1lel90f64uzjnqxrevsmywmq5xdftcxds9sp2q6", {
    game_details: {}
})
console.log(query_resposne)
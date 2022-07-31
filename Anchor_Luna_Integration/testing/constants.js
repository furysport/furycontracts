import dotenv from "dotenv";
dotenv.config();
import { LocalTerra, LCDClient } from "@terra-money/terra.js";
import { get_server_epoch_seconds } from "./utils.js";
import { MnemonicKey } from "@terra-money/terra.js";

export const terraTestnetClient = new LCDClient({
  URL: "https://bombay-lcd.terra.dev",
  chainID: "bombay-12",
});
terraTestnetClient.chainID = "bombay-12";
export const localTerraClient = new LocalTerra();
localTerraClient.chainID = "localterra";

console.log("terraTestnetClient.chainID = " + terraTestnetClient.chainID);
console.log("localTerraClient.chainID = " + localTerraClient.chainID);
export const terraClient =
  process.env.TERRA_CLIENT === "localTerra"
    ? localTerraClient
    : terraTestnetClient;

import { Program, AnchorProvider } from "@coral-xyz/anchor";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { BridgeCards } from "../target/types/bridge_cards";
import * as anchor from "@coral-xyz/anchor";
import fs from "fs";
import path from "path";
import readlineSync from "readline-sync";

const PROGRAM_NAME = "bridge_cards";

const STATE_SEED = Buffer.from("state");

// Loads a keypair from a given file name / path
function loadKeypair(filename: string): Keypair {
  const filePath = path.resolve(filename);
  if (!fs.existsSync(filePath)) {
    throw new Error(`Keypair file not found at path: ${filePath}`);
  }
  const secretKeyString = fs.readFileSync(filePath, { encoding: "utf8" });
  const secretKey = Uint8Array.from(JSON.parse(secretKeyString));
  return Keypair.fromSecretKey(secretKey);
}

// Finds the state PDA
// Returns [PDA, bump]
function findStatePDA(programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([STATE_SEED], programId);
}

// Loads the program ID from Anchor.toml
function getProgramId(): PublicKey {
  const configFile = fs.readFileSync("Anchor.toml", "utf8");
  const matches = configFile.match(`${PROGRAM_NAME} = "([^"]+)"`);
  if (!matches) {
    throw new Error("Could not find program ID in Anchor.toml");
  }

  return new PublicKey(matches[1]);
}

// Loads the program keypair from the given build directory.
// Requires `anchor build` to have been run first.
function getProgramKeypair(programId: PublicKey): Keypair {
  const programKeypairFile = path.join(
    "target",
    "deploy",
    `${PROGRAM_NAME}-keypair.json`,
  );
  let programKeypair: Keypair;
  try {
    programKeypair = loadKeypair(programKeypairFile);
    console.log(`Loaded program keypair from ${programKeypairFile}`);
    console.log(
      `Program Keypair Public Key: ${programKeypair.publicKey.toBase58()}`,
    );

    if (programKeypair.publicKey.toBase58() !== programId.toBase58()) {
      console.warn(
        `WARNING: Loaded program keypair public key (${programKeypair.publicKey.toBase58()}) does not match the program ID from Anchor.toml (${programId.toBase58()}). Ensure you are using the correct keypair for the deployed program.`,
      );
    }
  } catch (error) {
    console.error(
      `Failed to load program keypair from ${programKeypairFile}:`,
      error,
    );
    console.error(
      "Please ensure you have run 'anchor build' and the keypair file exists.",
    );
    process.exit(1);
  }

  return programKeypair;
}

// Loads the IDL from the target/idl directory
function loadIdl(): any {
  const idlPath = path.join("target", "idl", `${PROGRAM_NAME}.json`);
  const idlJson = fs.readFileSync(idlPath, "utf8");
  return JSON.parse(idlJson);
}

async function main() {
  const provider = AnchorProvider.env();
  anchor.setProvider(provider);

  try {
    // Get program ID from Anchor.toml
    const programId = getProgramId();
    console.log("Program ID:", programId.toString());

    const idl = loadIdl();

    const program = new Program<BridgeCards>(idl, provider);

    const [statePda, stateBump] = findStatePDA(programId);

    console.log("Bridge cards state PDA:", statePda.toString());
    console.log("State bump:", stateBump);

    const programKeypair = getProgramKeypair(programId);

    console.log("--- Transaction Details ---");
    console.log("Instruction: initialize");
    console.log(`Program ID: ${programId.toBase58()}`);
    console.log(`Payer (Admin): ${provider.wallet.publicKey.toBase58()}`);
    console.log(`State PDA: ${statePda.toBase58()} (Bump: ${stateBump})`);
    console.log(
      `Program Account (Signer): ${programKeypair.publicKey.toBase58()}`,
    );
    console.log(`System Program: ${SystemProgram.programId.toBase58()}`);
    console.log("--------------------------");

    console.log("!! NOTE: Using your wallet as the admin account !!");
    console.log("Make sure to keep your wallet's keypair secure!");

    const answer = readlineSync.question("Proceed with initialization? (y/N) ");

    if (answer.toLowerCase() !== "y") {
      console.log("Initialization cancelled by user.");
      process.exit(0);
    }

    console.log("Initializing program...");

    const tx = await program.methods
      .initialize()
      .accounts({
        // the payer becomes the admin account
        payer: provider.wallet.publicKey,
        // @ts-ignore - state is handled correctly by anchor based on struct def
        state: statePda,
        programAccount: programKeypair.publicKey,
        systemProgram: SystemProgram.programId,
      })
      // Add the program keypair as a required signer
      .signers([programKeypair])
      .rpc();

    console.log("Program initialized successfully!");
    console.log("Transaction signature:", tx);
  } catch (error) {
    console.error("Initialization failed:", error);
    // Log specific error information if available
    if (error instanceof Error) {
      console.error("Error details:", error.message);
      console.error("Stack trace:", error.stack);
    }
    process.exit(1);
  }
}

main();

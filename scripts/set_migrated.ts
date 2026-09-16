import { Program, AnchorProvider } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram } from "@solana/web3.js";
import { BridgeCards } from "../target/types/bridge_cards";
import * as anchor from "@coral-xyz/anchor";
import fs from "fs";
import path from "path";
import readlineSync from "readline-sync";

const PROGRAM_NAME = "bridge_cards";

const STATE_SEED = Buffer.from("state");
const MIGRATION_STATE_SEED = Buffer.from("migration");

// Finds the state PDA
function findStatePDA(programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([STATE_SEED], programId);
}

// Finds the migration state PDA
function findMigrationStatePDA(programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([MIGRATION_STATE_SEED], programId);
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

// Loads the IDL from the target/idl directory
function loadIdl(): any {
  const idlPath = path.join("target", "idl", `${PROGRAM_NAME}.json`);
  const idlJson = fs.readFileSync(idlPath, "utf8");
  return JSON.parse(idlJson);
}

async function main() {
  const args = process.argv.slice(2);
  if (args.length !== 1 || (args[0] !== "true" && args[0] !== "false")) {
    console.error("Usage: ts-node scripts/set_migrated.ts <true|false>");
    console.error(
      "  true  – marks the program as migrated (disables all instructions except cpi_transfer)",
    );
    console.error(
      "  false – clears the migrated flag (re-enables normal operation)",
    );
    process.exit(1);
  }

  const migrated = args[0] === "true";

  const provider = AnchorProvider.env();
  anchor.setProvider(provider);

  try {
    const programId = getProgramId();
    console.log("Program ID:", programId.toString());

    const idl = loadIdl();
    const program = new Program<BridgeCards>(idl, provider);

    const [statePda] = findStatePDA(programId);
    const [migrationStatePda, migrationStateBump] =
      findMigrationStatePDA(programId);

    console.log("Bridge cards state PDA:     ", statePda.toString());
    console.log("Migration state PDA:        ", migrationStatePda.toString());
    console.log("Migration state PDA bump:   ", migrationStateBump);

    // Fetch the on-chain state to confirm admin
    const onchainState = await program.account.bridgeCardsState.fetch(statePda);
    console.log("Current admin:              ", onchainState.admin.toString());

    console.log("\n--- Transaction Details ---");
    console.log(`Instruction: setMigrated(${migrated})`);
    console.log(`Program ID:         ${programId.toBase58()}`);
    console.log(`Admin (wallet):     ${provider.wallet.publicKey.toBase58()}`);
    console.log(`Payer (wallet):     ${provider.wallet.publicKey.toBase58()}`);
    console.log(`State PDA:          ${statePda.toBase58()}`);
    console.log(`Migration state PDA:${migrationStatePda.toBase58()}`);
    console.log(`System Program:     ${SystemProgram.programId.toBase58()}`);
    console.log(`migrated arg:       ${migrated}`);
    console.log("--------------------------\n");

    if (onchainState.admin.toString() !== provider.wallet.publicKey.toString()) {
      console.error(
        `ERROR: Your wallet (${provider.wallet.publicKey.toBase58()}) is not the program admin (${onchainState.admin.toBase58()}).`,
      );
      console.error("Only the admin can call set_migrated.");
      process.exit(1);
    }

    const action = migrated
      ? "mark the program as MIGRATED (disables normal operation)"
      : "CLEAR the migrated flag (re-enables normal operation)";

    const answer = readlineSync.question(
      `Proceed to ${action}? (y/N) `,
    );

    if (answer.toLowerCase() !== "y") {
      console.log("Cancelled by user.");
      process.exit(0);
    }

    console.log("Sending transaction...");

    const tx = await program.methods
      .setMigrated(migrated)
      .accounts({
        admin: provider.wallet.publicKey,
        payer: provider.wallet.publicKey,
        // @ts-ignore - state is handled correctly by anchor based on struct def
        state: statePda,
        migrationState: migrationStatePda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log(`set_migrated(${migrated}) succeeded!`);
    console.log("Transaction signature:", tx);
  } catch (error) {
    console.error("set_migrated failed:", error);
    if (error instanceof Error) {
      console.error("Error details:", error.message);
      console.error("Stack trace:", error.stack);
    }
    process.exit(1);
  }
}

main();

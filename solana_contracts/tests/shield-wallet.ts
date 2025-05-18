import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ShieldWalletSolana } from "../target/types/shield_wallet_solana";
import { PublicKey, SystemProgram, LAMPORTS_PER_SOL, AccountMeta } from "@solana/web3.js";
import { assert } from "chai";
import { BN } from "bn.js";

describe("shield-wallet-solana", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.ShieldWalletSolana as Program<ShieldWalletSolana>;

  // Test accounts
  const owner1 = anchor.web3.Keypair.generate();
  const owner2 = anchor.web3.Keypair.generate();
  const owner3 = anchor.web3.Keypair.generate();
  let walletConfigPda: PublicKey;
  let programAuthorityPda: PublicKey;

  before(async () => {
    // Airdrop SOL to test accounts
    for (const account of [owner1, owner2, owner3]) {
      const signature = await provider.connection.requestAirdrop(
        account.publicKey,
        2 * LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(signature);
    }

    // Find PDAs
    [programAuthorityPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("program_authority")],
      program.programId
    );

    [walletConfigPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("wallet"),
        owner1.publicKey.toBuffer(),
      ],
      program.programId,
    );

    // Initialize the program
    await program.methods
      .initialize()
      .accounts({
        authority: provider.wallet.publicKey,
        programAuthority: programAuthorityPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    await program.methods
        .initialize()
        .accounts({
          authority: provider.wallet.publicKey,
          programAuthority: programAuthorityPda,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      // Verify initialization
      const authorityAccount = await program.account.programAuthority.fetch(
        programAuthorityPda
      );
      assert.ok(authorityAccount.bump > 0, "Program authority not initialized properly");
    } catch (error) {
      console.error("Error initializing program:", error);
      throw error;
    }
  });

  it("Creates a new Shield Wallet", async () => {
    try {
      const owners = [owner1.publicKey, owner2.publicKey, owner3.publicKey];
      const managementThreshold = 2;
      const executionThreshold = 2;
      const revocationThreshold = 2;
      const timelockDuration = 3600; // 1 hour in seconds

      await program.methods
        .createWallet(
          owners,
          managementThreshold,
          executionThreshold,
          revocationThreshold,
          new anchor.BN(timelockDuration)
        )
        .accounts({
          payer: provider.wallet.publicKey,
          walletConfig: walletConfigPda,
          programAuthority: programAuthorityPda,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      // Verify wallet creation
      const walletConfig = await program.account.walletConfig.fetch(walletConfigPda);
      assert.equal(walletConfig.owners.length, 3, "Incorrect number of owners");
      assert.equal(walletConfig.managementThreshold, 2, "Incorrect management threshold");
      assert.equal(walletConfig.executionThreshold, 2, "Incorrect execution threshold");
      assert.equal(walletConfig.revocationThreshold, 2, "Incorrect revocation threshold");
      assert.equal(walletConfig.timelockDuration.toNumber(), 3600, "Incorrect timelock duration");
    } catch (error) {
      console.error("Error creating wallet:", error);
      throw error;
    }
  });

  it("Proposes and approves a transaction", async () => {
    try {
      // Create a real transaction - a SOL transfer
      const recipient = anchor.web3.Keypair.generate().publicKey;
      const transferAmount = new BN(0.01 * LAMPORTS_PER_SOL);
      
      const mockCall = {
        programId: SystemProgram.programId,
        accounts: [
          { pubkey: recipient, isSigner: false, isWritable: true },
        ],
        data: anchor.web3.SystemProgram.transfer({
          fromPubkey: walletConfigPda,
          toPubkey: recipient,
          lamports: transferAmount.toNumber(),
        }).data,
        value: transferAmount,
      };

      // Generate a transaction ID
      const txId = anchor.web3.Keypair.generate();
      const [transactionPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("transaction"),
          walletConfigPda.toBuffer(),
          txId.publicKey.toBuffer(),
        ],
        program.programId
      );

      // 1. Propose transaction
      await program.methods
        .proposeTransaction(mockCall)
        .accounts({
          proposer: owner1.publicKey,
          walletConfig: walletConfigPda,
          transaction: transactionPda,
          txId: txId.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner1])
        .rpc();
      // Verify transaction proposal
      let transaction = await program.account.transaction.fetch(transactionPda);
      assert.equal(transaction.approvals.length, 1, "Transaction should have 1 approval");
      assert.deepInclude(transaction.approvals, owner1.publicKey, "Owner 1 should have approved");

      // Fetch the wallet config to get the owners
      const walletConfig = await program.account.walletConfig.fetch(walletConfigPda);
      assert.deepInclude(walletConfig.owners, owner2.publicKey, "Owner 2 should be an owner");
      // Second owner approves
      await program.methods
        .approveTransaction()
        .accounts({
          approver: owner2.publicKey,
          walletConfig: walletConfigPda,
          transaction: transactionPda,
        })
        .signers([owner2])
        .rpc();

      // Verify approvals
      transaction = await program.account.transaction.fetch(transactionPda);
      assert.equal(transaction.approvals.length, 2, "Transaction should have 2 approvals");
      assert.deepInclude(transaction.approvals, owner2.publicKey, "Owner 2 should have approved");

      // 2. Execute the transaction
      // Add a whitelisted target (for SystemProgram in this case)
      const target = {
        programId: SystemProgram.programId,
        instructionDiscriminator: null,
        maxValue: new anchor.BN(LAMPORTS_PER_SOL),
      };
      const [signaturesPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("signatures"), walletConfigPda.toBuffer()],
        program.programId
      );
      await program.methods
        .addWhitelistedTarget(target)
        .accounts({
          manager: owner1.publicKey,
          walletConfig: walletConfigPda,
          signatures: signaturesPda,
        })
        .signers([owner1])
        .rpc();
      // Wait for timelock (simulated - for real test, use clock mocking or wait)
      console.log("Simulating timelock wait...");
      // Execute the transaction
      await program.methods
        .executeTransaction()
        .accounts({
          executor: provider.wallet.publicKey,
          walletConfig: walletConfigPda,
        })
    } catch (error) {
      console.error("Error in transaction proposal and approval:", error);
      throw error;
    }
  });

  it("Enforces timelock before execution", async () => {
    // TODO: Add test for timelock enforcement
    // This would require either:
    // 1. Mocking the Solana clock (in a local validator)
    // 2. Actually waiting for the timelock period
    // For hackathon purposes, we can skip this test or implement a shorter timelock
  });

  it("Whitelists a target and executes a transaction", async () => {
    try {
      // Add a whitelisted target for the System Program (for SOL transfers)
      const target = {
        programId: SystemProgram.programId,
        instructionDiscriminator: null,
        maxValue: new anchor.BN(LAMPORTS_PER_SOL),
      };

      // Create a signatures PDA (in real implementation, this would be handled differently)
      const [signaturesPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("signatures"), walletConfigPda.toBuffer()],
        program.programId
      );

      await program.methods
        .addWhitelistedTarget(target)
        .accounts({
          manager: owner1.publicKey,
          walletConfig: walletConfigPda,
          signatures: signaturesPda,
        })
        .signers([owner1])
        .rpc();

      // Verify whitelisted target
      const walletConfig = await program.account.walletConfig.fetch(walletConfigPda);
      assert.equal(walletConfig.whitelistedTargets.length, 1, "Target not whitelisted");

      // Create a transaction to transfer SOL
      const recipient = anchor.web3.Keypair.generate().publicKey;
      const transferAmount = new BN(0.01 * LAMPORTS_PER_SOL);
      
      // Get initial balances
      const initialRecipientBalance = await provider.connection.getBalance(recipient);
      
      // Create transaction ID
      const txId = anchor.web3.Keypair.generate();
      const [transactionPda] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("transaction"),
          walletConfigPda.toBuffer(),
          txId.publicKey.toBuffer(),
        ],
        program.programId
      );
      
      // Create the transaction
      const transferCall = {
        programId: SystemProgram.programId,
        accounts: [
          { pubkey: recipient, isSigner: false, isWritable: true },
        ],
        data: anchor.web3.SystemProgram.transfer({
          fromPubkey: walletConfigPda,
          toPubkey: recipient,
          lamports: transferAmount.toNumber(),
        }).data,
        value: transferAmount,
      };
      
      // Propose the transaction
      await program.methods
        .proposeTransaction(transferCall)
        .accounts({
          proposer: owner1.publicKey,
          walletConfig: walletConfigPda,
          transaction: transactionPda,
          txId: txId.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner1, txId])
        .rpc();
      
      // Get the other owners to approve
      await program.methods
        .approveTransaction()
        .accounts({
          approver: owner2.publicKey,
          walletConfig: walletConfigPda,
          transaction: transactionPda,
        })
        .signers([owner2])
        .rpc();
      
      // Simulate timelock passage (in real implementation, we'd wait)
      // For testing, we could modify the blockchain clock, but we'll skip that for now
      
      // Execute the transaction
      await program.methods
        .executeTransaction()
        .accounts({
          executor: provider.wallet.publicKey,
          walletConfig: walletConfigPda,
          transaction: transactionPda,
          systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
          { pubkey: recipient, isSigner: false, isWritable: true },
        ])
        .rpc();
      
      // Verify the transfer happened
      const finalRecipientBalance = await provider.connection.getBalance(recipient);
      assert.isAbove(
        finalRecipientBalance, 
        initialRecipientBalance, 
        "Recipient balance should have increased"
      );
    } catch (error) {
      console.error("Error in whitelist and execution test:", error);
      throw error;
    }
  });

  it("Changes threshold values", async () => {
    try {
      const [signaturesPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("signatures"), walletConfigPda.toBuffer()],
        program.programId
      );
      
      // Change execution threshold
      await program.methods
        .changeThreshold({ execution: {} }, 1) // Lower to 1 signature
        .accounts({
          manager: owner1.publicKey,
          walletConfig: walletConfigPda,
          signatures: signaturesPda,
        })
        .signers([owner1])
        .rpc();
      
      // Verify threshold change
      const walletConfig = await program.account.walletConfig.fetch(walletConfigPda);
      assert.equal(walletConfig.executionThreshold, 1, "Execution threshold should be updated");
    } catch (error) {
      console.error("Error changing threshold:", error);
      throw error;
    }
  });
  
  it("Adds a new owner", async () => {
    try {
      const newOwner = anchor.web3.Keypair.generate().publicKey;
      const [signaturesPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("signatures"), walletConfigPda.toBuffer()],
        program.programId
      );
      
      // Add the new owner
      await program.methods
        .addOwner(newOwner)
        .accounts({
          manager: owner1.publicKey,
          walletConfig: walletConfigPda,
          signatures: signaturesPda,
        })
        .signers([owner1])
        .rpc();
      
      // Verify owner addition
      const walletConfig = await program.account.walletConfig.fetch(walletConfigPda);
      assert.include(
        walletConfig.owners.map(key => key.toString()), 
        newOwner.toString(), 
        "New owner should be added"
      );
    } catch (error) {
      console.error("Error adding owner:", error);
      throw error;
    }
  });
  
  // Additional test ideas:
  // - Test transaction cancellation
  // - Test invalid operations (should fail)
  // - Test edge cases in approval counting
});

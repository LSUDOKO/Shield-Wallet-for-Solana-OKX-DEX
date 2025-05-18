use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_lang::solana_program::program::invoke_signed;
use std::collections::BTreeMap;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS"); // Replace with your program ID after deployment

// Error codes for the Shield Wallet program
#[error_code]
pub enum ShieldWalletError {
    #[msg("Transaction already exists")]
    TransactionAlreadyExists,
    #[msg("Transaction does not exist")]
    TransactionDoesNotExist,
    #[msg("Timelock period not passed")]
    TimelockNotPassed,
    #[msg("Insufficient approvals")]
    InsufficientApprovals,
    #[msg("Target not whitelisted")]
    TargetNotWhitelisted,
    #[msg("Not an owner")]
    NotAnOwner,
    #[msg("Invalid threshold type")]
    InvalidThresholdType,
    #[msg("Invalid threshold value")]
    InvalidThresholdValue,
    #[msg("Signer already approved")]
    SignerAlreadyApproved,
    #[msg("Maximum value exceeded")]
    MaxValueExceeded,
    #[msg("Operation not authorized")]
    NotAuthorized,
    #[msg("Invalid signature")]
    InvalidSignature,
    #[msg("Execution failed")]
    ExecutionFailed,
}

// Events
#[event]
pub struct TransactionExecuted {
    pub tx_id: [u8; 32],
    pub executor: Pubkey,
    pub timestamp: i64,
}

// Constants for the program
const MAX_OWNERS: usize = 10; 
const MAX_WHITELISTED_TARGETS: usize = 20;
const MAX_DATA_SIZE: usize = 1024;

// Enum for threshold types, mirroring the original Ethereum contract
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdType {
    Management,
    Execution,
    Revocation,
}

// Single call struct, similar to the Call struct in the Ethereum contract
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Call {
    pub program_id: Pubkey,
    pub accounts: Vec<AccountMeta>,
    pub data: Vec<u8>,
    pub value: u64,  // SOL amount in lamports
}

#[program]
pub mod shield_wallet_solana {
    use super::*;
    
    /// Initialize the program with a program authority
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Shield Wallet Solana Program Initialized!");
        ctx.accounts.program_authority.bump = *ctx.bumps.get("program_authority").unwrap();
        Ok(())
    }

    /// Create a new Shield Wallet instance
    pub fn create_wallet(
        ctx: Context<CreateWallet>, 
        owners: Vec<Pubkey>,
        management_threshold: u8,
        execution_threshold: u8,
        revocation_threshold: u8,
        timelock_duration: i64
    ) -> Result<()> {
        require!(
            management_threshold <= owners.len() as u8 && management_threshold > 0,
            ShieldWalletError::InvalidThresholdValue
        );
        require!(
            execution_threshold <= owners.len() as u8 && execution_threshold > 0,
            ShieldWalletError::InvalidThresholdValue
        );
        require!(
            revocation_threshold <= owners.len() as u8 && revocation_threshold > 0,
            ShieldWalletError::InvalidThresholdValue
        );
        
        let wallet_config = &mut ctx.accounts.wallet_config;
        wallet_config.owners = owners;
        wallet_config.management_threshold = management_threshold;
        wallet_config.execution_threshold = execution_threshold;
        wallet_config.revocation_threshold = revocation_threshold;
        wallet_config.whitelisted_targets = Vec::new();
        wallet_config.timelock_duration = timelock_duration;
        wallet_config.bump = *ctx.bumps.get("wallet_config").unwrap();
        
        Ok(())
    }

    /// Propose a transaction to be executed by the wallet
    pub fn propose_transaction(
        ctx: Context<ProposeTransaction>,
        call: Call,
    ) -> Result<()> {
        let wallet_config = &ctx.accounts.wallet_config;
        let tx = &mut ctx.accounts.transaction;
        let tx_id = &ctx.accounts.tx_id;
        
        // Store the transaction details
        tx.tx_id = tx_id.id;
        tx.call = call;
        tx.approvals = vec![ctx.accounts.proposer.key()]; // Proposer automatically approves
        tx.proposed_at = Clock::get()?.unix_timestamp;
        tx.bump = *ctx.bumps.get("transaction").unwrap();
        
        Ok(())
    }

    /// Approve a proposed transaction
    pub fn approve_transaction(ctx: Context<ApproveTransaction>) -> Result<()> {
        let transaction = &mut ctx.accounts.transaction;
        let approver_key = ctx.accounts.approver.key();
        
        // Check if the signer already approved
        require!(
            !transaction.approvals.contains(&approver_key),
            ShieldWalletError::SignerAlreadyApproved
        );
        
        // Add the approval
        transaction.approvals.push(approver_key);
        
        Ok(())
    }

    /// Execute a proposed transaction if conditions are met
    pub fn execute_transaction(ctx: Context<ExecuteTransaction>) -> Result<()> {
        let wallet_config = &ctx.accounts.wallet_config;
        let transaction = &mut ctx.accounts.transaction;
        
        // Check if the transaction has enough approvals
        require!(
            transaction.approvals.len() >= wallet_config.execution_threshold as usize,
            ShieldWalletError::InsufficientApprovals
        );
        
        // Check timelock
        let current_time = Clock::get()?.unix_timestamp;
        let required_time = transaction.proposed_at + wallet_config.timelock_duration;
        require!(
            current_time >= required_time,
            ShieldWalletError::TimelockNotPassed
        );
        
        // Check if the target is whitelisted
        let call = &transaction.call;
        let target_info = wallet_config.whitelisted_targets.iter().find(|target| {
            target.program_id == call.program_id && 
            (target.instruction_discriminator.is_none() || 
             (call.data.len() >= 8 && 
              target.instruction_discriminator.unwrap() == call.data[0..8].try_into().unwrap()))
        });
        
        require!(target_info.is_some(), ShieldWalletError::TargetNotWhitelisted);
        
        // Check value limits
        require!(
            call.value <= target_info.unwrap().max_value,
            ShieldWalletError::MaxValueExceeded
        );
        
        // Prepare the instruction for CPI
        let ix = Instruction {
            program_id: call.program_id,
            accounts: call.accounts.clone(),
            data: call.data.clone(),
        };
        
        // Get the PDA signer seeds
        let wallet_seeds = &[
            b"wallet".as_ref(),
            wallet_config.owners[0].as_ref(),
            &[wallet_config.bump],
        ];
        
        // If transferring SOL, handle that first
        if call.value > 0 {
            // Create a transfer instruction
            let transfer_ix = anchor_lang::solana_program::system_instruction::transfer(
                &ctx.accounts.wallet_config.to_account_info().key(),
                &call.accounts[0].pubkey, // Assuming first account is recipient
                call.value
            );
            
            // Execute the transfer
            invoke_signed(
                &transfer_ix,
                &[
                    ctx.accounts.wallet_config.to_account_info(),
                    ctx.remaining_accounts[0].clone(),
                    ctx.accounts.system_program.to_account_info(),
                ],
                &[wallet_seeds]
            )?;
        }
        
        // Execute the main instruction with the PDA as a signer
        invoke_signed(
            &ix,
            ctx.remaining_accounts,
            &[wallet_seeds]
        )?;
        
        // Mark transaction as executed by removing it from storage
        // In a production system, you might want to keep a record of executed transactions
        
        // Emit an event for the executed transaction
        emit!(TransactionExecuted {
            tx_id: transaction.tx_id,
            executor: ctx.accounts.executor.key(),
            timestamp: current_time
        });
        
        Ok(())
    }
    
    /// Add a whitelisted target for the wallet
    pub fn add_whitelisted_target(
        ctx: Context<ManageWallet>,
        target: WhitelistedTarget,
    ) -> Result<()> {
        let wallet_config = &mut ctx.accounts.wallet_config;
        
        // Check if the caller has sufficient approval
        require!(
            ctx.accounts.signatures.len() >= wallet_config.management_threshold as usize,
            ShieldWalletError::InsufficientApprovals
        );
        
        // Add the target
        wallet_config.whitelisted_targets.push(target);
        
        Ok(())
    }
    
    /// Add a new owner to the wallet
    pub fn add_owner(
        ctx: Context<ManageWallet>,
        new_owner: Pubkey,
    ) -> Result<()> {
        let wallet_config = &mut ctx.accounts.wallet_config;
        
        // Check if the caller has sufficient approval
        require!(
            ctx.accounts.signatures.len() >= wallet_config.management_threshold as usize,
            ShieldWalletError::InsufficientApprovals
        );
        
        // Check if owner already exists
        require!(
            !wallet_config.owners.contains(&new_owner),
            ShieldWalletError::NotAuthorized
        );
        
        // Add the owner
        wallet_config.owners.push(new_owner);
        
        Ok(())
    }
    
    /// Update a threshold value
    pub fn change_threshold(
        ctx: Context<ManageWallet>,
        threshold_type: ThresholdType,
        new_threshold: u8,
    ) -> Result<()> {
        let wallet_config = &mut ctx.accounts.wallet_config;
        
        // Check if the caller has sufficient approval
        require!(
            ctx.accounts.signatures.len() >= wallet_config.management_threshold as usize,
            ShieldWalletError::InsufficientApprovals
        );
        
        // Check if the new threshold is valid
        require!(
            new_threshold <= wallet_config.owners.len() as u8 && new_threshold > 0,
            ShieldWalletError::InvalidThresholdValue
        );
        
        // Update the appropriate threshold
        match threshold_type {
            ThresholdType::Management => wallet_config.management_threshold = new_threshold,
            ThresholdType::Execution => wallet_config.execution_threshold = new_threshold,
            ThresholdType::Revocation => wallet_config.revocation_threshold = new_threshold,
        }
        
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + 8 // Discriminator + bump
    )]
    pub program_authority: Account<'info, ProgramAuthority>,
    pub system_program: Program<'info, System>,
}

// Account contexts for the wallet creation
#[derive(Accounts)]
pub struct CreateWallet<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    
    // Wallet configuration PDA
    #[account(
        init,
        payer = payer,
        space = 8 + // Discriminator
        4 + (MAX_OWNERS * 32) + // Owners vector (4 bytes for len + pubkeys)
        1 + 1 + 1 + // Thresholds (3 u8 values)
        4 + (MAX_WHITELISTED_TARGETS * (32 + 8 + 8)) + // Whitelisted targets
        8, // Timelock duration
        seeds = [
            b"wallet".as_ref(),
            payer.key().as_ref(),
            &[bump]
        ],
        bump
    )]
    pub wallet_config: Account<'info, WalletConfig>,
    pub system_program: Program<'info, System>,
    #[account(
        mut,
        seeds = [b"program_authority".as_ref()],
        bump = program_authority.bump
    )]
    pub program_authority: Account<'info, ProgramAuthority>,
}

// Account contexts for proposing a meta transaction
#[derive(Accounts)]
pub struct ProposeTransaction<'info> {
    #[account(mut)]
    pub proposer: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"wallet".as_ref(), wallet_config.owners[0].key().as_ref(), wallet_config.bump.to_le_bytes().as_ref()],
        bump = wallet_config.bump,
        constraint = wallet_config.is_owner(proposer.key()) @ ShieldWalletError::NotAnOwner
    )]
    pub wallet_config: Account<'info, WalletConfig>,
    
    #[account(
        init,
        payer = proposer,
        space = 8 + // Discriminator
        4 + (4 + MAX_DATA_SIZE) + // Call data (simplified)
        4 + 32 * MAX_OWNERS + // Approvals (vec of pubkeys)
        8 + // Proposed timestamp
        1, // Bump
        seeds = [
            b"transaction".as_ref(),
            wallet_config.key().as_ref(),
            tx_id.as_ref()
        ],
        bump
    )]
    pub transaction: Account<'info, Transaction>,
    pub tx_id: Account<'info, TransactionId>, 
    pub system_program: Program<'info, System>,
}

// Account context for approving a transaction
#[derive(Accounts)]
pub struct ApproveTransaction<'info> {
    #[account(mut)]
    pub approver: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"wallet".as_ref(), wallet_config.owners[0].key().as_ref(), wallet_config.bump.to_le_bytes().as_ref()],
        bump = wallet_config.bump,
        constraint = wallet_config.is_owner(approver.key()) @ ShieldWalletError::NotAnOwner
    )]
    pub wallet_config: Account<'info, WalletConfig>,
    
    #[account(
        mut,
        seeds = [
            b"transaction".as_ref(), 
            wallet_config.key().as_ref(),
            transaction.tx_id.as_ref()
        ],
        bump = transaction.bump
    )]
    pub transaction: Account<'info, Transaction>,
}

// Account context for executing a transaction
#[derive(Accounts)]
pub struct ExecuteTransaction<'info> {
    #[account(mut)]
    pub executor: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"wallet".as_ref(), wallet_config.owners[0].key().as_ref(), wallet_config.bump.to_le_bytes().as_ref()],
        bump = wallet_config.bump
    )]
    pub wallet_config: Account<'info, WalletConfig>,
    
    #[account(
        mut,
        seeds = [
            b"transaction".as_ref(), 
            wallet_config.key().as_ref(),
            transaction.tx_id.as_ref()
        ],
        bump = transaction.bump
    )]
    pub transaction: Account<'info, Transaction>,
    
    // We'll need to pass in remaining accounts dynamically
    // based on the transaction we're executing
    pub system_program: Program<'info, System>,
}

// Account struct for managing program authority
#[account]
pub struct ProgramAuthority {
    pub bump: u8,
}

// Account struct for wallet configuration
#[account]
pub struct WalletConfig {
    pub owners: Vec<Pubkey>,
    pub management_threshold: u8,
    pub execution_threshold: u8,
    pub revocation_threshold: u8,
    pub whitelisted_targets: Vec<WhitelistedTarget>,
    pub timelock_duration: i64, // In seconds
    pub bump: u8,
}

impl WalletConfig {
    // Helper method to check if a pubkey is an owner
    pub fn is_owner(&self, key: Pubkey) -> bool {
        self.owners.contains(&key)
    }
}

// Account context for managing wallet configuration (adding owners, changing thresholds, etc.)
#[derive(Accounts)]
pub struct ManageWallet<'info> {
    #[account(mut)]
    pub manager: Signer<'info>,
    
    #[account(
        mut,
        seeds = [b"wallet".as_ref(), wallet_config.owners[0].key().as_ref(), wallet_config.bump.to_le_bytes().as_ref()],
        bump = wallet_config.bump
    )]
    pub wallet_config: Account<'info, WalletConfig>,
    
    // This represents the signatures collected off-chain for management actions
    // In a real implementation, you'd have a more robust mechanism
    pub signatures: UncheckedAccount<'info>,
}

// Structure for whitelisted targets
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct WhitelistedTarget {
    pub program_id: Pubkey,
    pub instruction_discriminator: Option<[u8; 8]>,
    pub max_value: u64,
}

// Transaction identifier - just a simple struct to hold a unique ID
#[account]
pub struct TransactionId {
    pub id: [u8; 32],
}

// Account struct for meta transactions
#[account]
pub struct Transaction {
    pub tx_id: [u8; 32],
    pub call: Call,
    pub approvals: Vec<Pubkey>,
    pub proposed_at: i64,
    pub bump: u8,
}

// --- NOTES ON PORTING FROM EVM ---
// - Solidity's `msg.sender` equivalent is checking signers in the Context.
// - Contract storage in Ethereum maps to data stored in Solana accounts (often PDAs).
// - Modifiers can be implemented as checks at the beginning of an instruction handler.
// - Events are emitted using `emit!(MyEvent {...})`.
// - Error handling is done via `Result<()>` and `err!(MyError::SomeError)`.
// - `ecrecover` for signature verification needs to be handled carefully.
//   Solana programs typically expect signatures to be verified by the runtime before instruction execution (via Signer type)
//   or use `solana_program::secp256k1_recover` for Ethereum-style signatures if absolutely needed,
//   but native Solana Ed25519 signatures are preferred. For multi-sig, each owner would sign the transaction
//   that calls `approve_meta_transaction`.

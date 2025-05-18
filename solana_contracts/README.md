# Shield Wallet - Solana Implementation for OKX Solana Accelerate Hackathon

This directory contains the Anchor-based smart contracts for the Solana version of Shield Wallet, optimized for integration with the OKX DEX API.

## Overview

We're rebuilding the core security features of Shield Wallet on the Solana blockchain to leverage Solana's high-throughput, low-cost transactions (< $0.01 per transaction) - essential for high-capital management operations. This includes:

- **Meta-transactions:** Allowing users to propose transactions that are then approved and executed by the wallet.
- **On-chain Timelocks:** Ensuring a delay between transaction proposal/approval and execution, with all data stored on-chain.
- **Threshold-based Roles:** Different thresholds for management, execution, and revocation operations.
- **Whitelisting:** Restricting the wallet to interact only with pre-approved programs and instructions.

## Architecture on Solana

- **Anchor Framework:** Smart contract development in Rust instead of Solidity.
- **Program-Derived Addresses (PDAs):** Store wallet configurations, meta-transaction details, and whitelists.
- **Account Model:** Leverages Solana's account model instead of Ethereum's storage model.
- **Client-Side Integration:** Front-end uses `@solana/web3.js` and OKX DEX API.

## OKX DEX API Integration

Shield Wallet on Solana is designed specifically to integrate with the OKX DEX API for:

1. **Secure Trading Operations:**
   - Multi-signature approvals and timelock protection for trades executed via OKX DEX.
   - Whitelisting of specific trading pairs and maximum trade values.

2. **Risk Management:**
   - Query OKX DEX for real-time pricing and liquidity data before executing high-value transactions.
   - Allow configurable thresholds based on asset volatility data from OKX.

3. **Portfolio Management:**
   - Secure management of digital assets traded on OKX DEX.
   - Privileged operations for rebalancing and asset allocation.

## Implementation Features

- **Solana Program (`lib.rs`):**
  - Complete implementation of Shield Wallet core functionality in Anchor/Rust.
  - Transaction proposal and approval workflows.
  - Timelock enforcement.
  - Threshold-based permission system.
  - Whitelisting mechanism for authorized targets.

- **Enhanced Security for OKX Users:**
  - Native protection against common crypto security issues.
  - Perfect for DAOs, investment groups, and treasury management using OKX DEX.

## Getting Started

1. Install Solana and Anchor:
   ```bash
   sh -c "$(curl -sSfL https://release.solana.com/v1.16.0/install)"
   npm install -g @coral-xyz/anchor
   ```

2. Build the program:
   ```bash
   cd solana_contracts
   anchor build
   ```

3. Deploy to Solana devnet:
   ```bash
   solana config set --url devnet
   anchor deploy
   ```

4. Front-end integration will be available in the `front-end` directory.

## OKX Hackathon Integration

This Solana implementation is designed specifically for the OKX Solana Accelerate Hackathon, focusing on the Trading and DeFi tracks by enhancing security for on-chain operations through the OKX DEX.

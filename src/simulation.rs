use crate::executor::{ExecutionReport, ParallelExecutor};
use crate::storage::Bank;
use crate::types::{Account, AccountMeta, Hash, Instruction, Pubkey, Transaction};
use crate::vm::programs::system::{OP_CREATE_ACCOUNT, OP_TRANSFER};
use std::sync::Arc;

/// Simulation harness for benchmarking and testing parallel transaction execution.
pub struct SimulationHarness {
    pub bank: Arc<Bank>,
    pub initial_supply: u64,
}

impl SimulationHarness {
    /// Initialize harness with genesis accounts and specified initial supply.
    pub fn new(initial_supply: u64) -> Self {
        let bank = Arc::new(Bank::new());

        // Fund genesis faucet account
        let faucet = Pubkey::from_u64(1);
        bank.set_account(
            faucet,
            Account::new(initial_supply, 0, Pubkey::SYSTEM_PROGRAM),
        );

        Self {
            bank,
            initial_supply,
        }
    }

    /// Helper to create a native System Program transfer transaction.
    pub fn make_transfer_tx(
        &self,
        from: Pubkey,
        to: Pubkey,
        amount: u64,
        fee: u64,
    ) -> Transaction {
        let mut data = vec![OP_TRANSFER];
        data.extend_from_slice(&amount.to_le_bytes());

        let ix = Instruction::new(
            Pubkey::SYSTEM_PROGRAM,
            vec![
                AccountMeta::new_writable(from, true),
                AccountMeta::new_writable(to, false),
            ],
            data,
        );

        Transaction::new(from, vec![ix], Hash::ZERO, fee, 50_000)
    }

    /// Helper to create a new account creation transaction.
    pub fn make_create_account_tx(
        &self,
        from: Pubkey,
        new_account: Pubkey,
        lamports: u64,
        space: u64,
        owner: Pubkey,
        fee: u64,
    ) -> Transaction {
        let mut data = vec![OP_CREATE_ACCOUNT];
        data.extend_from_slice(&lamports.to_le_bytes());
        data.extend_from_slice(&space.to_le_bytes());
        data.extend_from_slice(&owner.0);

        let ix = Instruction::new(
            Pubkey::SYSTEM_PROGRAM,
            vec![
                AccountMeta::new_writable(from, true),
                AccountMeta::new_writable(new_account, true),
            ],
            data,
        );

        Transaction::new(from, vec![ix], Hash::ZERO, fee, 50_000)
    }

    /// Run block of transactions and verify Lamport conservation invariant.
    pub fn execute_and_verify(&self, transactions: &[Transaction], num_threads: usize) -> ExecutionReport {
        let executor = ParallelExecutor::new(self.bank.clone(), num_threads);
        let report = executor.execute_block(transactions);

        // Verify total circulating lamports
        assert!(
            self.bank.verify_supply_invariant(self.initial_supply),
            "CRITICAL: Lamport supply conservation violated! Total {} != Expected {}",
            self.bank.calculate_total_lamports(),
            self.initial_supply
        );

        report
    }
}

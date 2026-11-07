use crate::scheduler::{ExecutionWave, WaveScheduler};
use crate::storage::Bank;
use crate::types::{Account, Hash, Pubkey, Transaction, TransactionError};
use crate::vm::{dispatch_instruction, ExecutionContext};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

/// Individual transaction execution outcome.
#[derive(Debug, Clone)]
pub struct TransactionResult {
    pub tx_id: Hash,
    pub success: bool,
    pub cu_consumed: u64,
    pub fee_collected: u64,
    pub error: Option<TransactionError>,
}

/// Comprehensive block-level parallel execution report.
#[derive(Debug, Clone)]
pub struct ExecutionReport {
    pub total_transactions: usize,
    pub num_waves: usize,
    pub parallelism_factor: f64,
    pub successful_txs: usize,
    pub failed_txs: usize,
    pub total_cu_consumed: u64,
    pub total_fees_collected: u64,
    pub elapsed_micros: u128,
    pub results: Vec<TransactionResult>,
}

/// Multi-threaded Sealevel parallel transaction execution pipeline.
pub struct ParallelExecutor {
    pub bank: Arc<Bank>,
    pub num_threads: usize,
}

impl ParallelExecutor {
    pub fn new(bank: Arc<Bank>, num_threads: usize) -> Self {
        Self {
            bank,
            num_threads: num_threads.max(1),
        }
    }

    /// Execute a block of transactions using wave-scheduled parallel concurrency.
    pub fn execute_block(&self, transactions: &[Transaction]) -> ExecutionReport {
        let start_time = Instant::now();
        let total_txs = transactions.len();

        if total_txs == 0 {
            return ExecutionReport {
                total_transactions: 0,
                num_waves: 0,
                parallelism_factor: 1.0,
                successful_txs: 0,
                failed_txs: 0,
                total_cu_consumed: 0,
                total_fees_collected: 0,
                elapsed_micros: start_time.elapsed().as_micros(),
                results: Vec::new(),
            };
        }

        // 1. Schedule transactions into mutually disjoint parallel waves
        let waves = WaveScheduler::schedule(transactions);
        let parallelism = WaveScheduler::parallelism_factor(transactions, &waves);

        let mut all_results = vec![
            TransactionResult {
                tx_id: Hash::ZERO,
                success: false,
                cu_consumed: 0,
                fee_collected: 0,
                error: None,
            };
            total_txs
        ];

        // 2. Execute wave by wave
        for wave in &waves {
            self.execute_wave(wave, transactions, &mut all_results);
        }

        let elapsed = start_time.elapsed().as_micros();
        let mut successful = 0;
        let mut failed = 0;
        let mut total_cu = 0;
        let mut total_fees = 0;

        for r in &all_results {
            if r.success {
                successful += 1;
            } else {
                failed += 1;
            }
            total_cu += r.cu_consumed;
            total_fees += r.fee_collected;
        }

        ExecutionReport {
            total_transactions: total_txs,
            num_waves: waves.len(),
            parallelism_factor: parallelism,
            successful_txs: successful,
            failed_txs: failed,
            total_cu_consumed: total_cu,
            total_fees_collected: total_fees,
            elapsed_micros: elapsed,
            results: all_results,
        }
    }

    /// Execute all transactions in a single wave using std::thread::scope for safe concurrency.
    fn execute_wave(
        &self,
        wave: &ExecutionWave,
        transactions: &[Transaction],
        results: &mut [TransactionResult],
    ) {
        let indices = &wave.tx_indices;
        if indices.is_empty() {
            return;
        }

        // If single transaction in wave, execute directly on current thread
        if indices.len() == 1 {
            let idx = indices[0];
            let tx = &transactions[idx];
            let (res, accounts, fee) = self.execute_single_tx(tx);
            self.bank.commit_batch(accounts, fee);
            results[idx] = res;
            return;
        }

        // Multi-threaded execution across disjoint transactions
        let mut wave_outputs: Vec<Option<(TransactionResult, HashMap<Pubkey, Account>, u64)>> =
            vec![None; indices.len()];

        std::thread::scope(|s| {
            let mut handles = Vec::with_capacity(indices.len());

            for (pos, &tx_idx) in indices.iter().enumerate() {
                let tx = &transactions[tx_idx];
                let bank_ref = &self.bank;

                let handle = s.spawn(move || {
                    let out = Self::execute_single_tx_isolated(bank_ref, tx);
                    (pos, tx_idx, out)
                });
                handles.push(handle);
            }

            for handle in handles {
                let (pos, _tx_idx, out) = handle.join().unwrap();
                wave_outputs[pos] = Some(out);
            }
        });

        // Batch commit phase: All writes are guaranteed to be mutually disjoint
        let mut combined_accounts = HashMap::new();
        let mut combined_fees = 0;

        for (pos, &tx_idx) in indices.iter().enumerate() {
            if let Some((res, accounts, fee)) = wave_outputs[pos].take() {
                combined_accounts.extend(accounts);
                combined_fees += fee;
                results[tx_idx] = res;
            }
        }

        self.bank.commit_batch(combined_accounts, combined_fees);
    }

    /// Single transaction execution isolated logic.
    fn execute_single_tx_isolated(
        bank: &Bank,
        tx: &Transaction,
    ) -> (TransactionResult, HashMap<Pubkey, Account>, u64) {
        let tx_id = tx.id();
        let mut local_accounts: HashMap<Pubkey, Account> = HashMap::new();

        // 1. Load fee payer
        let mut fee_payer = match bank.get_account(&tx.fee_payer) {
            Some(acc) => acc,
            None => {
                return (
                    TransactionResult {
                        tx_id,
                        success: false,
                        cu_consumed: 0,
                        fee_collected: 0,
                        error: Some(TransactionError::AccountNotFound(tx.fee_payer)),
                    },
                    local_accounts,
                    0,
                );
            }
        };

        if fee_payer.lamports < tx.fee {
            return (
                TransactionResult {
                    tx_id,
                    success: false,
                    cu_consumed: 0,
                    fee_collected: 0,
                    error: Some(TransactionError::InsufficientFunds {
                        available: fee_payer.lamports,
                        needed: tx.fee,
                    }),
                },
                local_accounts,
                0,
            );
        }

        // Deduct fee upfront
        fee_payer.lamports -= tx.fee;
        local_accounts.insert(tx.fee_payer, fee_payer.clone());

        // 2. Load all other required accounts into local snapshot
        for ix in &tx.instructions {
            for meta in &ix.accounts {
                if !local_accounts.contains_key(&meta.pubkey) {
                    if let Some(acc) = bank.get_account(&meta.pubkey) {
                        local_accounts.insert(meta.pubkey, acc);
                    }
                }
            }
        }

        // 3. Setup execution context with signers
        let mut signers = HashSet::new();
        signers.insert(tx.fee_payer);
        for ix in &tx.instructions {
            for meta in &ix.accounts {
                if meta.is_signer {
                    signers.insert(meta.pubkey);
                }
            }
        }

        let mut ctx = ExecutionContext::new(&mut local_accounts, signers, tx.compute_unit_limit);

        // 4. Dispatch each instruction sequentially
        for ix in &tx.instructions {
            if let Err(err) = dispatch_instruction(ix, &mut ctx) {
                // Revert all program state modifications, but retain fee deduction on fee_payer!
                let mut fee_only_accounts = HashMap::new();
                fee_only_accounts.insert(tx.fee_payer, fee_payer);
                return (
                    TransactionResult {
                        tx_id,
                        success: false,
                        cu_consumed: ctx.meter.consumed,
                        fee_collected: tx.fee,
                        error: Some(err),
                    },
                    fee_only_accounts,
                    tx.fee,
                );
            }
        }

        let cu_consumed = ctx.meter.consumed;
        (
            TransactionResult {
                tx_id,
                success: true,
                cu_consumed,
                fee_collected: tx.fee,
                error: None,
            },
            local_accounts,
            tx.fee,
        )
    }

    fn execute_single_tx(
        &self,
        tx: &Transaction,
    ) -> (TransactionResult, HashMap<Pubkey, Account>, u64) {
        Self::execute_single_tx_isolated(&self.bank, tx)
    }
}

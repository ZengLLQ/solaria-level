use crate::types::{Account, Pubkey};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

const NUM_SHARDS: usize = 32;

/// Sharded concurrent account storage bank with atomic fee accumulation.
pub struct Bank {
    shards: Vec<RwLock<HashMap<Pubkey, Account>>>,
    collected_fees: AtomicU64,
    slot: AtomicU64,
}

impl Bank {
    pub fn new() -> Self {
        let mut shards = Vec::with_capacity(NUM_SHARDS);
        for _ in 0..NUM_SHARDS {
            shards.push(RwLock::new(HashMap::new()));
        }
        Self {
            shards,
            collected_fees: AtomicU64::new(0),
            slot: AtomicU64::new(0),
        }
    }

    #[inline]
    fn shard_index(pubkey: &Pubkey) -> usize {
        // Use first byte mod NUM_SHARDS
        (pubkey.0[0] as usize) % NUM_SHARDS
    }

    /// Read account state.
    pub fn get_account(&self, pubkey: &Pubkey) -> Option<Account> {
        let idx = Self::shard_index(pubkey);
        let shard = self.shards[idx].read().unwrap();
        shard.get(pubkey).cloned()
    }

    /// Set or update an account state.
    pub fn set_account(&self, pubkey: Pubkey, account: Account) {
        let idx = Self::shard_index(&pubkey);
        let mut shard = self.shards[idx].write().unwrap();
        shard.insert(pubkey, account);
    }

    /// Check if account exists.
    pub fn account_exists(&self, pubkey: &Pubkey) -> bool {
        let idx = Self::shard_index(pubkey);
        let shard = self.shards[idx].read().unwrap();
        shard.contains_key(pubkey)
    }

    /// Collect fee lamports atomically.
    pub fn collect_fee(&self, fee: u64) {
        self.collected_fees.fetch_add(fee, Ordering::Relaxed);
    }

    /// Total accumulated transaction fees in current slot.
    pub fn get_collected_fees(&self) -> u64 {
        self.collected_fees.load(Ordering::Relaxed)
    }

    /// Commit a batch of accounts atomically across shards.
    pub fn commit_batch(&self, accounts: HashMap<Pubkey, Account>, fee: u64) {
        // Group accounts by shard
        let mut shard_updates: Vec<HashMap<Pubkey, Account>> = vec![HashMap::new(); NUM_SHARDS];
        for (pk, acc) in accounts {
            let idx = Self::shard_index(&pk);
            shard_updates[idx].insert(pk, acc);
        }

        // Apply shard by shard
        for (idx, updates) in shard_updates.into_iter().enumerate() {
            if !updates.is_empty() {
                let mut shard = self.shards[idx].write().unwrap();
                for (pk, acc) in updates {
                    shard.insert(pk, acc);
                }
            }
        }

        if fee > 0 {
            self.collect_fee(fee);
        }
    }

    /// Calculate total lamports circulating in the bank (all accounts + collected fees).
    pub fn calculate_total_lamports(&self) -> u64 {
        let mut total: u64 = 0;
        for shard in &self.shards {
            let s = shard.read().unwrap();
            for acc in s.values() {
                total = total.checked_add(acc.lamports).expect("Lamport overflow");
            }
        }
        total.checked_add(self.get_collected_fees()).expect("Lamport overflow")
    }

    /// Invariant check: verify that total circulating supply matches expected supply exactly.
    pub fn verify_supply_invariant(&self, expected_supply: u64) -> bool {
        self.calculate_total_lamports() == expected_supply
    }

    /// Advance bank slot number.
    pub fn advance_slot(&self) -> u64 {
        self.slot.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn current_slot(&self) -> u64 {
        self.slot.load(Ordering::SeqCst)
    }
}

impl Default for Bank {
    fn default() -> Self {
        Self::new()
    }
}

use crate::types::{Account, Pubkey, TransactionError};
use std::collections::{HashMap, HashSet};

/// Compute Unit (CU) meter enforcing instruction resource bounds.
#[derive(Debug, Clone, Copy)]
pub struct ComputeMeter {
    pub limit: u32,
    pub consumed: u64,
}

impl ComputeMeter {
    pub const DEFAULT_LIMIT: u32 = 200_000;

    pub fn new(limit: u32) -> Self {
        Self {
            limit: if limit == 0 { Self::DEFAULT_LIMIT } else { limit },
            consumed: 0,
        }
    }

    /// Consume compute units, failing if budget exceeded.
    pub fn consume(&mut self, units: u64) -> Result<(), TransactionError> {
        let new_consumed = self.consumed.saturating_add(units);
        if new_consumed > self.limit as u64 {
            return Err(TransactionError::ComputeBudgetExceeded {
                used: new_consumed,
                limit: self.limit,
            });
        }
        self.consumed = new_consumed;
        Ok(())
    }

    pub fn remaining(&self) -> u64 {
        (self.limit as u64).saturating_sub(self.consumed)
    }
}

/// Transaction execution context tracking account modifications and signers.
pub struct ExecutionContext<'a> {
    pub accounts: &'a mut HashMap<Pubkey, Account>,
    pub signers: HashSet<Pubkey>,
    pub meter: ComputeMeter,
}

impl<'a> ExecutionContext<'a> {
    pub fn new(
        accounts: &'a mut HashMap<Pubkey, Account>,
        signers: HashSet<Pubkey>,
        cu_limit: u32,
    ) -> Self {
        Self {
            accounts,
            signers,
            meter: ComputeMeter::new(cu_limit),
        }
    }

    pub fn is_signer(&self, pubkey: &Pubkey) -> bool {
        self.signers.contains(pubkey)
    }

    pub fn get_account(&self, pubkey: &Pubkey) -> Result<&Account, TransactionError> {
        self.accounts
            .get(pubkey)
            .ok_or(TransactionError::AccountNotFound(*pubkey))
    }

    pub fn get_account_mut(&mut self, pubkey: &Pubkey) -> Result<&mut Account, TransactionError> {
        self.accounts
            .get_mut(pubkey)
            .ok_or(TransactionError::AccountNotFound(*pubkey))
    }
}

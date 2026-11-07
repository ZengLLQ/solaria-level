use crate::types::{Account, Instruction, Pubkey, TransactionError};
use crate::vm::context::ExecutionContext;

pub const OP_CREATE_ACCOUNT: u8 = 0;
pub const OP_ASSIGN: u8 = 1;
pub const OP_TRANSFER: u8 = 2;
pub const OP_ALLOCATE: u8 = 3;

/// System Program instruction executor.
pub struct SystemProgram;

impl SystemProgram {
    pub fn execute(ix: &Instruction, ctx: &mut ExecutionContext) -> Result<(), TransactionError> {
        // Base cost for System Program invocation: 150 CU
        ctx.meter.consume(150)?;

        if ix.data.is_empty() {
            return Err(TransactionError::InstructionError(0, "Empty instruction data".into()));
        }

        let op = ix.data[0];
        match op {
            OP_CREATE_ACCOUNT => Self::create_account(ix, ctx),
            OP_ASSIGN => Self::assign(ix, ctx),
            OP_TRANSFER => Self::transfer(ix, ctx),
            OP_ALLOCATE => Self::allocate(ix, ctx),
            _ => Err(TransactionError::InstructionError(
                0,
                format!("Unknown system opcode: {}", op),
            )),
        }
    }

    /// Create new account: requires funder (signer), new_account (signer)
    /// Data format: [op: 1B, lamports: 8B, space: 8B, owner: 32B]
    fn create_account(ix: &Instruction, ctx: &mut ExecutionContext) -> Result<(), TransactionError> {
        ctx.meter.consume(300)?;

        if ix.accounts.len() < 2 {
            return Err(TransactionError::InstructionError(0, "CreateAccount requires 2 accounts".into()));
        }
        if ix.data.len() < 49 {
            return Err(TransactionError::InstructionError(0, "Invalid CreateAccount data length".into()));
        }

        let from_pk = ix.accounts[0].pubkey;
        let to_pk = ix.accounts[1].pubkey;

        if !ctx.is_signer(&from_pk) {
            return Err(TransactionError::MissingRequiredSignature(from_pk));
        }
        if !ctx.is_signer(&to_pk) {
            return Err(TransactionError::MissingRequiredSignature(to_pk));
        }

        let lamports = u64::from_le_bytes(ix.data[1..9].try_into().unwrap());
        let space = u64::from_le_bytes(ix.data[9..17].try_into().unwrap()) as usize;
        let mut owner_bytes = [0u8; 32];
        owner_bytes.copy_from_slice(&ix.data[17..49]);
        let owner = Pubkey::new(owner_bytes);

        // Deduct from funder
        let from_acc = ctx.get_account_mut(&from_pk)?;
        if from_acc.lamports < lamports {
            return Err(TransactionError::InsufficientFunds {
                available: from_acc.lamports,
                needed: lamports,
            });
        }
        from_acc.lamports -= lamports;

        // Initialize new account
        let new_acc = Account {
            lamports,
            data: vec![0u8; space],
            owner,
            executable: false,
            rent_epoch: 0,
        };
        ctx.accounts.insert(to_pk, new_acc);

        Ok(())
    }

    /// Transfer lamports: from (signer), to
    /// Data format: [op: 1B, lamports: 8B]
    fn transfer(ix: &Instruction, ctx: &mut ExecutionContext) -> Result<(), TransactionError> {
        ctx.meter.consume(150)?;

        if ix.accounts.len() < 2 {
            return Err(TransactionError::InstructionError(0, "Transfer requires 2 accounts".into()));
        }
        if ix.data.len() < 9 {
            return Err(TransactionError::InstructionError(0, "Invalid Transfer data length".into()));
        }

        let from_pk = ix.accounts[0].pubkey;
        let to_pk = ix.accounts[1].pubkey;

        if !ctx.is_signer(&from_pk) {
            return Err(TransactionError::MissingRequiredSignature(from_pk));
        }

        let lamports = u64::from_le_bytes(ix.data[1..9].try_into().unwrap());

        if from_pk == to_pk {
            return Ok(()); // Self-transfer is a no-op
        }

        let from_acc = ctx.get_account_mut(&from_pk)?;
        if from_acc.lamports < lamports {
            return Err(TransactionError::InsufficientFunds {
                available: from_acc.lamports,
                needed: lamports,
            });
        }
        from_acc.lamports -= lamports;

        // Credit destination account (or create if not yet existing)
        if let Some(to_acc) = ctx.accounts.get_mut(&to_pk) {
            to_acc.lamports = to_acc.lamports.checked_add(lamports).expect("Lamport overflow");
        } else {
            let to_acc = Account::new(lamports, 0, Pubkey::SYSTEM_PROGRAM);
            ctx.accounts.insert(to_pk, to_acc);
        }

        Ok(())
    }

    /// Assign account to new owner: account (signer)
    /// Data format: [op: 1B, owner: 32B]
    fn assign(ix: &Instruction, ctx: &mut ExecutionContext) -> Result<(), TransactionError> {
        ctx.meter.consume(150)?;
        if ix.accounts.is_empty() {
            return Err(TransactionError::InstructionError(0, "Assign requires 1 account".into()));
        }
        if ix.data.len() < 33 {
            return Err(TransactionError::InstructionError(0, "Invalid Assign data length".into()));
        }

        let target_pk = ix.accounts[0].pubkey;
        if !ctx.is_signer(&target_pk) {
            return Err(TransactionError::MissingRequiredSignature(target_pk));
        }

        let mut owner_bytes = [0u8; 32];
        owner_bytes.copy_from_slice(&ix.data[1..33]);
        let owner = Pubkey::new(owner_bytes);

        let acc = ctx.get_account_mut(&target_pk)?;
        acc.owner = owner;
        Ok(())
    }

    /// Allocate space for account: account (signer)
    /// Data format: [op: 1B, space: 8B]
    fn allocate(ix: &Instruction, ctx: &mut ExecutionContext) -> Result<(), TransactionError> {
        ctx.meter.consume(200)?;
        if ix.accounts.is_empty() {
            return Err(TransactionError::InstructionError(0, "Allocate requires 1 account".into()));
        }
        if ix.data.len() < 9 {
            return Err(TransactionError::InstructionError(0, "Invalid Allocate data length".into()));
        }

        let target_pk = ix.accounts[0].pubkey;
        if !ctx.is_signer(&target_pk) {
            return Err(TransactionError::MissingRequiredSignature(target_pk));
        }

        let space = u64::from_le_bytes(ix.data[1..9].try_into().unwrap()) as usize;
        let acc = ctx.get_account_mut(&target_pk)?;
        acc.data.resize(space, 0u8);
        Ok(())
    }
}

use crate::types::{Instruction, Pubkey, TransactionError};
use crate::vm::context::ExecutionContext;

pub const OP_TOKEN_INIT_MINT: u8 = 0;
pub const OP_TOKEN_INIT_ACCOUNT: u8 = 1;
pub const OP_TOKEN_TRANSFER: u8 = 2;
pub const OP_TOKEN_MINT_TO: u8 = 3;
pub const OP_TOKEN_BURN: u8 = 4;
pub const OP_TOKEN_FREEZE: u8 = 5;

pub const ACCOUNT_STATE_UNINITIALIZED: u8 = 0;
pub const ACCOUNT_STATE_ACTIVE: u8 = 1;
pub const ACCOUNT_STATE_FROZEN: u8 = 2;

/// SPL Token Mint structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mint {
    pub supply: u64,
    pub decimals: u8,
    pub is_initialized: bool,
    pub mint_authority: Pubkey,
}

impl Mint {
    pub fn unpack(data: &[u8]) -> Result<Self, TransactionError> {
        if data.len() < 42 {
            return Err(TransactionError::InstructionError(0, "Invalid mint data length".into()));
        }
        let supply = u64::from_le_bytes(data[0..8].try_into().unwrap());
        let decimals = data[8];
        let is_initialized = data[9] != 0;
        let mut auth = [0u8; 32];
        auth.copy_from_slice(&data[10..42]);
        Ok(Self {
            supply,
            decimals,
            is_initialized,
            mint_authority: Pubkey::new(auth),
        })
    }

    pub fn pack(&self, data: &mut [u8]) {
        data[0..8].copy_from_slice(&self.supply.to_le_bytes());
        data[8] = self.decimals;
        data[9] = if self.is_initialized { 1 } else { 0 };
        data[10..42].copy_from_slice(&self.mint_authority.0);
    }
}

/// SPL Token Account structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenAccount {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub amount: u64,
    pub state: u8,
}

impl TokenAccount {
    pub fn unpack(data: &[u8]) -> Result<Self, TransactionError> {
        if data.len() < 73 {
            return Err(TransactionError::InstructionError(0, "Invalid token account data length".into()));
        }
        let mut mint = [0u8; 32];
        mint.copy_from_slice(&data[0..32]);
        let mut owner = [0u8; 32];
        owner.copy_from_slice(&data[32..64]);
        let amount = u64::from_le_bytes(data[64..72].try_into().unwrap());
        let state = data[72];

        Ok(Self {
            mint: Pubkey::new(mint),
            owner: Pubkey::new(owner),
            amount,
            state,
        })
    }

    pub fn pack(&self, data: &mut [u8]) {
        data[0..32].copy_from_slice(&self.mint.0);
        data[32..64].copy_from_slice(&self.owner.0);
        data[64..72].copy_from_slice(&self.amount.to_le_bytes());
        data[72] = self.state;
    }
}

/// SPL Token Program execution logic.
pub struct TokenProgram;

impl TokenProgram {
    pub fn execute(ix: &Instruction, ctx: &mut ExecutionContext) -> Result<(), TransactionError> {
        ctx.meter.consume(300)?;

        if ix.data.is_empty() {
            return Err(TransactionError::InstructionError(0, "Empty token instruction data".into()));
        }

        let op = ix.data[0];
        match op {
            OP_TOKEN_INIT_MINT => Self::init_mint(ix, ctx),
            OP_TOKEN_INIT_ACCOUNT => Self::init_account(ix, ctx),
            OP_TOKEN_TRANSFER => Self::transfer(ix, ctx),
            OP_TOKEN_MINT_TO => Self::mint_to(ix, ctx),
            OP_TOKEN_BURN => Self::burn(ix, ctx),
            OP_TOKEN_FREEZE => Self::freeze(ix, ctx),
            _ => Err(TransactionError::InstructionError(
                0,
                format!("Unknown token opcode: {}", op),
            )),
        }
    }

    fn init_mint(ix: &Instruction, ctx: &mut ExecutionContext) -> Result<(), TransactionError> {
        ctx.meter.consume(500)?;
        if ix.accounts.is_empty() {
            return Err(TransactionError::InstructionError(0, "InitMint requires 1 account".into()));
        }
        if ix.data.len() < 34 {
            return Err(TransactionError::InstructionError(0, "Invalid InitMint data length".into()));
        }

        let mint_pk = ix.accounts[0].pubkey;
        let decimals = ix.data[1];
        let mut auth_bytes = [0u8; 32];
        auth_bytes.copy_from_slice(&ix.data[2..34]);
        let mint_authority = Pubkey::new(auth_bytes);

        let acc = ctx.get_account_mut(&mint_pk)?;
        acc.owner = Pubkey::TOKEN_PROGRAM;
        acc.data.resize(42, 0u8);

        let mint = Mint {
            supply: 0,
            decimals,
            is_initialized: true,
            mint_authority,
        };
        mint.pack(&mut acc.data);
        Ok(())
    }

    fn init_account(ix: &Instruction, ctx: &mut ExecutionContext) -> Result<(), TransactionError> {
        ctx.meter.consume(500)?;
        if ix.accounts.len() < 3 {
            return Err(TransactionError::InstructionError(0, "InitAccount requires 3 accounts (account, mint, owner)".into()));
        }

        let acc_pk = ix.accounts[0].pubkey;
        let mint_pk = ix.accounts[1].pubkey;
        let owner_pk = ix.accounts[2].pubkey;

        // Verify mint exists
        let _ = ctx.get_account(&mint_pk)?;

        let acc = ctx.get_account_mut(&acc_pk)?;
        acc.owner = Pubkey::TOKEN_PROGRAM;
        acc.data.resize(73, 0u8);

        let token_acc = TokenAccount {
            mint: mint_pk,
            owner: owner_pk,
            amount: 0,
            state: ACCOUNT_STATE_ACTIVE,
        };
        token_acc.pack(&mut acc.data);
        Ok(())
    }

    fn transfer(ix: &Instruction, ctx: &mut ExecutionContext) -> Result<(), TransactionError> {
        ctx.meter.consume(800)?;
        if ix.accounts.len() < 3 {
            return Err(TransactionError::InstructionError(0, "Token transfer requires 3 accounts (source, dest, authority)".into()));
        }
        if ix.data.len() < 9 {
            return Err(TransactionError::InstructionError(0, "Invalid transfer data length".into()));
        }

        let src_pk = ix.accounts[0].pubkey;
        let dst_pk = ix.accounts[1].pubkey;
        let auth_pk = ix.accounts[2].pubkey;

        if !ctx.is_signer(&auth_pk) {
            return Err(TransactionError::MissingRequiredSignature(auth_pk));
        }

        let amount = u64::from_le_bytes(ix.data[1..9].try_into().unwrap());

        if src_pk == dst_pk {
            return Ok(());
        }

        // Unpack source account
        let mut src_tok = {
            let src_acc = ctx.get_account(&src_pk)?;
            TokenAccount::unpack(&src_acc.data)?
        };

        if src_tok.owner != auth_pk {
            return Err(TransactionError::InstructionError(0, "Authority is not owner of source token account".into()));
        }
        if src_tok.state != ACCOUNT_STATE_ACTIVE {
            return Err(TransactionError::InstructionError(0, "Source token account is frozen".into()));
        }
        if src_tok.amount < amount {
            return Err(TransactionError::InsufficientFunds {
                available: src_tok.amount,
                needed: amount,
            });
        }

        // Unpack destination account
        let mut dst_tok = {
            let dst_acc = ctx.get_account(&dst_pk)?;
            TokenAccount::unpack(&dst_acc.data)?
        };

        if dst_tok.mint != src_tok.mint {
            return Err(TransactionError::InstructionError(0, "Token mint mismatch".into()));
        }
        if dst_tok.state != ACCOUNT_STATE_ACTIVE {
            return Err(TransactionError::InstructionError(0, "Destination token account is frozen".into()));
        }

        // Perform balance updates
        src_tok.amount -= amount;
        dst_tok.amount = dst_tok.amount.checked_add(amount).expect("Token amount overflow");

        // Pack back
        let src_acc = ctx.get_account_mut(&src_pk)?;
        src_tok.pack(&mut src_acc.data);

        let dst_acc = ctx.get_account_mut(&dst_pk)?;
        dst_tok.pack(&mut dst_acc.data);

        Ok(())
    }

    fn mint_to(ix: &Instruction, ctx: &mut ExecutionContext) -> Result<(), TransactionError> {
        ctx.meter.consume(800)?;
        if ix.accounts.len() < 3 {
            return Err(TransactionError::InstructionError(0, "MintTo requires 3 accounts (mint, dest, authority)".into()));
        }
        if ix.data.len() < 9 {
            return Err(TransactionError::InstructionError(0, "Invalid MintTo data length".into()));
        }

        let mint_pk = ix.accounts[0].pubkey;
        let dst_pk = ix.accounts[1].pubkey;
        let auth_pk = ix.accounts[2].pubkey;

        if !ctx.is_signer(&auth_pk) {
            return Err(TransactionError::MissingRequiredSignature(auth_pk));
        }

        let amount = u64::from_le_bytes(ix.data[1..9].try_into().unwrap());

        let mut mint = {
            let mint_acc = ctx.get_account(&mint_pk)?;
            Mint::unpack(&mint_acc.data)?
        };

        if mint.mint_authority != auth_pk {
            return Err(TransactionError::InstructionError(0, "Authority is not mint authority".into()));
        }

        let mut dst_tok = {
            let dst_acc = ctx.get_account(&dst_pk)?;
            TokenAccount::unpack(&dst_acc.data)?
        };

        if dst_tok.mint != mint_pk {
            return Err(TransactionError::InstructionError(0, "Token account mint mismatch".into()));
        }
        if dst_tok.state != ACCOUNT_STATE_ACTIVE {
            return Err(TransactionError::InstructionError(0, "Destination token account is frozen".into()));
        }

        mint.supply = mint.supply.checked_add(amount).expect("Mint supply overflow");
        dst_tok.amount = dst_tok.amount.checked_add(amount).expect("Token amount overflow");

        let mint_acc = ctx.get_account_mut(&mint_pk)?;
        mint.pack(&mut mint_acc.data);

        let dst_acc = ctx.get_account_mut(&dst_pk)?;
        dst_tok.pack(&mut dst_acc.data);

        Ok(())
    }

    fn burn(ix: &Instruction, ctx: &mut ExecutionContext) -> Result<(), TransactionError> {
        ctx.meter.consume(800)?;
        if ix.accounts.len() < 3 {
            return Err(TransactionError::InstructionError(0, "Burn requires 3 accounts (source, mint, authority)".into()));
        }
        if ix.data.len() < 9 {
            return Err(TransactionError::InstructionError(0, "Invalid Burn data length".into()));
        }

        let src_pk = ix.accounts[0].pubkey;
        let mint_pk = ix.accounts[1].pubkey;
        let auth_pk = ix.accounts[2].pubkey;

        if !ctx.is_signer(&auth_pk) {
            return Err(TransactionError::MissingRequiredSignature(auth_pk));
        }

        let amount = u64::from_le_bytes(ix.data[1..9].try_into().unwrap());

        let mut src_tok = {
            let src_acc = ctx.get_account(&src_pk)?;
            TokenAccount::unpack(&src_acc.data)?
        };

        if src_tok.owner != auth_pk {
            return Err(TransactionError::InstructionError(0, "Authority is not owner".into()));
        }
        if src_tok.mint != mint_pk {
            return Err(TransactionError::InstructionError(0, "Mint mismatch".into()));
        }
        if src_tok.amount < amount {
            return Err(TransactionError::InsufficientFunds {
                available: src_tok.amount,
                needed: amount,
            });
        }

        let mut mint = {
            let mint_acc = ctx.get_account(&mint_pk)?;
            Mint::unpack(&mint_acc.data)?
        };

        src_tok.amount -= amount;
        mint.supply = mint.supply.checked_sub(amount).expect("Mint supply underflow");

        let src_acc = ctx.get_account_mut(&src_pk)?;
        src_tok.pack(&mut src_acc.data);

        let mint_acc = ctx.get_account_mut(&mint_pk)?;
        mint.pack(&mut mint_acc.data);

        Ok(())
    }

    fn freeze(ix: &Instruction, ctx: &mut ExecutionContext) -> Result<(), TransactionError> {
        ctx.meter.consume(400)?;
        if ix.accounts.len() < 2 {
            return Err(TransactionError::InstructionError(0, "Freeze requires 2 accounts (account, mint_authority)".into()));
        }

        let acc_pk = ix.accounts[0].pubkey;
        let auth_pk = ix.accounts[1].pubkey;

        if !ctx.is_signer(&auth_pk) {
            return Err(TransactionError::MissingRequiredSignature(auth_pk));
        }

        let mut tok = {
            let acc = ctx.get_account(&acc_pk)?;
            TokenAccount::unpack(&acc.data)?
        };

        tok.state = ACCOUNT_STATE_FROZEN;
        let acc = ctx.get_account_mut(&acc_pk)?;
        tok.pack(&mut acc.data);
        Ok(())
    }
}

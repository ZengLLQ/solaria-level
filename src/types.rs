use std::collections::HashSet;
use std::fmt;

/// 32-byte Account Address (Pubkey).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Pubkey(pub [u8; 32]);

impl Pubkey {
    pub const SYSTEM_PROGRAM: Self = Self([0u8; 32]);
    pub const TOKEN_PROGRAM: Self = Self([
        0x06, 0xdd, 0xf2, 0x52, 0xad, 0x1b, 0xe2, 0xc8, 0x9b, 0x69, 0xc2, 0xb0, 0x68, 0xfc,
        0x37, 0x8d, 0xaa, 0x95, 0x2b, 0xa7, 0xf1, 0x63, 0xc4, 0xa1, 0x16, 0x28, 0xf5, 0x5a,
        0x4d, 0xf5, 0x23, 0xb3,
    ]);
    pub const SYSVAR_RENT: Self = Self([
        0x06, 0xa7, 0xd5, 0x17, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18,
        0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18,
        0x18, 0x18, 0x18, 0x18,
    ]);

    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn from_u64(val: u64) -> Self {
        let mut b = [0u8; 32];
        b[24..32].copy_from_slice(&val.to_be_bytes());
        Self(b)
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

impl fmt::Debug for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Pubkey(0x{}...{})", &self.to_hex()[..8], &self.to_hex()[56..])
    }
}

impl fmt::Display for Pubkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", self.to_hex())
    }
}

/// 32-byte Cryptographic Hash.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Hash(pub [u8; 32]);

impl Hash {
    pub const ZERO: Self = Self([0u8; 32]);

    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn compute(data: &[u8]) -> Self {
        Self(sha256(data))
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash(0x{}...{})", &self.to_hex()[..8], &self.to_hex()[56..])
    }
}

/// Minimal internal SHA-256 implementation ensuring zero dependencies.
pub fn sha256(input: &[u8]) -> [u8; 32] {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    let k: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    let bit_len = (input.len() as u64) * 8;
    let mut msg = input.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut h_var = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h_var.wrapping_add(s1).wrapping_add(ch).wrapping_add(k[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h_var = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(h_var);
    }

    let mut result = [0u8; 32];
    for (i, val) in h.iter().enumerate() {
        result[i * 4..i * 4 + 4].copy_from_slice(&val.to_be_bytes());
    }
    result
}

/// Declares read/write permissions for an account referenced in a transaction instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccountMeta {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl AccountMeta {
    pub fn new(pubkey: Pubkey, is_signer: bool, is_writable: bool) -> Self {
        Self { pubkey, is_signer, is_writable }
    }

    pub fn new_readonly(pubkey: Pubkey, is_signer: bool) -> Self {
        Self::new(pubkey, is_signer, false)
    }

    pub fn new_writable(pubkey: Pubkey, is_signer: bool) -> Self {
        Self::new(pubkey, is_signer, true)
    }
}

/// Instruction invoking a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    pub program_id: Pubkey,
    pub accounts: Vec<AccountMeta>,
    pub data: Vec<u8>,
}

impl Instruction {
    pub fn new(program_id: Pubkey, accounts: Vec<AccountMeta>, data: Vec<u8>) -> Self {
        Self { program_id, accounts, data }
    }
}

/// Transaction with upfront account locking declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub fee_payer: Pubkey,
    pub instructions: Vec<Instruction>,
    pub recent_blockhash: Hash,
    pub fee: u64,
    pub compute_unit_limit: u32,
    pub compute_unit_price: u64, // Micro-lamports per CU
}

impl Transaction {
    pub fn new(
        fee_payer: Pubkey,
        instructions: Vec<Instruction>,
        recent_blockhash: Hash,
        fee: u64,
        compute_unit_limit: u32,
    ) -> Self {
        Self {
            fee_payer,
            instructions,
            recent_blockhash,
            fee,
            compute_unit_limit,
            compute_unit_price: 0,
        }
    }

    /// Compute unique transaction digest ID.
    pub fn id(&self) -> Hash {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.fee_payer.0);
        bytes.extend_from_slice(&self.recent_blockhash.0);
        bytes.extend_from_slice(&self.fee.to_le_bytes());
        for ix in &self.instructions {
            bytes.extend_from_slice(&ix.program_id.0);
            for acc in &ix.accounts {
                bytes.extend_from_slice(&acc.pubkey.0);
                bytes.push(if acc.is_writable { 1 } else { 0 });
                bytes.push(if acc.is_signer { 1 } else { 0 });
            }
            bytes.extend_from_slice(&ix.data);
        }
        Hash::compute(&bytes)
    }

    /// Set of accounts that are read-only in this transaction.
    pub fn get_read_set(&self) -> HashSet<Pubkey> {
        let mut reads = HashSet::new();
        let writes = self.get_write_set();

        for ix in &self.instructions {
            reads.insert(ix.program_id);
            for acc in &ix.accounts {
                if !acc.is_writable && !writes.contains(&acc.pubkey) {
                    reads.insert(acc.pubkey);
                }
            }
        }
        reads
    }

    /// Set of accounts that require exclusive write locks.
    pub fn get_write_set(&self) -> HashSet<Pubkey> {
        let mut writes = HashSet::new();
        writes.insert(self.fee_payer); // Fee payer is always writable (deducting fee)

        for ix in &self.instructions {
            for acc in &ix.accounts {
                if acc.is_writable {
                    writes.insert(acc.pubkey);
                }
            }
        }
        writes
    }

    /// Check if this transaction conflicts with another (shared write lock, or write-read overlap).
    pub fn conflicts_with(&self, other: &Self) -> bool {
        let w1 = self.get_write_set();
        let r1 = self.get_read_set();

        let w2 = other.get_write_set();
        let r2 = other.get_read_set();

        // Write-Write conflict
        if !w1.is_disjoint(&w2) {
            return true;
        }
        // Write-Read conflict (RAW / WAR)
        if !w1.is_disjoint(&r2) {
            return true;
        }
        if !r1.is_disjoint(&w2) {
            return true;
        }

        false
    }
}

/// On-chain account state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub lamports: u64,
    pub data: Vec<u8>,
    pub owner: Pubkey,
    pub executable: bool,
    pub rent_epoch: u64,
}

impl Account {
    pub fn new(lamports: u64, data_len: usize, owner: Pubkey) -> Self {
        Self {
            lamports,
            data: vec![0u8; data_len],
            owner,
            executable: false,
            rent_epoch: 0,
        }
    }

    pub fn new_with_data(lamports: u64, data: Vec<u8>, owner: Pubkey) -> Self {
        Self {
            lamports,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        }
    }
}

/// Execution error taxonomy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionError {
    AccountNotFound(Pubkey),
    InsufficientFunds { available: u64, needed: u64 },
    AccountBorrowFailed,
    InstructionError(u32, String),
    ProgramNotFound(Pubkey),
    MissingRequiredSignature(Pubkey),
    ComputeBudgetExceeded { used: u64, limit: u32 },
    Custom(u32),
}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccountNotFound(p) => write!(f, "Account not found: {}", p),
            Self::InsufficientFunds { available, needed } => {
                write!(f, "Insufficient funds: available {}, needed {}", available, needed)
            }
            Self::AccountBorrowFailed => write!(f, "Account borrow conflict"),
            Self::InstructionError(idx, err) => write!(f, "Instruction {} error: {}", idx, err),
            Self::ProgramNotFound(p) => write!(f, "Program not found: {}", p),
            Self::MissingRequiredSignature(p) => write!(f, "Missing signature for: {}", p),
            Self::ComputeBudgetExceeded { used, limit } => {
                write!(f, "Compute budget exceeded: used {} > limit {}", used, limit)
            }
            Self::Custom(c) => write!(f, "Custom program error: {}", c),
        }
    }
}

pub mod context;
pub mod programs;

use crate::types::{Instruction, Pubkey, TransactionError};
pub use context::{ComputeMeter, ExecutionContext};
pub use programs::{SystemProgram, TokenProgram};

/// Dispatch instruction to registered native program.
pub fn dispatch_instruction(
    ix: &Instruction,
    ctx: &mut ExecutionContext,
) -> Result<(), TransactionError> {
    if ix.program_id == Pubkey::SYSTEM_PROGRAM {
        SystemProgram::execute(ix, ctx)
    } else if ix.program_id == Pubkey::TOKEN_PROGRAM {
        TokenProgram::execute(ix, ctx)
    } else {
        Err(TransactionError::ProgramNotFound(ix.program_id))
    }
}

use solaria_level::{
    types::{Account, AccountMeta, Hash, Instruction, Pubkey, Transaction, TransactionError},
    SimulationHarness,
};

#[test]
fn test_compute_budget_exceeded_rollback() {
    let harness = SimulationHarness::new(100_000_000);

    let sender = Pubkey::from_u64(10);
    let receiver = Pubkey::from_u64(20);

    harness.bank.set_account(sender, Account::new(100_000, 0, Pubkey::SYSTEM_PROGRAM));

    let mut transfer_data = vec![2]; // OP_TRANSFER
    transfer_data.extend_from_slice(&50_000u64.to_le_bytes());

    let ix = Instruction::new(
        Pubkey::SYSTEM_PROGRAM,
        vec![
            AccountMeta::new_writable(sender, true),
            AccountMeta::new_writable(receiver, false),
        ],
        transfer_data,
    );

    // Set an unrealistically low CU limit (100 CU), while System Transfer requires 300 CU
    let tx = Transaction::new(sender, vec![ix], Hash::ZERO, 500, 100);

    let executor = solaria_level::ParallelExecutor::new(harness.bank.clone(), 1);
    let report = executor.execute_block(&[tx]);

    assert_eq!(report.successful_txs, 0);
    assert_eq!(report.failed_txs, 1);

    // Verify error is ComputeBudgetExceeded
    match &report.results[0].error {
        Some(TransactionError::ComputeBudgetExceeded { used, limit }) => {
            assert_eq!(*limit, 100);
            assert!(*used > 100);
        }
        other => panic!("Expected ComputeBudgetExceeded error, got: {:?}", other),
    }

    // State rollback verification:
    // Transfer did NOT occur: receiver was NOT credited
    assert_eq!(harness.bank.get_account(&receiver), None);

    // But fee was still collected!
    // Fee payer paid 500 fee, original balance 100,000 -> 99,500
    let sender_acc = harness.bank.get_account(&sender).unwrap();
    assert_eq!(sender_acc.lamports, 99_500);
    assert_eq!(harness.bank.get_collected_fees(), 500);
}

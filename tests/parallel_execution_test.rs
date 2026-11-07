use solaria_level::{
    types::{Account, Pubkey},
    SimulationHarness,
};

#[test]
fn test_parallel_transfers_correctness() {
    let initial_supply = 1_000_000_000u64;
    let harness = SimulationHarness::new(initial_supply);

    let num_txs = 20;
    let faucet = Pubkey::from_u64(1);
    let mut txs = Vec::new();

    // Setup 20 senders with 10,000 lamports each
    for i in 0..num_txs {
        let sender = Pubkey::from_u64(100 + i as u64);
        let receiver = Pubkey::from_u64(200 + i as u64);

        harness.bank.set_account(sender, Account::new(10_000, 0, Pubkey::SYSTEM_PROGRAM));
        txs.push(harness.make_transfer_tx(sender, receiver, 3_000, 200));
    }

    // Deduct funded lamports from faucet
    let funded_total = (num_txs as u64) * 10_000;
    let mut faucet_acc = harness.bank.get_account(&faucet).unwrap();
    faucet_acc.lamports -= funded_total;
    harness.bank.set_account(faucet, faucet_acc);

    // Execute with 4 parallel threads
    let report = harness.execute_and_verify(&txs, 4);

    assert_eq!(report.successful_txs, num_txs);
    assert_eq!(report.failed_txs, 0);
    assert_eq!(report.num_waves, 1, "Disjoint txs should execute in single wave");

    // Check balances
    for i in 0..num_txs {
        let sender = Pubkey::from_u64(100 + i as u64);
        let receiver = Pubkey::from_u64(200 + i as u64);

        let sender_acc = harness.bank.get_account(&sender).unwrap();
        let receiver_acc = harness.bank.get_account(&receiver).unwrap();

        // Sender started with 10,000, sent 3,000, paid 200 fee -> remaining 6,800
        assert_eq!(sender_acc.lamports, 6_800);
        // Receiver received 3,000
        assert_eq!(receiver_acc.lamports, 3_000);
    }

    // Verify total supply invariant
    assert!(harness.bank.verify_supply_invariant(initial_supply));
}

#[test]
fn test_conflicting_transfers_serialization_deterministic() {
    let initial_supply = 500_000_000u64;
    let harness = SimulationHarness::new(initial_supply);

    let sender = Pubkey::from_u64(50);
    let receiver = Pubkey::from_u64(60);

    // Sender funded with 50,000
    harness.bank.set_account(sender, Account::new(50_000, 0, Pubkey::SYSTEM_PROGRAM));
    let mut faucet_acc = harness.bank.get_account(&Pubkey::from_u64(1)).unwrap();
    faucet_acc.lamports -= 50_000;
    harness.bank.set_account(Pubkey::from_u64(1), faucet_acc);

    // 5 transactions from the exact same sender (conflicting on sender write lock)
    let mut txs = Vec::new();
    for _ in 0..5 {
        txs.push(harness.make_transfer_tx(sender, receiver, 5_000, 100));
    }

    let report = harness.execute_and_verify(&txs, 4);
    assert_eq!(report.successful_txs, 5);
    assert_eq!(report.num_waves, 5, "5 conflicting txs must execute across 5 sequential waves");

    let final_sender = harness.bank.get_account(&sender).unwrap();
    let final_receiver = harness.bank.get_account(&receiver).unwrap();

    // Sender: 50,000 - 5 * (5,000 + 100) = 50,000 - 25,500 = 24,500
    assert_eq!(final_sender.lamports, 24_500);
    // Receiver: 5 * 5,000 = 25,000
    assert_eq!(final_receiver.lamports, 25_000);
    // Collected fees: 5 * 100 = 500
    assert_eq!(harness.bank.get_collected_fees(), 500);

    assert!(harness.bank.verify_supply_invariant(initial_supply));
}

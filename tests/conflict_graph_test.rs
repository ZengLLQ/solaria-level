use solaria_level::{
    scheduler::{ConflictGraph, WaveScheduler},
    types::{AccountMeta, Hash, Instruction, Pubkey, Transaction},
};

fn make_dummy_tx(fee_payer: Pubkey, write_accounts: Vec<Pubkey>, read_accounts: Vec<Pubkey>) -> Transaction {
    let mut metas = Vec::new();
    for pk in write_accounts {
        metas.push(AccountMeta::new_writable(pk, false));
    }
    for pk in read_accounts {
        metas.push(AccountMeta::new_readonly(pk, false));
    }
    let ix = Instruction::new(Pubkey::SYSTEM_PROGRAM, metas, vec![]);
    Transaction::new(fee_payer, vec![ix], Hash::ZERO, 500, 50_000)
}

#[test]
fn test_independent_transactions_single_wave() {
    let mut txs = Vec::new();
    for i in 0..10 {
        let p1 = Pubkey::from_u64(100 + i * 2);
        let p2 = Pubkey::from_u64(100 + i * 2 + 1);
        txs.push(make_dummy_tx(p1, vec![p2], vec![]));
    }

    let graph = ConflictGraph::build(&txs);
    for i in 0..10 {
        assert_eq!(graph.in_degree(i), 0, "Independent tx should have in_degree 0");
    }

    let waves = WaveScheduler::schedule(&txs);
    assert_eq!(waves.len(), 1, "All 10 independent txs should schedule into 1 wave");
    assert_eq!(waves[0].tx_indices.len(), 10);
    assert_eq!(WaveScheduler::parallelism_factor(&txs, &waves), 10.0);
}

#[test]
fn test_conflicting_chain_serial_waves() {
    let shared_account = Pubkey::from_u64(999);
    let mut txs = Vec::new();

    // 5 transactions writing to the exact same shared account
    for i in 0..5 {
        let payer = Pubkey::from_u64(10 + i);
        txs.push(make_dummy_tx(payer, vec![shared_account], vec![]));
    }

    let graph = ConflictGraph::build(&txs);
    // Tx 0: in_degree 0
    // Tx 1: in_degree 1 (depends on tx 0)
    assert_eq!(graph.in_degree(0), 0);
    assert_eq!(graph.in_degree(1), 1);

    let waves = WaveScheduler::schedule(&txs);
    assert_eq!(waves.len(), 5, "5 conflicting txs must be serialized into 5 waves");
    for (i, wave) in waves.iter().enumerate() {
        assert_eq!(wave.tx_indices, vec![i]);
    }
}

#[test]
fn test_read_after_write_conflict() {
    let writer_acc = Pubkey::from_u64(50);
    let tx0 = make_dummy_tx(Pubkey::from_u64(1), vec![writer_acc], vec![]);
    let tx1 = make_dummy_tx(Pubkey::from_u64(2), vec![], vec![writer_acc]); // Reads writer_acc

    let graph = ConflictGraph::build(&[tx0, tx1]);
    assert_eq!(graph.in_degree(0), 0);
    assert_eq!(graph.in_degree(1), 1, "Reader should depend on previous writer");

    let waves = WaveScheduler::schedule(&[
        make_dummy_tx(Pubkey::from_u64(1), vec![writer_acc], vec![]),
        make_dummy_tx(Pubkey::from_u64(2), vec![], vec![writer_acc]),
    ]);
    assert_eq!(waves.len(), 2, "RAW conflict must separate into 2 waves");
}

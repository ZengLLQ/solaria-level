use solaria_level::{
    storage::Bank,
    types::{Account, Pubkey},
};
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

#[test]
fn test_concurrent_bank_sharding_stress() {
    let bank = Arc::new(Bank::new());
    let num_threads = 8;
    let accounts_per_thread = 50;

    // Concurrently create accounts across multiple shards
    let mut handles = Vec::new();
    for t in 0..num_threads {
        let bank_clone = bank.clone();
        handles.push(thread::spawn(move || {
            let mut batch = HashMap::new();
            for i in 0..accounts_per_thread {
                let pk = Pubkey::from_u64((t * 1000 + i) as u64);
                let acc = Account::new(100, 0, Pubkey::SYSTEM_PROGRAM);
                batch.insert(pk, acc);
            }
            bank_clone.commit_batch(batch, 10);
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Expected accounts: 8 * 50 = 400
    // Expected lamports in accounts: 400 * 100 = 40,000
    // Expected fees: 8 * 10 = 80
    // Total supply = 40,080
    let total = bank.calculate_total_lamports();
    assert_eq!(total, 40_080);
    assert_eq!(bank.get_collected_fees(), 80);
    assert!(bank.verify_supply_invariant(40_080));
}

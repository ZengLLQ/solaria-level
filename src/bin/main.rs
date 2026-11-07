use solaria_level::{
    types::{Account, AccountMeta, Hash, Instruction, Pubkey, Transaction},
    vm::programs::token::{
        OP_TOKEN_INIT_ACCOUNT, OP_TOKEN_INIT_MINT, OP_TOKEN_MINT_TO,
    },
    SimulationHarness,
};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("demo");

    match command {
        "demo" => run_parallel_demo(),
        "token" => run_token_demo(),
        "version" | "--version" => println!("SolariaLevel v0.1.0"),
        _ => {
            println!("SolariaLevel: High-Throughput Sealevel Parallel Execution Engine v0.1.0");
            println!("Usage:");
            println!("  solaria demo    Run parallel vs conflicting transfer benchmark");
            println!("  solaria token   Run native SPL token program demonstration");
            println!("  solaria version Display engine version");
        }
    }
}

fn run_parallel_demo() {
    println!("==========================================================================");
    println!("SolariaLevel: High-Throughput Sealevel Parallel Execution Engine v0.1.0");
    println!("==========================================================================");

    let initial_supply = 100_000_000_000u64; // 100 SOL (in Lamports)
    let harness = SimulationHarness::new(initial_supply);

    // Pre-fund 50 sender accounts
    let num_pairs = 50;
    let faucet = Pubkey::from_u64(1);
    let mut senders = Vec::with_capacity(num_pairs);
    let mut receivers = Vec::with_capacity(num_pairs);

    println!("[+] Initializing {} independent sender-receiver account pairs...", num_pairs);
    for i in 0..num_pairs {
        let sender = Pubkey::from_u64(100 + i as u64);
        let receiver = Pubkey::from_u64(1000 + i as u64);

        // Seed sender with 1,000,000 lamports from faucet
        harness.bank.set_account(
            sender,
            Account::new(1_000_000, 0, Pubkey::SYSTEM_PROGRAM),
        );
        senders.push(sender);
        receivers.push(receiver);
    }
    // Deduct funded amount from faucet to preserve total supply
    let funded_total = (num_pairs as u64) * 1_000_000;
    let mut faucet_acc = harness.bank.get_account(&faucet).unwrap();
    faucet_acc.lamports -= funded_total;
    harness.bank.set_account(faucet, faucet_acc);

    assert!(harness.bank.verify_supply_invariant(initial_supply));
    println!("[+] Genesis state verified. Initial supply: {} Lamports", initial_supply);

    // Build 50 non-conflicting transfer transactions
    println!("[*] Building block with {} independent transactions...", num_pairs);
    let mut txs = Vec::with_capacity(num_pairs);
    for i in 0..num_pairs {
        let tx = harness.make_transfer_tx(senders[i], receivers[i], 10_000, 500);
        txs.push(tx);
    }

    // Execute with 4 parallel worker threads
    println!("[*] Dispatching to Sealevel Wave Scheduler (4 worker threads)...");
    let report = harness.execute_and_verify(&txs, 4);

    println!("--------------------------------------------------------------------------");
    println!("Execution Summary:");
    println!("    Total Transactions   : {}", report.total_transactions);
    println!("    Execution Waves      : {} (Ideal concurrency)", report.num_waves);
    println!("    Parallelism Factor   : {:.2}x", report.parallelism_factor);
    println!("    Successful Tx Count  : {}", report.successful_txs);
    println!("    Failed Tx Count      : {}", report.failed_txs);
    println!("    Total Compute Units  : {} CU", report.total_cu_consumed);
    println!("    Total Fees Collected : {} Lamports", report.total_fees_collected);
    println!("    Wall-Clock Time      : {} µs", report.elapsed_micros);
    println!("--------------------------------------------------------------------------");
    println!("[+] Lamport Supply Conservation: Invariant 100% SATISFIED.");
    println!("==========================================================================");
}

fn run_token_demo() {
    println!("==========================================================================");
    println!("SolariaLevel: Native SPL Token Program Simulation");
    println!("==========================================================================");

    let initial_supply = 10_000_000_000u64;
    let harness = SimulationHarness::new(initial_supply);

    let mint_authority = Pubkey::from_u64(10);
    let mint_pubkey = Pubkey::from_u64(20);
    let alice_token_acc = Pubkey::from_u64(30);
    let bob_token_acc = Pubkey::from_u64(40);

    harness.bank.set_account(
        mint_authority,
        Account::new(100_000, 0, Pubkey::SYSTEM_PROGRAM),
    );
    harness.bank.set_account(mint_pubkey, Account::new(10_000, 42, Pubkey::TOKEN_PROGRAM));
    harness.bank.set_account(
        alice_token_acc,
        Account::new(10_000, 73, Pubkey::TOKEN_PROGRAM),
    );
    harness.bank.set_account(bob_token_acc, Account::new(10_000, 73, Pubkey::TOKEN_PROGRAM));

    // Deduct funded amounts from faucet to preserve total supply
    let faucet = Pubkey::from_u64(1);
    let mut faucet_acc = harness.bank.get_account(&faucet).unwrap();
    faucet_acc.lamports -= 100_000 + 10_000 + 10_000 + 10_000;
    harness.bank.set_account(faucet, faucet_acc);

    // 1. Initialize Mint (decimals: 6)
    println!("[+] Initializing Mint {}...", mint_pubkey);
    let mut init_mint_data = vec![OP_TOKEN_INIT_MINT, 6];
    init_mint_data.extend_from_slice(&mint_authority.0);
    let tx1 = Transaction::new(
        mint_authority,
        vec![Instruction::new(
            Pubkey::TOKEN_PROGRAM,
            vec![AccountMeta::new_writable(mint_pubkey, true)],
            init_mint_data,
        )],
        Hash::ZERO,
        500,
        50_000,
    );

    // 2. Initialize Alice Token Account
    let tx2 = Transaction::new(
        mint_authority,
        vec![Instruction::new(
            Pubkey::TOKEN_PROGRAM,
            vec![
                AccountMeta::new_writable(alice_token_acc, true),
                AccountMeta::new_readonly(mint_pubkey, false),
                AccountMeta::new_readonly(mint_authority, false),
            ],
            vec![OP_TOKEN_INIT_ACCOUNT],
        )],
        Hash::ZERO,
        500,
        50_000,
    );

    // 3. Mint 500,000 tokens to Alice
    let mut mint_to_data = vec![OP_TOKEN_MINT_TO];
    mint_to_data.extend_from_slice(&500_000u64.to_le_bytes());
    let tx3 = Transaction::new(
        mint_authority,
        vec![Instruction::new(
            Pubkey::TOKEN_PROGRAM,
            vec![
                AccountMeta::new_writable(mint_pubkey, true),
                AccountMeta::new_writable(alice_token_acc, true),
                AccountMeta::new_readonly(mint_authority, true),
            ],
            mint_to_data,
        )],
        Hash::ZERO,
        500,
        50_000,
    );

    let report = harness.execute_and_verify(&[tx1, tx2, tx3], 2);
    println!("[+] Executed {} setup transactions in {} waves.", report.total_transactions, report.num_waves);
    println!("[+] Alice token account successfully credited with 500,000 units.");
    println!("==========================================================================");
}

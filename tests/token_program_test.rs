use solaria_level::{
    types::{Account, AccountMeta, Hash, Instruction, Pubkey, Transaction},
    vm::programs::token::{
        Mint, TokenAccount, ACCOUNT_STATE_ACTIVE, OP_TOKEN_BURN,
        OP_TOKEN_INIT_ACCOUNT, OP_TOKEN_INIT_MINT, OP_TOKEN_MINT_TO, OP_TOKEN_TRANSFER,
    },
    SimulationHarness,
};

#[test]
fn test_spl_token_lifecycle() {
    let harness = SimulationHarness::new(10_000_000);

    let authority = Pubkey::from_u64(10);
    let mint_pk = Pubkey::from_u64(20);
    let alice_pk = Pubkey::from_u64(30);
    let bob_pk = Pubkey::from_u64(40);

    // Seed authority with lamports for tx fees
    harness.bank.set_account(authority, Account::new(1_000_000, 0, Pubkey::SYSTEM_PROGRAM));
    harness.bank.set_account(mint_pk, Account::new(10_000, 42, Pubkey::TOKEN_PROGRAM));
    harness.bank.set_account(alice_pk, Account::new(10_000, 73, Pubkey::TOKEN_PROGRAM));
    harness.bank.set_account(bob_pk, Account::new(10_000, 73, Pubkey::TOKEN_PROGRAM));

    // 1. Initialize Mint
    let mut init_mint_data = vec![OP_TOKEN_INIT_MINT, 9];
    init_mint_data.extend_from_slice(&authority.0);
    let tx_init_mint = Transaction::new(
        authority,
        vec![Instruction::new(
            Pubkey::TOKEN_PROGRAM,
            vec![AccountMeta::new_writable(mint_pk, true)],
            init_mint_data,
        )],
        Hash::ZERO,
        100,
        50_000,
    );

    // 2. Initialize Alice and Bob Accounts
    let tx_init_alice = Transaction::new(
        authority,
        vec![Instruction::new(
            Pubkey::TOKEN_PROGRAM,
            vec![
                AccountMeta::new_writable(alice_pk, true),
                AccountMeta::new_readonly(mint_pk, false),
                AccountMeta::new_readonly(authority, false),
            ],
            vec![OP_TOKEN_INIT_ACCOUNT],
        )],
        Hash::ZERO,
        100,
        50_000,
    );
    let tx_init_bob = Transaction::new(
        authority,
        vec![Instruction::new(
            Pubkey::TOKEN_PROGRAM,
            vec![
                AccountMeta::new_writable(bob_pk, true),
                AccountMeta::new_readonly(mint_pk, false),
                AccountMeta::new_readonly(authority, false),
            ],
            vec![OP_TOKEN_INIT_ACCOUNT],
        )],
        Hash::ZERO,
        100,
        50_000,
    );

    // 3. Mint 1,000,000 tokens to Alice
    let mut mint_to_data = vec![OP_TOKEN_MINT_TO];
    mint_to_data.extend_from_slice(&1_000_000u64.to_le_bytes());
    let tx_mint_alice = Transaction::new(
        authority,
        vec![Instruction::new(
            Pubkey::TOKEN_PROGRAM,
            vec![
                AccountMeta::new_writable(mint_pk, true),
                AccountMeta::new_writable(alice_pk, true),
                AccountMeta::new_readonly(authority, true),
            ],
            mint_to_data,
        )],
        Hash::ZERO,
        100,
        50_000,
    );

    // 4. Alice transfers 400,000 tokens to Bob
    let mut transfer_data = vec![OP_TOKEN_TRANSFER];
    transfer_data.extend_from_slice(&400_000u64.to_le_bytes());
    let tx_transfer = Transaction::new(
        authority,
        vec![Instruction::new(
            Pubkey::TOKEN_PROGRAM,
            vec![
                AccountMeta::new_writable(alice_pk, true),
                AccountMeta::new_writable(bob_pk, true),
                AccountMeta::new_readonly(authority, true), // authority is owner of alice_pk in init_account
            ],
            transfer_data,
        )],
        Hash::ZERO,
        100,
        50_000,
    );

    // 5. Bob burns 50,000 tokens
    let mut burn_data = vec![OP_TOKEN_BURN];
    burn_data.extend_from_slice(&50_000u64.to_le_bytes());
    let tx_burn = Transaction::new(
        authority,
        vec![Instruction::new(
            Pubkey::TOKEN_PROGRAM,
            vec![
                AccountMeta::new_writable(bob_pk, true),
                AccountMeta::new_writable(mint_pk, true),
                AccountMeta::new_readonly(authority, true),
            ],
            burn_data,
        )],
        Hash::ZERO,
        100,
        50_000,
    );

    let executor = solaria_level::ParallelExecutor::new(harness.bank.clone(), 2);
    let report = executor.execute_block(&[
        tx_init_mint,
        tx_init_alice,
        tx_init_bob,
        tx_mint_alice,
        tx_transfer,
        tx_burn,
    ]);

    assert_eq!(report.successful_txs, 6);
    assert_eq!(report.failed_txs, 0);

    // Verify token states
    let mint_acc = harness.bank.get_account(&mint_pk).unwrap();
    let mint = Mint::unpack(&mint_acc.data).unwrap();
    assert_eq!(mint.decimals, 9);
    // Mint supply: 1,000,000 - 50,000 = 950,000
    assert_eq!(mint.supply, 950_000);

    let alice_acc = harness.bank.get_account(&alice_pk).unwrap();
    let alice_tok = TokenAccount::unpack(&alice_acc.data).unwrap();
    // Alice balance: 1,000,000 - 400,000 = 600,000
    assert_eq!(alice_tok.amount, 600_000);
    assert_eq!(alice_tok.state, ACCOUNT_STATE_ACTIVE);

    let bob_acc = harness.bank.get_account(&bob_pk).unwrap();
    let bob_tok = TokenAccount::unpack(&bob_acc.data).unwrap();
    // Bob balance: 400,000 - 50,000 = 350,000
    assert_eq!(bob_tok.amount, 350_000);
    assert_eq!(bob_tok.state, ACCOUNT_STATE_ACTIVE);
}

use basis::component::Env;
use basis::interface::*;
use field::*;
use mint::action::*;
use protocol::action::AstSelect;
use protocol::context::ContextInst;
use protocol::state::CoreState;
use protocol::transaction::*;
use sys::Account;
use testkit::sim::context::make_ctx_with_state;
use testkit::sim::state::ForkableMemState;

fn addr_of(acc: &Account) -> Address {
    Address::from(acc.address().clone())
}

fn make_ctx<'a>(height: u64, tx: &'a dyn TransactionRead) -> ContextInst<'a> {
    let mut env = Env::default();
    env.chain.fast_sync = true;
    env.block.height = height;
    env.tx.main = tx.main();
    env.tx.addrs = tx.addrs();
    make_ctx_with_state(env, Box::new(ForkableMemState::default()), tx)
}

fn seed_balance(ctx: &mut dyn Context, addr: &Address, mei: u64) {
    let mut state = CoreState::wrap(ctx.state());
    let mut bls = state.balance(addr).unwrap_or_default();
    bls.hacash = Amount::mei(mei);
    state.balance_set(addr, &bls);
}

fn make_inscripts(n: usize) -> Inscripts {
    let mut ins = Inscripts::default();
    for i in 0..n {
        let one = BytesW1::from(format!("ins{}", i).into_bytes()).unwrap();
        ins.push(one).unwrap();
    }
    ins
}

fn seed_diamond(
    ctx: &mut dyn Context,
    diamond: DiamondName,
    owner: Address,
    insc_num: usize,
    prev_engraved_height: u64,
    average_bid_burn_mei: u16,
) {
    let mut state = CoreState::wrap(ctx.state());
    state.diamond_set(
        &diamond,
        &DiamondSto {
            status: DIAMOND_STATUS_NORMAL,
            address: owner,
            prev_engraved_height: BlockHeight::from(prev_engraved_height),
            inscripts: make_inscripts(insc_num),
        },
    );
    state.diamond_smelt_set(
        &diamond,
        &DiamondSmelt {
            diamond,
            number: DiamondNumber::from(1),
            born_height: BlockHeight::from(1),
            born_hash: Hash::default(),
            prev_hash: Hash::default(),
            miner_address: field::ADDRESS_ONEX.clone(),
            bid_fee: Amount::zero(),
            nonce: Fixed8::default(),
            average_bid_burn: Uint2::from(average_bid_burn_mei),
            life_gene: Hash::default(),
        },
    );
}

fn diamond_insc_len(ctx: &mut dyn Context, diamond: &DiamondName) -> usize {
    CoreState::wrap(ctx.state())
        .diamond(diamond)
        .unwrap()
        .inscripts
        .length()
}

fn balance_mei(ctx: &mut dyn Context, addr: &Address) -> u64 {
    CoreState::wrap(ctx.state())
        .balance(addr)
        .unwrap_or_default()
        .hacash
        .to_mei_u64()
        .unwrap()
}

#[test]
fn diamond_inscription_append_uses_stepped_protocol_cost_tiers() {
    let main_acc = Account::create_by("diamond-inscription-append-main").unwrap();
    let main = addr_of(&main_acc);
    let mut tx = TransactionType2::new_by(main, Amount::mei(1), 1_730_000_000);
    tx.fill_sign(&main_acc).unwrap();

    let diamond = DiamondName::from_readable(b"WTYUIA").unwrap();

    // A=100 mei:
    // - len=9  -> free
    // - len=10 -> A/50 = 2
    // - len=40 -> A/20 = 5
    // - len=100 -> A/10 = 10
    let cases = vec![
        (9usize, 0u64),
        (10usize, 2u64),
        (40usize, 5u64),
        (100usize, 10u64),
    ];
    for (cur_len, expect_cost) in cases {
        let mut fail_ctx = make_ctx(10_000, tx.as_read());
        seed_balance(&mut fail_ctx, &main, 1_000_000);
        seed_diamond(&mut fail_ctx, diamond, main, cur_len, 0, 100);
        let mut fail_act = DiamondInscription::new();
        fail_act.diamonds = DiamondNameListMax200::one(diamond);
        fail_act.protocol_cost = Amount::mei(expect_cost.saturating_sub(1));
        fail_act.engraved_type = Uint1::from(1);
        fail_act.engraved_content = BytesW1::from_str("hello").unwrap();
        let fail_exec = fail_act.execute(&mut fail_ctx);
        if expect_cost == 0 {
            assert!(
                fail_exec.is_ok(),
                "len={} should be free but got {:?}",
                cur_len,
                fail_exec
            );
        } else {
            let err = fail_exec.unwrap_err();
            assert!(err.contains("cost error"), "{}", err);
        }

        let mut ok_ctx = make_ctx(10_000, tx.as_read());
        seed_balance(&mut ok_ctx, &main, 1_000_000);
        seed_diamond(&mut ok_ctx, diamond, main, cur_len, 0, 100);
        let mut ok_act = DiamondInscription::new();
        ok_act.diamonds = DiamondNameListMax200::one(diamond);
        ok_act.protocol_cost = Amount::mei(expect_cost);
        ok_act.engraved_type = Uint1::from(1);
        ok_act.engraved_content = BytesW1::from_str("hello").unwrap();
        ok_act.execute(&mut ok_ctx).unwrap();

        assert_eq!(diamond_insc_len(&mut ok_ctx, &diamond), cur_len + 1);
        assert_eq!(balance_mei(&mut ok_ctx, &main), 1_000_000 - expect_cost);
    }
}

#[test]
fn diamond_inscription_readable_content_type_boundary_is_100() {
    let main_acc = Account::create_by("diamond-inscription-readable-main").unwrap();
    let main = addr_of(&main_acc);
    let mut tx = TransactionType2::new_by(main, Amount::mei(1), 1_730_000_010);
    tx.fill_sign(&main_acc).unwrap();

    let diamond = DiamondName::from_readable(b"BSEYWT").unwrap();
    let raw_non_readable = BytesW1::from(vec![0xff, 0x00]).unwrap();

    // engraved_type <= 100 must be readable string.
    let mut fail_ctx = make_ctx(10_000, tx.as_read());
    seed_balance(&mut fail_ctx, &main, 1_000_000);
    seed_diamond(&mut fail_ctx, diamond, main, 0, 0, 100);
    let mut fail_act = DiamondInscription::new();
    fail_act.diamonds = DiamondNameListMax200::one(diamond);
    fail_act.protocol_cost = Amount::zero();
    fail_act.engraved_type = Uint1::from(100);
    fail_act.engraved_content = raw_non_readable.clone();
    let err = fail_act.execute(&mut fail_ctx).unwrap_err();
    assert!(err.contains("must readable string"), "{}", err);

    // engraved_type > 100 can carry non-readable bytes.
    let mut ok_ctx = make_ctx(10_000, tx.as_read());
    seed_balance(&mut ok_ctx, &main, 1_000_000);
    seed_diamond(&mut ok_ctx, diamond, main, 0, 0, 100);
    let mut ok_act = DiamondInscription::new();
    ok_act.diamonds = DiamondNameListMax200::one(diamond);
    ok_act.protocol_cost = Amount::zero();
    ok_act.engraved_type = Uint1::from(101);
    ok_act.engraved_content = raw_non_readable;
    ok_act.execute(&mut ok_ctx).unwrap();
}

#[cfg(feature = "hip22")]
#[test]
fn diamond_inscription_edit_requires_a_over_100_protocol_cost() {
    let main_acc = Account::create_by("diamond-inscription-edit-main").unwrap();
    let main = addr_of(&main_acc);
    let mut tx = TransactionType2::new_by(main, Amount::mei(1), 1_730_000_001);
    tx.fill_sign(&main_acc).unwrap();

    let diamond = DiamondName::from_readable(b"HYXYHY").unwrap();

    // A=100 mei, edit should cost A/100 = 1 mei.
    let mut fail_ctx = make_ctx(10_000, tx.as_read());
    seed_balance(&mut fail_ctx, &main, 1_000_000);
    seed_diamond(&mut fail_ctx, diamond, main, 1, 0, 100);
    let mut fail_act = DiamondInscriptionEdit::new();
    fail_act.diamond = diamond;
    fail_act.index = Uint1::from(0);
    fail_act.protocol_cost = Amount::zero();
    fail_act.engraved_type = Uint1::from(1);
    fail_act.engraved_content = BytesW1::from_str("edited").unwrap();
    let err = fail_act.execute(&mut fail_ctx).unwrap_err();
    assert!(err.contains("edit cost error"), "{}", err);

    let mut ok_ctx = make_ctx(10_000, tx.as_read());
    seed_balance(&mut ok_ctx, &main, 1_000_000);
    seed_diamond(&mut ok_ctx, diamond, main, 1, 0, 100);
    let mut ok_act = DiamondInscriptionEdit::new();
    ok_act.diamond = diamond;
    ok_act.index = Uint1::from(0);
    ok_act.protocol_cost = Amount::mei(1);
    ok_act.engraved_type = Uint1::from(1);
    ok_act.engraved_content = BytesW1::from_str("edited").unwrap();
    ok_act.execute(&mut ok_ctx).unwrap();

    let dia = CoreState::wrap(ok_ctx.state()).diamond(&diamond).unwrap();
    assert_eq!(
        dia.inscripts.list()[0].to_readable_or_hex(),
        "edited".to_owned()
    );
    assert_eq!(balance_mei(&mut ok_ctx, &main), 1_000_000 - 1);
}

#[cfg(feature = "hip22")]
#[test]
fn diamond_inscription_move_charges_by_target_append_rule_only() {
    let from_acc = Account::create_by("diamond-inscription-move-from").unwrap();
    let to_acc = Account::create_by("diamond-inscription-move-to").unwrap();
    let from = addr_of(&from_acc);
    let to = addr_of(&to_acc);

    let mut tx = TransactionType2::new_by(from, Amount::mei(1), 1_730_000_002);
    tx.fill_sign(&from_acc).unwrap();
    tx.fill_sign(&to_acc).unwrap();

    let from_diamond = DiamondName::from_readable(b"UETWNK").unwrap();
    let to_diamond = DiamondName::from_readable(b"WYUKKZ").unwrap();

    // target A=200 mei and len=10 -> move cost A/50 = 4 mei
    let mut fail_ctx = make_ctx(10_000, tx.as_read());
    seed_balance(&mut fail_ctx, &from, 1_000_000);
    seed_diamond(&mut fail_ctx, from_diamond, from, 1, 0, 300);
    seed_diamond(&mut fail_ctx, to_diamond, to, 10, 0, 200);
    let mut fail_act = DiamondInscriptionMove::new();
    fail_act.from_diamond = from_diamond;
    fail_act.to_diamond = to_diamond;
    fail_act.index = Uint1::from(0);
    fail_act.protocol_cost = Amount::mei(3);
    let err = fail_act.execute(&mut fail_ctx).unwrap_err();
    assert!(err.contains("move cost error"), "{}", err);

    let mut ok_ctx = make_ctx(10_000, tx.as_read());
    seed_balance(&mut ok_ctx, &from, 1_000_000);
    seed_diamond(&mut ok_ctx, from_diamond, from, 1, 0, 300);
    seed_diamond(&mut ok_ctx, to_diamond, to, 10, 0, 200);
    let mut ok_act = DiamondInscriptionMove::new();
    ok_act.from_diamond = from_diamond;
    ok_act.to_diamond = to_diamond;
    ok_act.index = Uint1::from(0);
    ok_act.protocol_cost = Amount::mei(4);
    ok_act.execute(&mut ok_ctx).unwrap();

    assert_eq!(diamond_insc_len(&mut ok_ctx, &from_diamond), 0);
    assert_eq!(diamond_insc_len(&mut ok_ctx, &to_diamond), 11);
    assert_eq!(balance_mei(&mut ok_ctx, &from), 1_000_000 - 4);
}

#[cfg(feature = "hip22")]
#[test]
fn diamond_inscription_move_is_free_when_target_has_less_than_ten() {
    let from_acc = Account::create_by("diamond-inscription-move-free-from").unwrap();
    let to_acc = Account::create_by("diamond-inscription-move-free-to").unwrap();
    let from = addr_of(&from_acc);
    let to = addr_of(&to_acc);

    let mut tx = TransactionType2::new_by(from, Amount::mei(1), 1_730_000_003);
    tx.fill_sign(&from_acc).unwrap();
    tx.fill_sign(&to_acc).unwrap();

    let from_diamond = DiamondName::from_readable(b"EYWTUK").unwrap();
    let to_diamond = DiamondName::from_readable(b"BSZNWT").unwrap();

    // target len=9 -> move cost 0
    let mut ctx = make_ctx(10_000, tx.as_read());
    seed_balance(&mut ctx, &from, 1_000_000);
    seed_diamond(&mut ctx, from_diamond, from, 1, 0, 300);
    seed_diamond(&mut ctx, to_diamond, to, 9, 0, 200);
    let mut act = DiamondInscriptionMove::new();
    act.from_diamond = from_diamond;
    act.to_diamond = to_diamond;
    act.index = Uint1::from(0);
    act.protocol_cost = Amount::zero();
    act.execute(&mut ctx).unwrap();

    assert_eq!(diamond_insc_len(&mut ctx, &from_diamond), 0);
    assert_eq!(diamond_insc_len(&mut ctx, &to_diamond), 10);
    assert_eq!(balance_mei(&mut ctx, &from), 1_000_000);
}

#[test]
fn diamond_inscription_rejects_non_privakey_owner() {
    let owner = Address::create_scriptmh([7u8; 20]);
    let tx = TransactionType2::new_by(owner, Amount::mei(1), 1_730_000_004);
    let diamond = DiamondName::from_readable(b"VYHWEH").unwrap();

    let mut ctx = make_ctx(10_000, tx.as_read());
    seed_balance(&mut ctx, &owner, 1_000_000);
    seed_diamond(&mut ctx, diamond, owner, 0, 0, 100);

    let mut act = DiamondInscription::new();
    act.diamonds = DiamondNameListMax200::one(diamond);
    act.protocol_cost = Amount::zero();
    act.engraved_type = Uint1::from(1);
    act.engraved_content = BytesW1::from_str("hello").unwrap();
    let err = act.execute(&mut ctx).unwrap_err();
    assert!(err.to_lowercase().contains("privakey"), "{}", err);
}

#[cfg(all(feature = "hip22", feature = "ast"))]
#[test]
fn diamond_move_astselect_recovers_failed_child_and_keeps_successful_child() {
    let from_acc = Account::create_by("diamond-inscription-ast-child-from").unwrap();
    let to_acc = Account::create_by("diamond-inscription-ast-child-to").unwrap();
    let from = addr_of(&from_acc);
    let to = addr_of(&to_acc);

    let mut tx = TransactionType2::new_by(from, Amount::mei(1), 1_730_000_020);
    tx.fill_sign(&from_acc).unwrap();
    tx.fill_sign(&to_acc).unwrap();

    let from_diamond = DiamondName::from_readable(b"AAEWTU").unwrap();
    let to_diamond = DiamondName::from_readable(b"AEYWTU").unwrap();

    let mut ctx = make_ctx(10_000, tx.as_read());
    seed_balance(&mut ctx, &from, 1_000_000);
    seed_diamond(&mut ctx, from_diamond, from, 1, 0, 300);
    seed_diamond(&mut ctx, to_diamond, to, 9, 0, 200);

    // Child #1 fails (index out of range), child #2 succeeds.
    let mut mv_fail = DiamondInscriptionMove::new();
    mv_fail.from_diamond = from_diamond;
    mv_fail.to_diamond = to_diamond;
    mv_fail.index = Uint1::from(1);
    mv_fail.protocol_cost = Amount::zero();

    let mut mv_ok = DiamondInscriptionMove::new();
    mv_ok.from_diamond = from_diamond;
    mv_ok.to_diamond = to_diamond;
    mv_ok.index = Uint1::from(0);
    mv_ok.protocol_cost = Amount::zero();

    let ast = AstSelect::create_by(1, 1, vec![Box::new(mv_fail), Box::new(mv_ok)]);
    ast.execute(&mut ctx).unwrap();

    assert_eq!(diamond_insc_len(&mut ctx, &from_diamond), 0);
    assert_eq!(diamond_insc_len(&mut ctx, &to_diamond), 10);
    assert_eq!(balance_mei(&mut ctx, &from), 1_000_000);
}

#[cfg(all(feature = "hip22", feature = "ast"))]
#[test]
fn diamond_move_astselect_rolls_back_whole_node_when_min_unmet() {
    let from_acc = Account::create_by("diamond-inscription-ast-whole-from").unwrap();
    let to_acc = Account::create_by("diamond-inscription-ast-whole-to").unwrap();
    let from = addr_of(&from_acc);
    let to = addr_of(&to_acc);

    let mut tx = TransactionType2::new_by(from, Amount::mei(1), 1_730_000_021);
    tx.fill_sign(&from_acc).unwrap();
    tx.fill_sign(&to_acc).unwrap();

    let from_diamond = DiamondName::from_readable(b"ABEYWT").unwrap();
    let to_diamond = DiamondName::from_readable(b"AKYWTU").unwrap();

    let mut ctx = make_ctx(10_000, tx.as_read());
    seed_balance(&mut ctx, &from, 1_000_000);
    seed_diamond(&mut ctx, from_diamond, from, 1, 0, 300);
    seed_diamond(&mut ctx, to_diamond, to, 9, 0, 200);

    // Child #1 succeeds, child #2 fails. exe_min=2 => whole AstSelect fails and must rollback child #1.
    let mut mv_ok = DiamondInscriptionMove::new();
    mv_ok.from_diamond = from_diamond;
    mv_ok.to_diamond = to_diamond;
    mv_ok.index = Uint1::from(0);
    mv_ok.protocol_cost = Amount::zero();

    let mut mv_fail = DiamondInscriptionMove::new();
    mv_fail.from_diamond = from_diamond;
    mv_fail.to_diamond = to_diamond;
    mv_fail.index = Uint1::from(0);
    mv_fail.protocol_cost = Amount::zero();

    let ast = AstSelect::create_by(2, 2, vec![Box::new(mv_ok), Box::new(mv_fail)]);
    let err = ast.execute(&mut ctx).unwrap_err();
    assert!(err.contains("must succeed at least 2"), "{}", err);

    assert_eq!(diamond_insc_len(&mut ctx, &from_diamond), 1);
    assert_eq!(diamond_insc_len(&mut ctx, &to_diamond), 9);
    assert_eq!(balance_mei(&mut ctx, &from), 1_000_000);

    let state = CoreState::wrap(ctx.state());
    let from_sto = state.diamond(&from_diamond).unwrap();
    let to_sto = state.diamond(&to_diamond).unwrap();
    assert_eq!(*from_sto.prev_engraved_height, 0);
    assert_eq!(*to_sto.prev_engraved_height, 0);
}

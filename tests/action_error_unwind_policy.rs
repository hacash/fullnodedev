#![cfg(feature = "vm")]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use basis::interface::{ActExec, Context, State, StateOperat, TransactionRead};
use field::*;
use mint::action::ChannelOpen;
use protocol::action::{ChainAllow, ChainIDList, HeightScope};
use protocol::operate::{
    asset_check, asset_sub, asset_transfer, check_diamond_status, diamond_owned_move, hac_check,
    hac_sub, hacd_sub, hacd_transfer, sat_check, sat_sub, sat_transfer,
};
use protocol::state::CoreState;
use sys::{XError, XRet, TError, IntoXRet, Ret, UNWIND_PREFIX};
use testkit::sim::integration::{
    make_ctx_from_tx as make_ctx, make_stub_tx as make_tx, test_guard, vm_alt_addr as alt_addr,
    vm_main_addr as main_addr,
};
use testkit::sim::logs::MemLogs;
use testkit::sim::state::FlatMemState as StateMem;
use vm::machine::check_vm_return_value;
use vm::rt::{ItrErr, ItrErrCode};
use vm::value::{CompoItem, Value};

fn mk_ctx<'a>(
    height: u64,
    chain_id: u64,
    tx: &'a dyn TransactionRead,
    state: Box<dyn State>,
) -> protocol::context::ContextInst<'a> {
    let mut ctx = make_ctx(height, tx, state, Box::new(MemLogs::default()));
    ctx.env.chain.fast_sync = true;
    ctx.env.chain.id = chain_id as u32;
    ctx
}

fn expect_unwind_ret<T>(ret: Ret<T>, msg: &str) -> XError {
    let err = match ret.into_xret() {
        Ok(_) => panic!("expect unwind error but got Ok"),
        Err(err) => err,
    };
    assert!(err.is_unwind(), "expect unwind but got {err}");
    if !msg.is_empty() {
        assert!(err.contains(msg), "expect '{msg}' in '{err}'");
    }
    err
}

fn expect_unwind_bret<T>(ret: XRet<T>, msg: &str) -> XError {
    let err = match ret {
        Ok(_) => panic!("expect unwind error but got Ok"),
        Err(err) => err,
    };
    assert!(err.is_unwind(), "expect unwind but got {err}");
    if !msg.is_empty() {
        assert!(err.contains(msg), "expect '{msg}' in '{err}'");
    }
    err
}

fn expect_interrupt_bret<T>(ret: XRet<T>, msg: &str) -> XError {
    let err = match ret {
        Ok(_) => panic!("expect interrupt error but got Ok"),
        Err(err) => err,
    };
    assert!(err.is_interrupt(), "expect interrupt but got {err}");
    if !msg.is_empty() {
        assert!(err.contains(msg), "expect '{msg}' in '{err}'");
    }
    err
}

fn expect_interrupt_ret<T>(ret: Ret<T>, msg: &str) -> XError {
    let err = match ret.into_xret() {
        Ok(_) => panic!("expect interrupt error but got Ok"),
        Err(err) => err,
    };
    assert!(err.is_interrupt(), "expect interrupt but got {err}");
    if !msg.is_empty() {
        assert!(err.contains(msg), "expect '{msg}' in '{err}'");
    }
    err
}

fn seed_diamond_status(ctx: &mut dyn Context, name: DiamondName, owner: Address, status: Uint1) {
    let mut state = CoreState::wrap(ctx.state());
    state.diamond_set(
        &name,
        &DiamondSto {
            status,
            address: owner,
            prev_engraved_height: BlockHeight::from(0),
            inscripts: Inscripts::default(),
        },
    );
}

#[test]
fn guard_action_failures_are_unwind() {
    let _guard = test_guard();
    let main = main_addr();
    let tx = make_tx(3, main, vec![main], 17);
    let mut ctx = mk_ctx(100, 9, &tx, Box::new(StateMem::default()));

    let mut bad_range = HeightScope::new();
    bad_range.start = BlockHeight::from(20);
    bad_range.end = BlockHeight::from(10);
    expect_interrupt_bret(bad_range.execute(&mut ctx), "cannot big than");

    let mut out_of_range = HeightScope::new();
    out_of_range.start = BlockHeight::from(200);
    out_of_range.end = BlockHeight::from(300);
    expect_unwind_bret(out_of_range.execute(&mut ctx), "submit in height between");

    let mut allow = ChainAllow::new();
    allow.chains = ChainIDList::from_list(vec![Uint4::from(1), Uint4::from(2)]).unwrap();
    expect_unwind_bret(allow.execute(&mut ctx), "must belong to chains");
}

#[test]
fn state_data_business_failures_split_unwind_and_interrupt() {
    let _guard = test_guard();
    let main = main_addr();
    let alt = alt_addr();
    let tx = make_tx(3, main, vec![main, alt], 17);
    let mut ctx = mk_ctx(100, 1, &tx, Box::new(StateMem::default()));

    // HAC
    expect_interrupt_ret(hac_sub(&mut ctx, &main, &Amount::zero()), "not positive");
    expect_unwind_ret(hac_sub(&mut ctx, &main, &Amount::mei(1)), "insufficient");
    expect_unwind_ret(hac_check(&mut ctx, &main, &Amount::mei(1)), "insufficient");

    // BTC (Satoshi)
    expect_interrupt_ret(
        sat_check(&mut ctx, &main, &Satoshi::from(0)),
        "cannot empty",
    );
    expect_unwind_ret(sat_sub(&mut ctx, &main, &Satoshi::from(1)), "insufficient");
    expect_interrupt_ret(
        sat_transfer(&mut ctx, &main, &main, &Satoshi::from(1)),
        "cannot trs to self",
    );

    // ASSET
    let zero_asset = AssetAmt::from(7, 0).unwrap();
    expect_interrupt_ret(asset_check(&mut ctx, &main, &zero_asset), "cannot empty");
    {
        let mut state = CoreState::wrap(ctx.state());
        expect_unwind_ret(
            asset_sub(&mut state, &main, &AssetAmt::from(7, 1).unwrap()),
            "insufficient",
        );
    }
    expect_interrupt_ret(
        asset_transfer(&mut ctx, &main, &main, &AssetAmt::from(7, 1).unwrap()),
        "cannot trs to self",
    );

    // HACD
    {
        let mut state = CoreState::wrap(ctx.state());
        expect_unwind_ret(
            hacd_sub(&mut state, &main, &DiamondNumber::from(1)),
            "insufficient",
        );
    }
    let dlist_self = DiamondNameListMax200::one(DiamondName::from_readable(b"WTYUIA").unwrap());
    {
        let mut state = CoreState::wrap(ctx.state());
        expect_interrupt_ret(
            hacd_transfer(
                &mut state,
                &main,
                &main,
                &DiamondNumber::from(1),
                &dlist_self,
            ),
            "cannot transfer to self",
        );
    }

    let dia_mortgaged = DiamondName::from_readable(b"HXVMEK").unwrap();
    let dia_not_belong = DiamondName::from_readable(b"BSZNWT").unwrap();
    seed_diamond_status(
        &mut ctx,
        dia_mortgaged,
        main,
        DIAMOND_STATUS_LENDING_TO_SYSTEM,
    );
    seed_diamond_status(&mut ctx, dia_not_belong, main, DIAMOND_STATUS_NORMAL);
    {
        let mut state = CoreState::wrap(ctx.state());
        expect_unwind_ret(
            check_diamond_status(&mut state, &main, &dia_mortgaged),
            "mortgaged",
        );
        expect_unwind_ret(
            check_diamond_status(&mut state, &alt, &dia_not_belong),
            "not belong",
        );
        let miss = DiamondNameListMax200::one(DiamondName::from_readable(b"WTYUKB").unwrap());
        expect_interrupt_ret(
            diamond_owned_move(&mut state, &main, &alt, &miss),
            "not find",
        );
    }
}

#[test]
fn channel_open_insufficient_balance_is_unwind() {
    let _guard = test_guard();
    let main = main_addr();
    let alt = alt_addr();
    let tx = make_tx(3, main, vec![main, alt], 17);
    let mut ctx = mk_ctx(100, 1, &tx, Box::new(StateMem::default()));

    let mut open = ChannelOpen::new();
    open.channel_id = ChannelId::from([1u8; 16]);
    open.left_bill = AddrHac {
        address: main,
        amount: Amount::mei(50),
    };
    open.right_bill = AddrHac {
        address: alt,
        amount: Amount::mei(1),
    };

    expect_unwind_bret(open.execute(&mut ctx), "insufficient");
}

#[test]
fn vm_non_zero_return_code_is_unwind() {
    let _guard = test_guard();
    assert!(check_vm_return_value(&Value::nil(), "main call").is_ok());
    assert!(check_vm_return_value(&Value::u8(0), "main call").is_ok());
    expect_unwind_bret(
        check_vm_return_value(&Value::u8(7), "main call"),
        "return error code 7",
    );
}

#[test]
fn vm_non_numeric_return_is_unwind_with_stable_detail() {
    let _guard = test_guard();
    expect_unwind_bret(
        check_vm_return_value(&Value::bytes(b"bad".to_vec()), "main call"),
        "return error bytes \"bad\"",
    );
    expect_unwind_bret(
        check_vm_return_value(&Value::bytes(vec![0xff, 0x00]), "main call"),
        "return error bytes 0xff00",
    );
    let addr = main_addr();
    expect_unwind_bret(
        check_vm_return_value(&Value::Address(addr), "main call"),
        &format!("return error address {}", addr.to_readable()),
    );
    expect_interrupt_bret(
        check_vm_return_value(&Value::HeapSlice((0, 2)), "main call"),
        "return type HeapSlice is not supported",
    );
    expect_unwind_bret(
        check_vm_return_value(&Value::Compo(CompoItem::new_list()), "main call"),
        "return error object",
    );
}

#[test]
fn vm_throw_abort_and_action_pass_through_policy() {
    let _guard = test_guard();

    let throw_abort: XError = ItrErr(ItrErrCode::ThrowAbort, "contract abort".to_owned()).into();
    assert!(throw_abort.is_unwind(), "{throw_abort}");
    let throw_abort_wire: Error =
        ItrErr(ItrErrCode::ThrowAbort, "contract abort".to_owned()).into();
    assert!(
        throw_abort_wire.starts_with(UNWIND_PREFIX),
        "{throw_abort_wire}"
    );

    let action_unwind: XError =
        ItrErr(ItrErrCode::ActCallUnwind, "biz fail".to_owned()).into();
    assert!(action_unwind.is_unwind(), "{action_unwind}");
    assert!(action_unwind.contains("ActCallUnwind"), "{action_unwind}");
    let action_unwind_wire: Error =
        ItrErr(ItrErrCode::ActCallUnwind, "biz fail".to_owned()).into();
    assert!(
        action_unwind_wire.starts_with(UNWIND_PREFIX),
        "{action_unwind_wire}"
    );

    let action_interrupt: XError =
        ItrErr(ItrErrCode::ActCallError, "plain fail".to_owned()).into();
    assert!(action_interrupt.is_interrupt(), "{action_interrupt}");
    let action_interrupt_wire: Error =
        ItrErr(ItrErrCode::ActCallError, "plain fail".to_owned()).into();
    assert!(
        !action_interrupt_wire.starts_with(UNWIND_PREFIX),
        "{action_interrupt_wire}"
    );
}

#[test]
fn vm_itr_err_code_unwind_mapping_is_strict() {
    let _guard = test_guard();
    use ItrErrCode::*;

    // Keep this exhaustive list in sync with vm/src/rt/error.rs.
    let all_codes = [
        ContractError,
        NotFindContract,
        AbstTypeError,
        CodeTypeError,
        InheritError,
        LibraryError,
        ComplieError,
        ContractAddrErr,
        ContractUpgradeErr,
        CodeError,
        CodeTooLong,
        CodeOverflow,
        CodeEmpty,
        CodeNotWithEnd,
        JumpOverflow,
        JumpInDataSeg,
        IRNodeOverDepth,
        InstInvalid,
        InstDisabled,
        ActDisabled,
        InstNeverTouch,
        InstParamsErr,
        OutOfGas,
        OutOfStack,
        OutOfLocal,
        OutOfHeap,
        OutOfMemory,
        OutOfGlobal,
        OutOfCallDepth,
        OutOfLoadContract,
        OutOfValueSize,
        OutOfCompoLen,
        GasError,
        StackError,
        LocalError,
        HeapError,
        MemoryError,
        GlobalError,
        StorageError,
        LogError,
        CallNotExist,
        CallLibIdxOverflow,
        CallInvalid,
        CallExitInvalid,
        CallInCallcode,
        CallInAbst,
        CallOtherInMain,
        CallLocInView,
        CallInPure,
        CallOtherInP2sh,
        CallNoReturn,
        CallNotExternal,
        CallArgvTypeFail,
        CastFail,
        CastParamFail,
        CastBeKeyFail,
        CastBeUintFail,
        CastBeBytesFail,
        CastBeValueFail,
        CastBeFnArgvFail,
        CastBeCallDataFail,
        CompoOpInvalid,
        CompoOpOverflow,
        CompoToSerialize,
        CompoOpNotMatch,
        CompoPackError,
        CompoNoFindItem,
        Arithmetic,
        BytesHandle,
        NativeFuncError,
        NativeEnvError,
        ActCallError,
        ActCallUnwind,
        ItemNoSize,
        StorageKeyInvalid,
        StorageKeyNotFind,
        StorageExpired,
        StorageNotExpired,
        StoragePeriodErr,
        StorageValSizeErr,
        StorageRestoreNotMatch,
        ThrowAbort,
        NeverError,
    ];
    assert_eq!(all_codes.len(), 83);

    for code in all_codes {
        let xerr: XError = ItrErr(code, "x".to_owned()).into();
        let should_unwind = matches!(code, ThrowAbort | ActCallUnwind);
        assert_eq!(
            xerr.is_unwind(),
            should_unwind,
            "ItrErrCode::{:?} unwind mismatch: {}",
            code,
            xerr
        );
    }
}

fn normalized(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn walk_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {} failed: {}", dir.display(), e));
    for entry in entries {
        let entry =
            entry.unwrap_or_else(|e| panic!("read_dir entry failed in {}: {}", dir.display(), e));
        let path = entry.path();
        if path.is_dir() {
            walk_rs_files(&path, out);
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn scan_lines_with_patterns(patterns: &[&str]) -> BTreeMap<String, usize> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![];
    for rel in ["protocol/src", "mint/src", "vm/src"] {
        walk_rs_files(&root.join(rel), &mut files);
    }
    files.sort();

    let mut found = BTreeMap::new();
    for file in files {
        let rel = file.strip_prefix(root).unwrap().display().to_string();
        let src = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("read_to_string {} failed: {}", file.display(), e));
        for line in src.lines() {
            if patterns.iter().any(|pat| line.contains(pat)) {
                let key = format!("{}:{}", rel, normalized(line));
                *found.entry(key).or_insert(0) += 1;
            }
        }
    }
    found
}

fn expected_multiset(entries: &[&str]) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for e in entries {
        *out.entry((*e).to_owned()).or_insert(0) += 1;
    }
    out
}

fn expect_multiset_eq(
    label: &str,
    actual: &BTreeMap<String, usize>,
    expected: BTreeMap<String, usize>,
) {
    let mut missing = vec![];
    let mut extra = vec![];
    for (k, need) in &expected {
        let got = *actual.get(k).unwrap_or(&0);
        if got < *need {
            missing.push(format!("{} x{}", k, need - got));
        }
    }
    for (k, got) in actual {
        let need = *expected.get(k).unwrap_or(&0);
        if *got > need {
            extra.push(format!("{} x{}", k, got - need));
        }
    }
    if !missing.is_empty() || !extra.is_empty() {
        panic!(
            "{} mismatch\nmissing({}):\n{}\nextra({}):\n{}",
            label,
            missing.len(),
            missing.join("\n"),
            extra.len(),
            extra.join("\n")
        );
    }
}

#[test]
fn unwind_macro_callsites_are_allowlisted() {
    let _guard = test_guard();
    let actual = scan_lines_with_patterns(&["xerr_r!(", "xerr_rf!("]);
    let expected = expected_multiset(&[
        "protocol/src/action/astselect.rs:return xerr_rf!(\"action ast select must succeed at least {} but only {}\", slt_min, ok);",
        "protocol/src/action/chain.rs:return xerr_rf!(\"transction must submit in height between {} and {}\", left, right)",
        "protocol/src/action/chain.rs:return xerr_rf!(\"transction must belong to chains {} but on chain {}\", cids, cid)",
        "protocol/src/action/chain.rs:return xerr_rf!(",
        "protocol/src/action/chain.rs:return xerr_rf!(",
        "protocol/src/action/chain.rs:return xerr_rf!(",
        "protocol/src/action/chain.rs:return xerr_rf!(",
        "protocol/src/operate/asset.rs:return xerr_rf!(\"address {} asset {} is insufficient, at least {}\",",
        "protocol/src/operate/asset.rs:xerr_rf!(\"address {} asset is insufficient, at least {}\", addr, ast)",
        "protocol/src/operate/diamond.rs:return xerr_rf!(\"address {} diamond {} is insufficient, at least {}\",",
        "protocol/src/operate/diamond.rs:return xerr_rf!(\"diamond {} has been mortgaged and cannot be transferred\", hacd_name.to_readable())",
        "protocol/src/operate/diamond.rs:return xerr_rf!(\"diamond {} not belong to address {}\", hacd_name.to_readable(), addr_from)",
        "protocol/src/operate/hacash.rs:return xerr_rf!(\"address {} balance {} is insufficient, at least {}\",",
        "protocol/src/operate/hacash.rs:xerr_rf!(\"address {} balance is insufficient, at least {}\", addr, amt)",
        "protocol/src/operate/satoshi.rs:return xerr_rf!(\"address {} satoshi {} is insufficient, at least {}\",",
        "protocol/src/operate/satoshi.rs:xerr_rf!(\"address {} satoshi is insufficient\", addr)",
    ]);
    expect_multiset_eq("unwind macro callsites", &actual, expected);
}

#[test]
fn direct_xerror_revert_callsites_are_allowlisted() {
    let _guard = test_guard();
    let actual = scan_lines_with_patterns(&["XError::revert("]);
    let expected = expected_multiset(&[
        "vm/src/rt/error.rs:maybe!(is_unwind, XError::revert(text), XError::fault(text))",
        "vm/src/interpreter/test.rs:return Err(XError::revert(e.clone()));",
        "vm/src/machine/setup.rs:Some(d) => Err(XError::revert(format!(\"{} return error {}\", err_msg, d))),",
    ]);
    expect_multiset_eq("direct XError::revert callsites", &actual, expected);
}

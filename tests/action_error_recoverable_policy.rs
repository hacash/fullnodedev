#![cfg(feature = "vm")]

use std::collections::BTreeSet;
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
use sys::{BError, BRet, Error, IntoBRet, Ret, UNWIND_PREFIX};
use testkit::sim::integration::{
    make_ctx_from_tx as make_ctx, make_stub_tx as make_tx, test_guard, vm_alt_addr as alt_addr,
    vm_main_addr as main_addr,
};
use testkit::sim::logs::MemLogs;
use testkit::sim::state::FlatMemState as StateMem;
use vm::machine::check_vm_return_value;
use vm::rt::{ItrErr, ItrErrCode};
use vm::value::Value;

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

fn expect_recoverable_ret<T>(ret: Ret<T>, msg: &str) -> BError {
    let err = match ret.into_bret() {
        Ok(_) => panic!("expect recoverable error but got Ok"),
        Err(err) => err,
    };
    assert!(err.is_recoverable(), "expect recoverable but got {err}");
    if !msg.is_empty() {
        assert!(err.contains(msg), "expect '{msg}' in '{err}'");
    }
    err
}

fn expect_recoverable_bret<T>(ret: BRet<T>, msg: &str) -> BError {
    let err = match ret {
        Ok(_) => panic!("expect recoverable error but got Ok"),
        Err(err) => err,
    };
    assert!(err.is_recoverable(), "expect recoverable but got {err}");
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
fn guard_action_failures_are_recoverable() {
    let _guard = test_guard();
    let main = main_addr();
    let tx = make_tx(3, main, vec![main], 17);
    let mut ctx = mk_ctx(100, 9, &tx, Box::new(StateMem::default()));

    let mut bad_range = HeightScope::new();
    bad_range.start = BlockHeight::from(20);
    bad_range.end = BlockHeight::from(10);
    expect_recoverable_bret(bad_range.execute(&mut ctx), "cannot big than");

    let mut out_of_range = HeightScope::new();
    out_of_range.start = BlockHeight::from(200);
    out_of_range.end = BlockHeight::from(300);
    expect_recoverable_bret(out_of_range.execute(&mut ctx), "submit in height between");

    let mut allow = ChainAllow::new();
    allow.chains = ChainIDList::from_list(vec![Uint4::from(1), Uint4::from(2)]).unwrap();
    expect_recoverable_bret(allow.execute(&mut ctx), "must belong to chains");
}

#[test]
fn state_data_business_failures_are_recoverable() {
    let _guard = test_guard();
    let main = main_addr();
    let alt = alt_addr();
    let tx = make_tx(3, main, vec![main, alt], 17);
    let mut ctx = mk_ctx(100, 1, &tx, Box::new(StateMem::default()));

    // HAC
    expect_recoverable_ret(hac_sub(&mut ctx, &main, &Amount::zero()), "not positive");
    expect_recoverable_ret(hac_sub(&mut ctx, &main, &Amount::mei(1)), "insufficient");
    expect_recoverable_ret(hac_check(&mut ctx, &main, &Amount::mei(1)), "insufficient");

    // BTC (Satoshi)
    expect_recoverable_ret(
        sat_check(&mut ctx, &main, &Satoshi::from(0)),
        "cannot empty",
    );
    expect_recoverable_ret(sat_sub(&mut ctx, &main, &Satoshi::from(1)), "insufficient");
    expect_recoverable_ret(
        sat_transfer(&mut ctx, &main, &main, &Satoshi::from(1)),
        "cannot trs to self",
    );

    // ASSET
    let zero_asset = AssetAmt::from(7, 0).unwrap();
    expect_recoverable_ret(asset_check(&mut ctx, &main, &zero_asset), "cannot empty");
    {
        let mut state = CoreState::wrap(ctx.state());
        expect_recoverable_ret(
            asset_sub(&mut state, &main, &AssetAmt::from(7, 1).unwrap()),
            "insufficient",
        );
    }
    expect_recoverable_ret(
        asset_transfer(&mut ctx, &main, &main, &AssetAmt::from(7, 1).unwrap()),
        "cannot trs to self",
    );

    // HACD
    {
        let mut state = CoreState::wrap(ctx.state());
        expect_recoverable_ret(
            hacd_sub(&mut state, &main, &DiamondNumber::from(1)),
            "insufficient",
        );
    }
    let dlist_self = DiamondNameListMax200::one(DiamondName::from_readable(b"WTYUIA").unwrap());
    {
        let mut state = CoreState::wrap(ctx.state());
        expect_recoverable_ret(
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
        expect_recoverable_ret(
            check_diamond_status(&mut state, &main, &dia_mortgaged),
            "mortgaged",
        );
        expect_recoverable_ret(
            check_diamond_status(&mut state, &alt, &dia_not_belong),
            "not belong",
        );
        let miss = DiamondNameListMax200::one(DiamondName::from_readable(b"WTYUKB").unwrap());
        expect_recoverable_ret(
            diamond_owned_move(&mut state, &main, &alt, &miss),
            "not find",
        );
    }
}

#[test]
fn channel_open_insufficient_balance_is_recoverable() {
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

    expect_recoverable_bret(open.execute(&mut ctx), "insufficient");
}

#[test]
fn vm_non_zero_return_code_is_recoverable() {
    let _guard = test_guard();
    assert!(check_vm_return_value(&Value::nil(), "main call").is_ok());
    assert!(check_vm_return_value(&Value::u8(0), "main call").is_ok());
    expect_recoverable_ret(
        check_vm_return_value(&Value::u8(7), "main call"),
        "return error code 7",
    );
}

#[test]
fn vm_throw_abort_and_ext_action_pass_through_policy() {
    let _guard = test_guard();

    let throw_abort: BError = ItrErr(ItrErrCode::ThrowAbort, "contract abort".to_owned()).into();
    assert!(throw_abort.is_recoverable(), "{throw_abort}");
    let throw_abort_wire: Error =
        ItrErr(ItrErrCode::ThrowAbort, "contract abort".to_owned()).into();
    assert!(
        throw_abort_wire.starts_with(UNWIND_PREFIX),
        "{throw_abort_wire}"
    );

    let ext_prefixed: BError = ItrErr(
        ItrErrCode::ExtActCallError,
        format!("{UNWIND_PREFIX}biz fail"),
    )
    .into();
    assert!(ext_prefixed.is_recoverable(), "{ext_prefixed}");
    assert!(ext_prefixed.contains("ExtActCallError"), "{ext_prefixed}");

    let ext_prefixed_wire: Error = ItrErr(
        ItrErrCode::ExtActCallError,
        format!("{UNWIND_PREFIX}biz fail"),
    )
    .into();
    assert!(
        ext_prefixed_wire.starts_with(UNWIND_PREFIX),
        "{ext_prefixed_wire}"
    );

    let ext_plain: BError = ItrErr(ItrErrCode::ExtActCallError, "plain fail".to_owned()).into();
    assert!(ext_plain.is_unrecoverable(), "{ext_plain}");
    let ext_plain_wire: Error = ItrErr(ItrErrCode::ExtActCallError, "plain fail".to_owned()).into();
    assert!(
        !ext_plain_wire.starts_with(UNWIND_PREFIX),
        "{ext_plain_wire}"
    );
}

#[test]
fn vm_itr_err_code_recoverable_mapping_is_strict() {
    let _guard = test_guard();
    use ItrErrCode::*;

    // Keep this exhaustive list in sync with vm/src/rt/error.rs.
    let all_codes = [
        ContractError,
        NotFindContract,
        AbstTypeError,
        CodeTypeError,
        InheritsError,
        LibrarysError,
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
        ExtActDisabled,
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
        CallNotPublic,
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
        ExtActCallError,
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
    assert_eq!(all_codes.len(), 82);

    for code in all_codes {
        let berr: BError = ItrErr(code, "x".to_owned()).into();
        let should_recoverable = matches!(code, ThrowAbort);
        assert_eq!(
            berr.is_recoverable(),
            should_recoverable,
            "ItrErrCode::{:?} recoverable mismatch: {}",
            code,
            berr
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

fn scan_lines_with_patterns(patterns: &[&str]) -> BTreeSet<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![];
    for rel in ["protocol/src", "mint/src", "vm/src"] {
        walk_rs_files(&root.join(rel), &mut files);
    }
    files.sort();

    let mut found = BTreeSet::new();
    for file in files {
        let rel = file.strip_prefix(root).unwrap().display().to_string();
        let src = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("read_to_string {} failed: {}", file.display(), e));
        for (ln, line) in src.lines().enumerate() {
            if patterns.iter().any(|pat| line.contains(pat)) {
                found.insert(format!("{}:{}:{}", rel, ln + 1, normalized(line)));
            }
        }
    }
    found
}

fn expect_set_eq(label: &str, actual: &BTreeSet<String>, expected: BTreeSet<String>) {
    let missing: Vec<_> = expected.difference(actual).cloned().collect();
    let extra: Vec<_> = actual.difference(&expected).cloned().collect();
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
fn recoverable_macro_callsites_are_allowlisted() {
    let _guard = test_guard();
    let actual = scan_lines_with_patterns(&["erru!(", "erruf!(", "berru!(", "berruf!("]);
    let expected: BTreeSet<String> = [
        "mint/src/action/diamond_insc.rs:397:return erruf!(",
        "mint/src/action/diamond_insc.rs:403:return erruf!(",
        "protocol/src/action/astselect.rs:27:return erruf!(\"action ast select max cannot less than min\")",
        "protocol/src/action/astselect.rs:30:return erruf!(\"action ast select max cannot more than list num\")",
        "protocol/src/action/astselect.rs:33:return erruf!(\"action ast select num cannot more than {}\", TX_ACTIONS_MAX)",
        "protocol/src/action/astselect.rs:73:return erruf!(\"action ast select must succeed at least {} but only {}\", slt_min, ok)",
        "protocol/src/action/chain.rs:21:return erruf!(\"left height {} cannot big than rigth height {}\", left, right)",
        "protocol/src/action/chain.rs:24:return erruf!(\"transction must submit in height between {} and {}\", left, right)",
        "protocol/src/action/chain.rs:48:return erruf!(\"transction must belong to chains {} but on chain {}\", cids, cid)",
        "protocol/src/action/util.rs:19:None => return erruf!(\"ast tree depth overflow\"),",
        "protocol/src/action/util.rs:22:return erruf!(",
        "protocol/src/action/util.rs:35:BError::Unwind(msg) => erru!(msg),",
        "protocol/src/operate/asset.rs:8:return erruf!(\"Asset operate amount cannot be zero\")",
        "protocol/src/operate/asset.rs:36:return erruf!(\"address {} asset {} is insufficient, at least {}\",",
        "protocol/src/operate/asset.rs:51:return erruf!(\"cannot trs to self\")",
        "protocol/src/operate/asset.rs:70:return erruf!(\"check asset is cannot empty\")",
        "protocol/src/operate/asset.rs:81:erruf!(\"address {} asset is insufficient, at least {}\", addr, ast)",
        "protocol/src/operate/diamond.rs:34:return erruf!(\"address {} diamond {} is insufficient, at least {}\",",
        "protocol/src/operate/diamond.rs:50:return erruf!(\"cannot transfer to self\")",
        "protocol/src/operate/diamond.rs:73:return erruf!(\"cannot transfer to self\")",
        "protocol/src/operate/diamond.rs:92:return erruf!(\"diamond {} has been mortgaged and cannot be transferred\", hacd_name.to_readable())",
        "protocol/src/operate/diamond.rs:95:return erruf!(\"diamond {} not belong to address {}\", hacd_name.to_readable(), addr_from)",
        "protocol/src/operate/diamond.rs:126:return erruf!(\"cannot transfer to self\")",
        "protocol/src/operate/diamond.rs:131:return erruf!(\"from diamond owned form not find\")",
        "protocol/src/operate/hacash.rs:6:return erruf!(\"amount {} value is not positive\", $amt)",
        "protocol/src/operate/hacash.rs:33:return erruf!(\"address {} balance {} is insufficient, at least {}\",",
        "protocol/src/operate/hacash.rs:85:erruf!(\"address {} balance is insufficient, at least {}\", addr, amt)",
        "protocol/src/operate/satoshi.rs:9:return erruf!(\"satoshi value cannot be zero\")",
        "protocol/src/operate/satoshi.rs:37:return erruf!(\"address {} satoshi {} is insufficient, at least {}\",",
        "protocol/src/operate/satoshi.rs:52:return erruf!(\"cannot trs to self\")",
        "protocol/src/operate/satoshi.rs:72:return erruf!(\"check satoshi is cannot empty\")",
        "protocol/src/operate/satoshi.rs:81:erruf!(\"address {} satoshi is insufficient\", addr)",
        "vm/src/machine/setup.rs:20:return erruf!(\"{} return error code {}\", err_msg, rv.to_uint())",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    expect_set_eq("recoverable macro callsites", &actual, expected);
}

#[test]
fn direct_berror_recoverable_callsites_are_allowlisted() {
    let _guard = test_guard();
    let actual = scan_lines_with_patterns(&["BError::recoverable("]);
    let expected: BTreeSet<String> = [
        "vm/src/rt/error.rs:138:return BError::recoverable(format!(\"{:?}({}): {}\", code, code as u8, m));",
        "vm/src/rt/error.rs:144:BError::recoverable(text)",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    expect_set_eq("direct BError::recoverable callsites", &actual, expected);
}

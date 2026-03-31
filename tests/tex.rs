#[cfg(test)]
#[allow(unused)]
mod tex {

    use basis::interface::{ActExec, StateOperat};
    use field::*;
    use mint::action::{AssetCreate, ASSET_ALIVE_HEIGHT};
    use protocol::{action::*, tex::*};
    use sys::*;
    use vm::contract::*;

    /*
        1MzNY1oA3kfgYi75zquj3SRUPYztzXHzK9   123456
        18dekVcACnj6Tbd69SsexVMQ5KLBZZfn5K   123457
        emqjNS9PscqdBpMtnC3Jfuc4mvZUPYTPS


    */

    #[test]
    fn trs1() {
        let acc1 = Account::create_by_password("123456").unwrap();
        let acc2 = Account::create_by_password("123457").unwrap();
        let adr1 = Address::from(*acc1.address());
        let adr2 = Address::from(*acc2.address());

        let mut tex1 = TexCellAct::create_by(adr1);
        let mut tex2 = TexCellAct::create_by(adr2);

        tex1.add_cell(Box::new(CellTrsZhuGet::new(
            Fold64::from(100000000).unwrap(),
        )))
        .unwrap();
        tex1.add_cell(Box::new(CellTrsSatPay::new(Fold64::from(2).unwrap())))
            .unwrap();
        tex1.add_cell(Box::new(CellTrsDiaPay::new(
            DiamondNameListMax200::from_readable("KKKKVA,HYXYHY,UETWNK").unwrap(),
        )))
        .unwrap();
        tex1.add_cell(Box::new(CellTrsAssetGet::new(
            AssetAmt::from(5, 100).unwrap(),
        )))
        .unwrap();
        tex1.do_sign(&acc1).unwrap();

        tex2.add_cell(Box::new(CellTrsZhuPay::new(
            Fold64::from(100000000).unwrap(),
        )))
        .unwrap();
        tex2.add_cell(Box::new(CellTrsSatGet::new(Fold64::from(2).unwrap())))
            .unwrap();
        tex2.add_cell(Box::new(CellTrsDiaGet::new(DiamondNumber::from(3))))
            .unwrap();
        tex2.add_cell(Box::new(CellTrsAssetPay::new(
            AssetAmt::from(5, 100).unwrap(),
        )))
        .unwrap();
        tex2.do_sign(&acc2).unwrap();
        // tex2.sign.signature[1] = 1; // like sign bug

        let mut act1 = HacToTrs::new();
        act1.to = AddrOrPtr::from_addr(adr2);
        act1.hacash = Amount::mei(2);

        let mut act2 = AssetCreate::new();
        act2.metadata = AssetSmelt {
            serial: Fold64::from(5).unwrap(),
            supply: Fold64::from(10000).unwrap(),
            decimal: Uint1::from(2),
            issuer: adr2,
            ticket: BytesW1::from_str("USDT").unwrap(),
            name: BytesW1::from_str("Teather").unwrap(),
        };
        act2.protocol_cost = Amount::mei(1);

        //
        curl_trs_1(vec![
            Box::new(act1),
            Box::new(act2),
            Box::new(tex1),
            Box::new(tex2),
        ]);
    }

    #[test]
    fn asset_create_serial_unlocks_at_minsri_on_first_allowed_height() {
        use mint::genesis;
        use protocol::state::CoreState;
        use testkit::sim::integration::{make_ctx_from_tx, make_stub_tx, scoped_setup, test_guard, vm_main_addr};
        use testkit::sim::logs::MemLogs;
        use testkit::sim::state::FlatMemState as StateMem;

        let _guard = test_guard();
        let _setup = scoped_setup(None);
        let main = vm_main_addr();
        let height = ASSET_ALIVE_HEIGHT;
        let tx = make_stub_tx(3, main, vec![main], 17);
        let mut ctx = make_ctx_from_tx(height, &tx, Box::new(StateMem::default()), Box::new(MemLogs::default()));
        ctx.env.chain.fast_sync = true;

        let bls = Balance::hac(genesis::block_reward(height));
        CoreState::wrap(ctx.state()).balance_set(&main, &bls);

        let mut act = AssetCreate::new();
        act.metadata = AssetSmelt {
            serial: Fold64::from(1025).unwrap(),
            supply: Fold64::from(10000).unwrap(),
            decimal: Uint1::from(2),
            issuer: main,
            ticket: BytesW1::from_str("USDT").unwrap(),
            name: BytesW1::from_str("Tether").unwrap(),
        };
        act.protocol_cost = genesis::block_reward(height);

        act.execute(&mut ctx).unwrap();

        let sta = CoreState::wrap(ctx.state());
        assert!(sta.asset(&Fold64::from(1025).unwrap()).is_some());
        let bls = sta.balance(&main).unwrap();
        assert_eq!(bls.asset(Fold64::from(1025).unwrap()).unwrap().amount.uint(), 10000);
    }
}

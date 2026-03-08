#![cfg(feature = "vm")]

#[cfg(test)]
mod tests {
    use basis::interface::Context;
    use testkit::sim::integration::{make_ctx_from_tx, make_stub_tx, vm_main_addr};
    use testkit::sim::state::FlatMemState as MemState;
    use vm::interpreter::execute_code;
    use vm::machine::CtxHost;
    use vm::rt::SpaceCap;
    use vm::rt::{Bytecode, ExecCtx, GasExtra, GasTable, ItrErrCode};
    use vm::space::{CtcKVMap, GKVMap, Heap, Stack};
    use vm::ContractAddress;

    #[test]
    fn reject_unknown_action_ids() {
        let codes = vec![
            Bytecode::PNIL as u8,
            Bytecode::ACTION as u8,
            98u8, // ContractUpdate::KIND low byte (dangerous if not allowlisted)
            Bytecode::END as u8,
        ];

        let mut pc: usize = 0;
        let mut gas: i64 = 10_000;
        let cadr = ContractAddress::default();

        let main = vm_main_addr();
        let tx = make_stub_tx(3, main, vec![main], 1);
        let mut ctx = make_ctx_from_tx(
            1,
            &tx,
            Box::new(MemState::default()),
            Box::new(protocol::state::EmptyLogs {}),
        );
        let ctx: &mut dyn Context = &mut ctx;

        let mut ops = Stack::new(16);
        let mut locals = Stack::new(16);
        let mut heap = Heap::new(16);

        let mut host = CtxHost::new(ctx);
        let res = execute_code(
            &mut pc,
            &codes,
            ExecCtx::main(),
            &mut ops,
            &mut locals,
            &mut heap,
            &cadr,
            &cadr,
            &mut gas,
            &GasTable::new(1),
            &GasExtra::new(1),
            &SpaceCap::new(1),
            &mut GKVMap::new(4),
            &mut CtcKVMap::new(4),
            &mut host,
        );

        let err = res.unwrap_err();
        assert_eq!(err.0, ItrErrCode::ActCallError);
    }
}

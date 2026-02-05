#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use basis::component::Env;
    use basis::interface::{Context, State, TransactionRead};
    use protocol::state::EmptyLogs;
    use sys::Ret;
    use vm::rt::{Bytecode, ExecMode, GasExtra, GasTable, ItrErrCode};
    use vm::rt::SpaceCap;
    use vm::space::{CtcKVMap, GKVMap, Heap, Stack};
    use vm::ContractAddress;
    use vm::machine::CtxHost;
    use vm::interpreter::execute_code;
    use field::{Address, Amount, Hash};
    use protocol::context::ContextInst;

    #[derive(Default)]
    struct MemState {
        kv: HashMap<Vec<u8>, Vec<u8>>,
    }

    impl State for MemState {
        fn get(&self, k: Vec<u8>) -> Option<Vec<u8>> {
            self.kv.get(&k).cloned()
        }
        fn set(&mut self, k: Vec<u8>, v: Vec<u8>) {
            self.kv.insert(k, v);
        }
        fn del(&mut self, k: Vec<u8>) {
            self.kv.remove(&k);
        }
    }

    #[derive(Default, Clone, Debug)]
    struct DummyTx;

    impl field::Serialize for DummyTx {
        fn size(&self) -> usize { 0 }
        fn serialize(&self) -> Vec<u8> { vec![] }
    }
    impl basis::interface::TxExec for DummyTx {}
    impl TransactionRead for DummyTx {
        fn ty(&self) -> u8 { 3 }
        fn hash(&self) -> Hash { Hash::default() }
        fn hash_with_fee(&self) -> Hash { Hash::default() }
        fn main(&self) -> Address { Address::default() }
        fn addrs(&self) -> Vec<Address> { vec![Address::default()] }
        fn fee(&self) -> &Amount { Amount::zero_ref() }
        fn fee_purity(&self) -> u64 { 1 }
        fn fee_extend(&self) -> Ret<(u16, Amount)> { Ok((1, Amount::zero())) }
    }

    #[test]
    fn reject_unknown_extaction_ids() {
        let codes = vec![
            Bytecode::PNIL as u8,
            Bytecode::EXTACTION as u8,
            98u8, // ContractUpdate::KIND low byte (dangerous if not allowlisted)
            Bytecode::END as u8,
        ];

        let mut pc: usize = 0;
        let mut gas: i64 = 10_000;
        let cadr = ContractAddress::default();

        let tx = DummyTx::default();
        let mut env = Env::default();
        env.block.height = 1;
        let mut ctx = ContextInst::new(env, Box::new(MemState::default()), Box::new(EmptyLogs {}), &tx);
        let ctx: &mut dyn Context = &mut ctx;

        let mut ops = Stack::new(16);
        let mut locals = Stack::new(16);
        let mut heap = Heap::new(16);

        let mut host = CtxHost::new(ctx);
        let res = execute_code(
            &mut pc,
            &codes,
            ExecMode::Main,
            false,
            0,
            &mut gas,
            &GasTable::new(1),
            &GasExtra::new(1),
            &SpaceCap::new(1),
            &mut ops,
            &mut locals,
            &mut heap,
            &mut GKVMap::new(4),
            &mut CtcKVMap::new(4),
            &mut host,
            &cadr,
            &cadr,
        );

        let err = res.unwrap_err();
        assert_eq!(err.0, ItrErrCode::ExtActCallError);
    }
}

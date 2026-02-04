#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use basis::interface::{ActCall, State};
    use protocol::state::EmptyLogs;
    use sys::Ret;
    use vm::rt::{Bytecode, ExecMode, GasExtra, GasTable, ItrErrCode};
    use vm::rt::SpaceCap;
    use vm::space::{CtcKVMap, GKVMap, Heap, Stack};
    use vm::ContractAddress;
    use vm::{interpreter::execute_code, VMState};

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

    #[derive(Default)]
    struct PanicActCall;

    impl ActCall for PanicActCall {
        fn height(&self) -> u64 {
            1
        }
        fn action_call(&mut self, _: u16, _: Vec<u8>) -> Ret<(u32, Vec<u8>)> {
            panic!("action_call should not be invoked for unknown EXTACTION id")
        }
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

        let mut raw_state = MemState::default();
        let mut sta = VMState::wrap(&mut raw_state);
        let mut ctx = PanicActCall::default();

        let mut ops = Stack::new(16);
        let mut locals = Stack::new(16);
        let mut heap = Heap::new(16);

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
            &mut ctx,
            &mut EmptyLogs {},
            &mut sta,
            &cadr,
            &cadr,
        );

        let err = res.unwrap_err();
        assert_eq!(err.0, ItrErrCode::ExtActCallError);
    }
}

#[cfg(test)]
mod tests {
    use basis::interface::Context;
    use field::{Address, Amount};
    use sys::XRet;
    use testkit::sim::integration::{make_ctx_from_tx, make_stub_tx, vm_main_addr};
    use testkit::sim::state::FlatMemState as MemState;
    use vm::ContractAddress;
    use vm::interpreter::execute_code;
    use vm::machine::DeferCallbacks;
    use vm::machine::VmHost;
    use vm::rt::FrameBindings;
    use vm::rt::{Bytecode, ExecCtx, GasExtra, GasTable, ItrErr, ItrErrCode, SpaceCap, VmrtRes};
    use vm::space::{CtcKVMap, GKVMap, Heap, Stack};
    use vm::{ContractEdition, ContractSto, VmLog};

    struct TestVmHost<'a> {
        ctx: &'a mut dyn Context,
        gas_remaining: i64,
    }

    impl VmHost for TestVmHost<'_> {
        fn height(&self) -> u64 {
            self.ctx.env().block.height
        }

        fn main_entry_bindings(&self) -> FrameBindings {
            FrameBindings::root(self.ctx.tx().main(), self.ctx.env().tx.addrs.clone().into())
        }

        fn gas_remaining(&self) -> i64 {
            self.gas_remaining
        }

        fn gas_charge(&mut self, gas: i64) -> VmrtRes<()> {
            if gas < 0 {
                return Err(ItrErr::new(
                    ItrErrCode::GasError,
                    &format!("gas cost invalid: {}", gas),
                ));
            }
            self.gas_remaining -= gas;
            if self.gas_remaining < 0 {
                return Err(ItrErr::code(ItrErrCode::OutOfGas));
            }
            Ok(())
        }

        fn gas_rebate(&mut self, gas: i64) -> VmrtRes<()> {
            let _ = gas;
            Ok(())
        }

        fn contract_edition(&mut self, addr: &ContractAddress) -> Option<ContractEdition> {
            vm::VMState::wrap(self.ctx.state()).contract_edition(addr)
        }

        fn contract(&mut self, addr: &ContractAddress) -> Option<ContractSto> {
            vm::VMState::wrap(self.ctx.state()).contract(addr)
        }

        fn action_call(&mut self, kid: u16, body: Vec<u8>) -> XRet<(u32, Vec<u8>)> {
            self.ctx.action_call(kid, body)
        }

        fn log_push(&mut self, addr: &Address, items: Vec<vm::value::Value>) -> VmrtRes<()> {
            let lgdt = VmLog::new(*addr, items)?;
            self.ctx.logs().push(&lgdt);
            Ok(())
        }

        fn sstat(&mut self, gst: &GasExtra, cap: &SpaceCap, addr: &Address, key: &vm::value::Value) -> VmrtRes<vm::value::Value> {
            let _ = (gst, cap, addr, key);
            Err(ItrErr::code(ItrErrCode::StorageError))
        }

        fn sload(&mut self, gst: &GasExtra, cap: &SpaceCap, addr: &Address, key: &vm::value::Value) -> VmrtRes<vm::value::Value> {
            let _ = (gst, cap, addr, key);
            Err(ItrErr::code(ItrErrCode::StorageError))
        }

        fn sdel(&mut self, gst: &GasExtra, cap: &SpaceCap, addr: &Address, key: vm::value::Value) -> VmrtRes<i64> {
            let _ = (gst, cap, addr, key);
            Err(ItrErr::code(ItrErrCode::StorageError))
        }

        fn snew(
            &mut self,
            gst: &GasExtra,
            cap: &SpaceCap,
            addr: &Address,
            key: vm::value::Value,
            val: vm::value::Value,
            period: vm::value::Value,
        ) -> VmrtRes<i64> {
            let _ = (gst, cap, addr, key, val, period);
            Err(ItrErr::code(ItrErrCode::StorageError))
        }

        fn sedit(
            &mut self,
            gst: &GasExtra,
            cap: &SpaceCap,
            addr: &Address,
            key: vm::value::Value,
            val: vm::value::Value,
        ) -> VmrtRes<i64> {
            let _ = (gst, cap, addr, key, val);
            Err(ItrErr::code(ItrErrCode::StorageError))
        }

        fn srent(
            &mut self,
            gst: &GasExtra,
            cap: &SpaceCap,
            addr: &Address,
            key: vm::value::Value,
            period: vm::value::Value,
        ) -> VmrtRes<i64> {
            let _ = (gst, cap, addr, key, period);
            Err(ItrErr::code(ItrErrCode::StorageError))
        }

        fn srecv(
            &mut self,
            gst: &GasExtra,
            cap: &SpaceCap,
            addr: &Address,
            key: vm::value::Value,
            period: vm::value::Value,
        ) -> VmrtRes<i64> {
            let _ = (gst, cap, addr, key, period);
            Err(ItrErr::code(ItrErrCode::StorageError))
        }
    }

    #[test]
    fn reject_unknown_action_ids() {
        let codes = vec![
            Bytecode::PNIL as u8,
            Bytecode::ACTION as u8,
            98u8, // ContractUpdate::KIND low byte (dangerous if not allowlisted)
            Bytecode::END as u8,
        ];

        let mut pc: usize = 0;
        let gas: i64 = 10_000;
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
        protocol::operate::hac_add(ctx, &main, &Amount::unit238(1_000_000_000)).unwrap();

        let mut ops = Stack::new(16);
        let mut locals = Stack::new(16);
        let mut heap = Heap::new(16);
        let mut gas_use = basis::interface::GasUse::default();

        let mut host = TestVmHost {
            ctx,
            gas_remaining: gas,
        };
        let mut defer_callbacks = DeferCallbacks::default();
        let res = execute_code(
            &mut pc,
            &codes,
            ExecCtx::main(),
            &mut ops,
            &mut locals,
            &mut heap,
            &cadr,
            &cadr,
            &GasTable::new(1),
            &GasExtra::new(1),
            &SpaceCap::new(1),
            &mut gas_use,
            &mut GKVMap::new(4),
            &mut CtcKVMap::new(4),
            &mut defer_callbacks,
            &mut host,
        );

        let err = res.unwrap_err();
        assert_eq!(err.0, ItrErrCode::ActCallError);
    }
}

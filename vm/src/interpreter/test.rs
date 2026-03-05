

#[cfg(test)]
mod bounds_tests {
    use super::*;
    use crate::machine::VmHost;
    use crate::rt::{ExecMode, GasExtra, GasTable, ItrErr, ItrErrCode, SpaceCap, VmrtErr, VmrtRes};
    use crate::space::{CtcKVMap, GKVMap, Heap, Stack};
    use crate::value::{CompoItem, Value, ValueTy, RESERVED_U256_TYPE_ID};
    use field::Address;
    use crate::ContractAddress;
    use sys::Ret;

    #[derive(Default)]
    struct DummyHost {
        ext_res: Vec<u8>,
        ext_gas: u32,
        ext_err: Option<String>,
        ext_body: Vec<u8>,
        srest_res: Option<Value>,
        sload_res: Option<Value>,
        log_calls: usize,
    }

    impl VmHost for DummyHost {
        fn height(&mut self) -> u64 {
            1
        }

        fn ext_action_call(&mut self, _kid: u16, body: Vec<u8>) -> Ret<(u32, Vec<u8>)> {
            if let Some(e) = &self.ext_err {
                return Err(e.clone())
            }
            self.ext_body = body;
            Ok((self.ext_gas, self.ext_res.clone()))
        }

        fn log_push(&mut self, _cadr: &ContractAddress, _items: Vec<Value>) -> VmrtErr {
            self.log_calls += 1;
            Ok(())
        }

        fn srest(&mut self, _hei: u64, _cadr: &ContractAddress, _key: &Value) -> VmrtRes<Value> {
            match &self.srest_res {
                Some(v) => Ok(v.clone()),
                None => itr_err_code!(ItrErrCode::StorageError),
            }
        }

        fn sload(&mut self, _hei: u64, _cadr: &ContractAddress, _key: &Value) -> VmrtRes<Value> {
            match &self.sload_res {
                Some(v) => Ok(v.clone()),
                None => itr_err_code!(ItrErrCode::StorageError),
            }
        }

        fn sdel(&mut self, _cadr: &ContractAddress, _key: Value) -> VmrtErr {
            itr_err_code!(ItrErrCode::StorageError)
        }

        fn ssave(
            &mut self,
            _gst: &GasExtra,
            _hei: u64,
            _cadr: &ContractAddress,
            _key: Value,
            _val: Value,
        ) -> VmrtRes<i64> {
            itr_err_code!(ItrErrCode::StorageError)
        }

        fn srent(
            &mut self,
            _gst: &GasExtra,
            _hei: u64,
            _cadr: &ContractAddress,
            _key: Value,
            _period: Value,
        ) -> VmrtRes<i64> {
            itr_err_code!(ItrErrCode::StorageError)
        }
    }

    fn run_with_setup<F>(codes: Vec<u8>, host: DummyHost, setup: F) -> i64
    where
        F: FnOnce(&mut Stack, &mut Stack, &mut Heap, &mut GKVMap, &mut CtcKVMap, &ContractAddress),
    {
        let mut pc: usize = 0;
        let mut gas: i64 = 1000;
        let mut host = host;

        let mut operands = Stack::new(256);
        let mut locals = Stack::new(256);
        let mut heap = Heap::new(64);
        let mut globals = GKVMap::new(20);
        let mut memorys = CtcKVMap::new(12);
        let cadr = ContractAddress::default();
        setup(
            &mut operands,
            &mut locals,
            &mut heap,
            &mut globals,
            &mut memorys,
            &cadr,
        );

        execute_code(
            &mut pc,
            &codes,
            ExecMode::Main,
            false,
            0,
            &mut gas,
            &GasTable::new(1),
            &GasExtra::new(1),
            &SpaceCap::new(1),
            &mut operands,
            &mut locals,
            &mut heap,
            &mut globals,
            &mut memorys,
            &mut host,
            &cadr,
            &cadr,
        )
        .unwrap();
        1000 - gas
    }

    #[test]
    fn execute_code_rejects_truncated_params() {
        use crate::rt::Bytecode;

        let codes = vec![Bytecode::PU16 as u8]; // missing 2 bytes param

        let mut pc: usize = 0;
        let mut gas: i64 = 1000;
        let mut host = DummyHost::default();

        let mut operands = Stack::new(256);
        let mut locals = Stack::new(256);
        let mut heap = Heap::new(64);
        let mut globals = GKVMap::new(20);
        let mut memorys = CtcKVMap::new(12);

        let cadr = ContractAddress::default();

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
            &mut operands,
            &mut locals,
            &mut heap,
            &mut globals,
            &mut memorys,
            &mut host,
            &cadr,
            &cadr,
        );

        assert!(matches!(res, Err(ItrErr(ItrErrCode::CodeOverflow, _))));
    }

    #[test]
    fn execute_code_rejects_reserved_type_id_for_tis_and_cto() {
        use crate::rt::Bytecode;

        for inst in [Bytecode::TIS, Bytecode::CTO] {
            let codes = vec![Bytecode::P0 as u8, inst as u8, RESERVED_U256_TYPE_ID, Bytecode::END as u8];

            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost::default();

            let mut operands = Stack::new(256);
            let mut locals = Stack::new(256);
            let mut heap = Heap::new(64);
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);

            let cadr = ContractAddress::default();

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
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            );

            assert!(
                matches!(res, Err(ItrErr(ItrErrCode::InstParamsErr, _))),
                "instruction {:?} should fail with InstParamsErr",
                inst
            );
        }
    }

    #[test]
    fn execute_code_rejects_unknown_type_id_for_tis_and_cto() {
        use crate::rt::Bytecode;

        for raw in [12u8, 13u8] {
            for inst in [Bytecode::TIS, Bytecode::CTO] {
                let codes = vec![Bytecode::P0 as u8, inst as u8, raw, Bytecode::END as u8];

                let mut pc: usize = 0;
                let mut gas: i64 = 1000;
                let mut host = DummyHost::default();

                let mut operands = Stack::new(256);
                let mut locals = Stack::new(256);
                let mut heap = Heap::new(64);
                let mut globals = GKVMap::new(20);
                let mut memorys = CtcKVMap::new(12);

                let cadr = ContractAddress::default();

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
                    &mut operands,
                    &mut locals,
                    &mut heap,
                    &mut globals,
                    &mut memorys,
                    &mut host,
                    &cadr,
                    &cadr,
                );

                assert!(
                    matches!(res, Err(ItrErr(ItrErrCode::InstParamsErr, _))),
                    "instruction {:?} with type id {} should fail with InstParamsErr",
                    inst,
                    raw
                );
            }
        }
    }

    #[test]
    fn execute_code_rejects_cto_targets_outside_cast_set() {
        use crate::rt::Bytecode;

        let non_castable_targets = [ValueTy::Nil, ValueTy::HeapSlice, ValueTy::Compo];
        for ty in non_castable_targets {
            let codes = vec![Bytecode::P0 as u8, Bytecode::CTO as u8, ty as u8, Bytecode::END as u8];

            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost::default();

            let mut operands = Stack::new(256);
            let mut locals = Stack::new(256);
            let mut heap = Heap::new(64);
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);

            let cadr = ContractAddress::default();

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
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            );

            assert!(
                matches!(res, Err(ItrErr(ItrErrCode::InstParamsErr, _))),
                "CTO with target {:?} should fail with InstParamsErr",
                ty
            );
        }
    }

    #[test]
    fn execute_code_tis_accepts_declared_non_cast_types() {
        use crate::rt::Bytecode;

        let run = |stack_v: Value, ty: ValueTy| -> VmrtRes<Value> {
            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost::default();

            let mut operands = Stack::new(256);
            let mut locals = Stack::new(256);
            let mut heap = Heap::new(64);
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);
            let cadr = ContractAddress::default();
            let codes = vec![Bytecode::TIS as u8, ty as u8, Bytecode::END as u8];

            operands.push(stack_v)?;
            execute_code(
                &mut pc,
                &codes,
                ExecMode::Main,
                false,
                0,
                &mut gas,
                &GasTable::new(1),
                &GasExtra::new(1),
                &SpaceCap::new(1),
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            )?;
            operands.pop()
        };

        assert_eq!(run(Value::Nil, ValueTy::Nil).unwrap(), Value::Bool(true));
        assert_eq!(run(Value::U8(0), ValueTy::Nil).unwrap(), Value::Bool(false));
        assert_eq!(run(Value::HeapSlice((0, 0)), ValueTy::HeapSlice).unwrap(), Value::Bool(true));
        assert_eq!(run(Value::U8(1), ValueTy::HeapSlice).unwrap(), Value::Bool(false));
        assert_eq!(run(Value::Compo(CompoItem::new_list()), ValueTy::Compo).unwrap(), Value::Bool(true));
    }

    #[test]
    fn execute_code_cto_accepts_cast_set_targets() {
        use crate::rt::Bytecode;

        let run = |stack_v: Value, ty: ValueTy| -> VmrtRes<Value> {
            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost::default();

            let mut operands = Stack::new(256);
            let mut locals = Stack::new(256);
            let mut heap = Heap::new(64);
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);
            let cadr = ContractAddress::default();
            let codes = vec![Bytecode::CTO as u8, ty as u8, Bytecode::END as u8];

            operands.push(stack_v)?;
            execute_code(
                &mut pc,
                &codes,
                ExecMode::Main,
                false,
                0,
                &mut gas,
                &GasTable::new(1),
                &GasExtra::new(1),
                &SpaceCap::new(1),
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            )?;
            operands.pop()
        };

        let addr = Address::default();
        assert_eq!(run(Value::Nil, ValueTy::Bool).unwrap(), Value::Bool(false));
        assert_eq!(run(Value::Bool(true), ValueTy::U16).unwrap(), Value::U16(1));
        assert_eq!(run(Value::U16(0x0102), ValueTy::Bytes).unwrap(), Value::Bytes(vec![0x01, 0x02]));
        assert_eq!(run(Value::Bytes(addr.to_vec()), ValueTy::Address).unwrap(), Value::Address(addr));
    }

    #[test]
    fn execute_code_tis_and_cto_diverge_on_non_cast_targets() {
        use crate::rt::Bytecode;

        let run = |inst: Bytecode, stack_v: Value, ty: ValueTy| -> VmrtRes<Value> {
            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost::default();

            let mut operands = Stack::new(256);
            let mut locals = Stack::new(256);
            let mut heap = Heap::new(64);
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);
            let cadr = ContractAddress::default();
            let codes = vec![inst as u8, ty as u8, Bytecode::END as u8];

            operands.push(stack_v)?;
            execute_code(
                &mut pc,
                &codes,
                ExecMode::Main,
                false,
                0,
                &mut gas,
                &GasTable::new(1),
                &GasExtra::new(1),
                &SpaceCap::new(1),
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            )?;
            operands.pop()
        };

        assert_eq!(run(Bytecode::TIS, Value::Nil, ValueTy::Nil).unwrap(), Value::Bool(true));
        assert!(matches!(
            run(Bytecode::CTO, Value::Nil, ValueTy::Nil),
            Err(ItrErr(ItrErrCode::InstParamsErr, _))
        ));

        assert_eq!(
            run(Bytecode::TIS, Value::HeapSlice((0, 0)), ValueTy::HeapSlice).unwrap(),
            Value::Bool(true)
        );
        assert!(matches!(
            run(Bytecode::CTO, Value::HeapSlice((0, 0)), ValueTy::HeapSlice),
            Err(ItrErr(ItrErrCode::InstParamsErr, _))
        ));

        let list = CompoItem::new_list();
        assert_eq!(
            run(Bytecode::TIS, Value::Compo(list.clone()), ValueTy::Compo).unwrap(),
            Value::Bool(true)
        );
        assert!(matches!(
            run(Bytecode::CTO, Value::Compo(list), ValueTy::Compo),
            Err(ItrErr(ItrErrCode::InstParamsErr, _))
        ));
    }

    #[test]
    fn execute_code_cto_valid_target_with_invalid_operand_fails_castfail() {
        use crate::rt::Bytecode;

        let run = |stack_v: Value, ty: ValueTy| -> VmrtRes<Value> {
            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost::default();

            let mut operands = Stack::new(256);
            let mut locals = Stack::new(256);
            let mut heap = Heap::new(64);
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);
            let cadr = ContractAddress::default();
            let codes = vec![Bytecode::CTO as u8, ty as u8, Bytecode::END as u8];

            operands.push(stack_v)?;
            execute_code(
                &mut pc,
                &codes,
                ExecMode::Main,
                false,
                0,
                &mut gas,
                &GasTable::new(1),
                &GasExtra::new(1),
                &SpaceCap::new(1),
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            )?;
            operands.pop()
        };

        assert!(matches!(
            run(Value::U8(1), ValueTy::Address),
            Err(ItrErr(ItrErrCode::CastFail, _))
        ));
        assert!(matches!(
            run(Value::HeapSlice((0, 0)), ValueTy::Bool),
            Err(ItrErr(ItrErrCode::CastFail, _))
        ));
        assert!(matches!(
            run(Value::Bytes(vec![1, 2, 3]), ValueTy::U16),
            Err(ItrErr(ItrErrCode::CastFail, _))
        ));
    }

    #[test]
    fn xlg_ordering_marks_execute_with_display_semantics() {
        use crate::rt::Bytecode;

        let run = |mark: u8, stack_v: Value, local_v: Value| -> (Value, Value) {
            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost::default();

            let mut operands = Stack::new(256);
            let mut locals = Stack::new(256);
            let mut heap = Heap::new(64);
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);
            let cadr = ContractAddress::default();
            let codes = vec![Bytecode::XLG as u8, mark, Bytecode::END as u8];

            let idx = (mark & 0b0001_1111) as u16;
            locals.alloc((idx + 1) as u8).unwrap();
            for i in 0..=idx {
                locals.save(i, Value::U8(0)).unwrap();
            }
            locals.save(idx, local_v).unwrap();
            operands.push(stack_v).unwrap();

            execute_code(
                &mut pc,
                &codes,
                ExecMode::Main,
                false,
                0,
                &mut gas,
                &GasTable::new(1),
                &GasExtra::new(1),
                &SpaceCap::new(1),
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            )
            .unwrap();

            let out = operands.pop().unwrap();
            let local_after = locals.load(idx as usize).unwrap();
            (out, local_after)
        };

        let (v40, l40) = run((4 << 5) | 0, Value::U8(3), Value::U8(2)); // 2 > 3
        assert_eq!(v40, Value::Bool(false));
        assert_eq!(l40, Value::U8(2));
        let (v41, l41) = run((4 << 5) | 0, Value::U8(1), Value::U8(2)); // 2 > 1
        assert_eq!(v41, Value::Bool(true));
        assert_eq!(l41, Value::U8(2));
        let (v50, l50) = run((5 << 5) | 0, Value::U8(2), Value::U8(2)); // 2 >= 2
        assert_eq!(v50, Value::Bool(true));
        assert_eq!(l50, Value::U8(2));
        let (v61, l61) = run((6 << 5) | 1, Value::U8(1), Value::U8(2)); // 2 < 1
        assert_eq!(v61, Value::Bool(false));
        assert_eq!(l61, Value::U8(2));
        let (v60, l60) = run((6 << 5) | 0, Value::U8(3), Value::U8(2)); // 2 < 3
        assert_eq!(v60, Value::Bool(true));
        assert_eq!(l60, Value::U8(2));
        let (v70, l70) = run((7 << 5) | 0, Value::U8(2), Value::U8(2)); // 2 <= 2
        assert_eq!(v70, Value::Bool(true));
        assert_eq!(l70, Value::U8(2));
    }

    #[test]
    fn xop_marks_execute_with_assignment_semantics() {
        use crate::rt::Bytecode;

        let run = |mark: u8, local_init: Value, rhs: Value| -> Value {
            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost::default();

            let mut operands = Stack::new(256);
            let mut locals = Stack::new(256);
            let mut heap = Heap::new(64);
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);
            let cadr = ContractAddress::default();
            let codes = vec![Bytecode::XOP as u8, mark, Bytecode::END as u8];

            let idx = (mark & 0b0011_1111) as u16;
            locals.alloc((idx + 1) as u8).unwrap();
            for i in 0..=idx {
                locals.save(i, Value::U8(0)).unwrap();
            }
            locals.save(idx, local_init).unwrap();
            operands.push(rhs).unwrap();

            execute_code(
                &mut pc,
                &codes,
                ExecMode::Main,
                false,
                0,
                &mut gas,
                &GasTable::new(1),
                &GasExtra::new(1),
                &SpaceCap::new(1),
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            )
            .unwrap();

            locals.load(idx as usize).unwrap()
        };

        assert_eq!(run((0 << 6) | 0, Value::U8(2), Value::U8(3)), Value::U8(5));  // +=
        assert_eq!(run((1 << 6) | 1, Value::U8(9), Value::U8(4)), Value::U8(5));  // -= (idx=1)
        assert_eq!(run((2 << 6) | 0, Value::U8(3), Value::U8(4)), Value::U8(12)); // *=
        assert_eq!(run((3 << 6) | 0, Value::U8(12), Value::U8(3)), Value::U8(4)); // /=
    }

    #[test]
    fn hreadul_charges_dynamic_read_bytes() {
        use crate::rt::Bytecode;

        let run = |mark_hi: u8, mark_lo: u8| -> i64 {
            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost::default();

            let mut operands = Stack::new(256);
            let mut locals = Stack::new(256);
            let mut heap = Heap::new(64);
            heap.grow(1).unwrap();
            heap.write(0, Value::Bytes(vec![0u8; 16])).unwrap();
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);

            let cadr = ContractAddress::default();
            let codes = vec![Bytecode::HREADUL as u8, mark_hi, mark_lo, Bytecode::END as u8];

            execute_code(
                &mut pc,
                &codes,
                ExecMode::Main,
                false,
                0,
                &mut gas,
                &GasTable::new(1),
                &GasExtra::new(1),
                &SpaceCap::new(1),
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            )
            .unwrap();
            1000 - gas
        };

        // mark=0x0000 -> u8 at segment 0, len=1
        let gas_u8 = run(0x00, 0x00);
        // mark=0x8000 -> u128 at segment 0, len=16
        let gas_u128 = run(0x80, 0x00);

        assert_eq!(gas_u128, gas_u8, "HREADUL u8 and u128 are both in the first ceil bucket (<=16 bytes)");
    }

    #[test]
    fn extenv_return_value_is_metered() {
        use crate::rt::Bytecode;

        let run = |idx: u8, ret: Vec<u8>| -> i64 {
            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost { ext_res: ret, ..Default::default() };

            let mut operands = Stack::new(256);
            let mut locals = Stack::new(256);
            let mut heap = Heap::new(64);
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);

            let cadr = ContractAddress::default();
            let codes = vec![Bytecode::EXTENV as u8, idx, Bytecode::END as u8];

            execute_code(
                &mut pc,
                &codes,
                ExecMode::Main,
                false,
                0,
                &mut gas,
                &GasTable::new(1),
                &GasExtra::new(1),
                &SpaceCap::new(1),
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            )
            .unwrap();
            1000 - gas
        };

        // idx=1 -> EnvHeight (U64, 8 bytes), 8/16 = 0 extra
        let gas_u64 = run(1, 1u64.to_be_bytes().to_vec());
        // idx=2 -> EnvMainAddr (Address, 21 bytes), 21/16 = 1 extra
        let gas_addr = run(2, Address::default().to_vec());

        assert_eq!(gas_addr, gas_u64 + 1, "EXTENV should meter return value bytes");
    }

    #[test]
    fn srest_is_fixed_gas_without_dynamic_bytes() {
        use crate::rt::Bytecode;

        let run = |retv: Value| -> i64 {
            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost { srest_res: Some(retv), ..Default::default() };

            let mut operands = Stack::new(256);
            operands.push(Value::U8(1)).unwrap();
            let mut locals = Stack::new(256);
            let mut heap = Heap::new(64);
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);

            let cadr = ContractAddress::default();
            let codes = vec![Bytecode::SREST as u8, Bytecode::END as u8];

            execute_code(
                &mut pc,
                &codes,
                ExecMode::Main,
                false,
                0,
                &mut gas,
                &GasTable::new(1),
                &GasExtra::new(1),
                &SpaceCap::new(1),
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            )
            .unwrap();
            1000 - gas
        };

        let gas_u8 = run(Value::U8(7));
        let gas_addr = run(Value::Address(Address::default()));
        assert_eq!(gas_addr, gas_u8, "SREST should be fixed gas without return-size dynamic billing");

        let gst = GasTable::new(1);
        let expect = gst.gas(Bytecode::SREST as u8) + gst.gas(Bytecode::END as u8);
        assert_eq!(gas_u8, expect);
    }

    #[test]
    fn ntenv_return_value_is_metered() {
        use crate::native::NativeEnv;
        use crate::rt::Bytecode;

        let mut pc: usize = 0;
        let mut gas: i64 = 1000;
        let mut host = DummyHost::default();

        let mut operands = Stack::new(256);
        let mut locals = Stack::new(256);
        let mut heap = Heap::new(64);
        let mut globals = GKVMap::new(20);
        let mut memorys = CtcKVMap::new(12);

        let cadr = ContractAddress::default();
        let idx = NativeEnv::idx_context_address;
        let codes = vec![Bytecode::NTENV as u8, idx, Bytecode::END as u8];

        let gas_table = GasTable::new(1);
        let gas_extra = GasExtra::new(1);
        execute_code(
            &mut pc,
            &codes,
            ExecMode::Main,
            false,
            0,
            &mut gas,
            &gas_table,
            &gas_extra,
            &SpaceCap::new(1),
            &mut operands,
            &mut locals,
            &mut heap,
            &mut globals,
            &mut memorys,
            &mut host,
            &cadr,
            &cadr,
        )
        .unwrap();

        let expect = gas_table.gas(Bytecode::NTENV as u8)
            + NativeEnv::gas(idx).unwrap()
            + gas_extra.ntfunc_bytes(field::Address::SIZE)
            + gas_table.gas(Bytecode::END as u8);
        assert_eq!(1000 - gas, expect);
    }

    #[test]
    fn get0_stack_copy_uses_val_size() {
        use crate::rt::Bytecode;

        let run = |v: Value| -> i64 {
            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost::default();

            let mut operands = Stack::new(256);
            let mut locals = Stack::new(256);
            locals.alloc(1).unwrap();
            locals.save(0, v).unwrap();

            let mut heap = Heap::new(64);
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);

            let cadr = ContractAddress::default();
            let codes = vec![Bytecode::GET0 as u8, Bytecode::END as u8];

            execute_code(
                &mut pc,
                &codes,
                ExecMode::Main,
                false,
                0,
                &mut gas,
                &GasTable::new(1),
                &GasExtra::new(1),
                &SpaceCap::new(1),
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            )
            .unwrap();
            1000 - gas
        };

        let gas_32 = run(Value::Bytes(vec![0u8; 32]));
        let gas_33 = run(Value::Bytes(vec![0u8; 33]));
        assert_eq!(gas_33, gas_32 + 1);
    }

    #[test]
    fn itemget_compo_bytes_counts_non_bytes_values() {
        use crate::rt::Bytecode;

        let mut pc: usize = 0;
        let mut gas: i64 = 1000;
        let mut host = DummyHost::default();

        let mut operands = Stack::new(256);
        let mut locals = Stack::new(256);
        let mut heap = Heap::new(64);
        let mut globals = GKVMap::new(20);
        let mut memorys = CtcKVMap::new(12);

        let mut map = CompoItem::new_map();
        let cap = SpaceCap::new(1);
        map.insert(&cap, Value::U8(1), Value::Address(Address::default())).unwrap();
        operands.push(Value::Compo(map)).unwrap();
        operands.push(Value::U8(1)).unwrap();

        let cadr = ContractAddress::default();
        let codes = vec![Bytecode::ITEMGET as u8, Bytecode::END as u8];

        execute_code(
            &mut pc,
            &codes,
            ExecMode::Main,
            false,
            0,
            &mut gas,
            &GasTable::new(1),
            &GasExtra::new(1),
            &cap,
            &mut operands,
            &mut locals,
            &mut heap,
            &mut globals,
            &mut memorys,
            &mut host,
            &cadr,
            &cadr,
        )
        .unwrap();

        let gas_table = GasTable::new(1);
        let gas_extra = GasExtra::new(1);
        let expect = gas_table.gas(Bytecode::ITEMGET as u8)
            + gas_table.gas(Bytecode::END as u8)
            + gas_extra.compo_items_read(1)
            + gas_extra.compo_bytes(field::Address::SIZE);
        assert_eq!(1000 - gas, expect);
    }

    #[test]
    fn mget_stack_copy_uses_val_size() {
        use crate::rt::Bytecode;

        let run = |v: Value| -> i64 {
            run_with_setup(
                vec![Bytecode::MGET as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, _globals, memorys, cadr| {
                    let key = Value::Bytes(vec![1u8]);
                    memorys.entry_mut(cadr).unwrap().put(key.clone(), v).unwrap();
                    ops.push(key).unwrap();
                },
            )
        };

        let gas_32 = run(Value::Bytes(vec![0u8; 32]));
        let gas_33 = run(Value::Bytes(vec![0u8; 33]));
        assert_eq!(gas_33, gas_32 + 1);
    }

    #[test]
    fn dup_stack_copy_uses_val_size() {
        use crate::rt::Bytecode;

        let run = |v: Value| -> i64 {
            run_with_setup(
                vec![Bytecode::DUP as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, _globals, _memorys, _cadr| {
                    ops.push(v).unwrap();
                },
            )
        };

        let gas_32 = run(Value::Bytes(vec![0u8; 32]));
        let gas_33 = run(Value::Bytes(vec![0u8; 33]));
        assert_eq!(gas_33, gas_32 + 1);
    }

    #[test]
    fn gget_stack_copy_uses_val_size() {
        use crate::rt::Bytecode;

        let run = |v: Value| -> i64 {
            run_with_setup(
                vec![Bytecode::GGET as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, globals, _memorys, _cadr| {
                    let key = Value::Bytes(vec![2u8]);
                    globals.put(key.clone(), v).unwrap();
                    ops.push(key).unwrap();
                },
            )
        };

        let gas_32 = run(Value::Bytes(vec![0u8; 32]));
        let gas_33 = run(Value::Bytes(vec![0u8; 33]));
        assert_eq!(gas_33, gas_32 + 1);
    }

    #[test]
    fn put_space_write_uses_val_size() {
        use crate::rt::Bytecode;

        let run = |v: Value| -> i64 {
            run_with_setup(
                vec![Bytecode::PUT as u8, 0, Bytecode::END as u8],
                DummyHost::default(),
                |ops, locals, _heap, _globals, _memorys, _cadr| {
                    locals.alloc(1).unwrap();
                    ops.push(v).unwrap();
                },
            )
        };

        let gas_24 = run(Value::Bytes(vec![0u8; 24]));
        let gas_25 = run(Value::Bytes(vec![0u8; 25]));
        assert_eq!(gas_25, gas_24 + 1);
    }

    #[test]
    fn putx_space_write_uses_val_size() {
        use crate::rt::Bytecode;

        let run = |v: Value| -> i64 {
            run_with_setup(
                vec![Bytecode::PUTX as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, locals, _heap, _globals, _memorys, _cadr| {
                    locals.alloc(1).unwrap();
                    ops.push(Value::U16(0)).unwrap();
                    ops.push(v).unwrap();
                },
            )
        };

        let gas_24 = run(Value::Bytes(vec![0u8; 24]));
        let gas_25 = run(Value::Bytes(vec![0u8; 25]));
        assert_eq!(gas_25, gas_24 + 1);
    }

    #[test]
    fn mput_space_write_uses_val_size() {
        use crate::rt::Bytecode;

        let key = Value::Bytes(vec![0x35u8]);
        let run = |v: Value| -> i64 {
            run_with_setup(
                vec![Bytecode::MPUT as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, _globals, memorys, cadr| {
                    memorys
                        .entry_mut(cadr)
                        .unwrap()
                        .put(key.clone(), Value::U8(1))
                        .unwrap();
                    ops.push(key.clone()).unwrap();
                    ops.push(v).unwrap();
                },
            )
        };

        let gas_24 = run(Value::Bytes(vec![0u8; 24]));
        let gas_25 = run(Value::Bytes(vec![0u8; 25]));
        assert_eq!(gas_25, gas_24 + 1);
    }

    #[test]
    fn gput_space_write_uses_val_size() {
        use crate::rt::Bytecode;

        let key = Value::Bytes(vec![0x36u8]);
        let run = |v: Value| -> i64 {
            run_with_setup(
                vec![Bytecode::GPUT as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, globals, _memorys, _cadr| {
                    globals.put(key.clone(), Value::U8(1)).unwrap();
                    ops.push(key.clone()).unwrap();
                    ops.push(v).unwrap();
                },
            )
        };

        let gas_24 = run(Value::Bytes(vec![0u8; 24]));
        let gas_25 = run(Value::Bytes(vec![0u8; 25]));
        assert_eq!(gas_25, gas_24 + 1);
    }

    #[test]
    fn clear_charges_edit_items_and_bytes() {
        use crate::rt::Bytecode;
        use std::collections::VecDeque;

        let run = |blen: usize| -> i64 {
            run_with_setup(
                vec![Bytecode::CLEAR as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, _globals, _memorys, _cadr| {
                    let list = Value::Compo(
                        CompoItem::list(VecDeque::from(vec![Value::Bytes(vec![0u8; blen])])).unwrap(),
                    );
                    ops.push(list).unwrap();
                },
            )
        };

        assert_eq!(run(41), run(40) + 1);
    }

    #[test]
    fn insert_map_charges_key_bytes() {
        use crate::rt::Bytecode;

        let run = |klen: usize| -> i64 {
            run_with_setup(
                vec![Bytecode::INSERT as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, _globals, _memorys, _cadr| {
                    ops.push(Value::Compo(CompoItem::new_map())).unwrap();
                    ops.push(Value::Bytes(vec![1u8; klen])).unwrap();
                    ops.push(Value::U8(7)).unwrap();
                },
            )
        };

        assert_eq!(run(41), run(40) + 1);
    }

    #[test]
    fn gput_charges_key_bytes_by_stack_write_div() {
        use crate::rt::Bytecode;

        let run = |klen: usize| -> i64 {
            run_with_setup(
                vec![Bytecode::GPUT as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, globals, _memorys, _cadr| {
                    let key = Value::Bytes(vec![0x55u8; klen]);
                    globals.put(key.clone(), Value::U8(1)).unwrap();
                    ops.push(key).unwrap();
                    ops.push(Value::U8(9)).unwrap();
                },
            )
        };

        assert_eq!(run(25), run(24) + 1);
    }

    #[test]
    fn mput_charges_key_bytes_by_stack_write_div() {
        use crate::rt::Bytecode;

        let run = |klen: usize| -> i64 {
            run_with_setup(
                vec![Bytecode::MPUT as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, _globals, memorys, cadr| {
                    let key = Value::Bytes(vec![0x66u8; klen]);
                    memorys
                        .entry_mut(cadr)
                        .unwrap()
                        .put(key.clone(), Value::U8(1))
                        .unwrap();
                    ops.push(key).unwrap();
                    ops.push(Value::U8(9)).unwrap();
                },
            )
        };

        assert_eq!(run(25), run(24) + 1);
    }

    #[test]
    fn rev_has_no_dynamic_copy_gas() {
        use crate::rt::Bytecode;

        let run = |v1: Value, v2: Value| -> i64 {
            run_with_setup(
                vec![Bytecode::REV as u8, 2, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, _globals, _memorys, _cadr| {
                    ops.push(v1).unwrap();
                    ops.push(v2).unwrap();
                },
            )
        };

        let gas_small = run(Value::U8(1), Value::U8(2));
        let gas_large = run(Value::Bytes(vec![0u8; 64]), Value::Bytes(vec![0u8; 64]));
        assert_eq!(gas_large, gas_small, "REV should not meter dynamic bytes");
    }

    #[test]
    fn cat_dynamic_gas_uses_stack_op_div_20() {
        use crate::rt::Bytecode;

        let run = |l1: usize, l2: usize| -> i64 {
            run_with_setup(
                vec![Bytecode::CAT as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, _globals, _memorys, _cadr| {
                    ops.push(Value::Bytes(vec![1u8; l1])).unwrap();
                    ops.push(Value::Bytes(vec![2u8; l2])).unwrap();
                },
            )
        };

        assert_eq!(run(11, 10), run(10, 10) + 1);
    }

    #[test]
    fn left_dynamic_gas_uses_stack_op_div_20() {
        use crate::rt::Bytecode;

        let run = |take: u8| -> i64 {
            run_with_setup(
                vec![Bytecode::LEFT as u8, take, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, _globals, _memorys, _cadr| {
                    ops.push(Value::Bytes(vec![9u8; 64])).unwrap();
                },
            )
        };

        assert_eq!(run(21), run(20), "LEFT currently meters by input size");
    }

    #[test]
    fn extview_meters_input_and_return_bytes() {
        use crate::rt::Bytecode;

        // view idx=2 -> returns Bytes; input body comes from stack top bytes.
        let run = |input_len: usize, ret_len: usize| -> i64 {
            run_with_setup(
                vec![Bytecode::EXTVIEW as u8, 2, Bytecode::END as u8],
                DummyHost {
                    ext_res: vec![0u8; ret_len],
                    ..Default::default()
                },
                |ops, _locals, _heap, _globals, _memorys, _cadr| {
                    ops.push(Value::Bytes(vec![9u8; input_len])).unwrap();
                },
            )
        };

        assert_eq!(run(17, 0), run(16, 0) + 1);
        assert_eq!(run(0, 17), run(0, 16) + 1);
    }

    #[test]
    fn extaction_meters_input_body_bytes() {
        use crate::rt::Bytecode;

        // action idx=1 is valid in allowlist.
        let run = |input_len: usize| -> i64 {
            run_with_setup(
                vec![Bytecode::EXTACTION as u8, 1, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, _globals, _memorys, _cadr| {
                    ops.push(Value::Bytes(vec![7u8; input_len])).unwrap();
                },
            )
        };

        assert_eq!(run(11), run(10) + 1);
        assert_eq!(run(21), run(20) + 1);
    }

    #[test]
    fn extaction_host_error_still_charges_input_dynamic_gas() {
        use crate::rt::Bytecode;
        use sys::UNWIND_PREFIX;

        let mut pc: usize = 0;
        let mut gas: i64 = 1000;
        let mut host = DummyHost{
            ext_err: Some(format!("{}mock ext recoverable fail", UNWIND_PREFIX)),
            ..Default::default()
        };
        let mut operands = Stack::new(256);
        let mut locals = Stack::new(256);
        let mut heap = Heap::new(64);
        let mut globals = GKVMap::new(20);
        let mut memorys = CtcKVMap::new(12);
        let cadr = ContractAddress::default();
        let codes = vec![Bytecode::EXTACTION as u8, 1, Bytecode::END as u8];
        operands.push(Value::Bytes(vec![7u8; 21])).unwrap();
        let gas_table = GasTable::new(1);
        let gas_extra = GasExtra::new(1);

        let res = execute_code(
            &mut pc,
            &codes,
            ExecMode::Main,
            false,
            0,
            &mut gas,
            &gas_table,
            &gas_extra,
            &SpaceCap::new(1),
            &mut operands,
            &mut locals,
            &mut heap,
            &mut globals,
            &mut memorys,
            &mut host,
            &cadr,
            &cadr,
        );

        assert!(matches!(res, Err(ItrErr(ItrErrCode::ExtActCallError, _))));
        let used = 1000 - gas;
        let expected = gas_table.gas(Bytecode::EXTACTION as u8) + gas_extra.extaction_bytes(21);
        assert_eq!(used, expected);
    }

    #[test]
    fn extaction_host_error_out_of_gas_has_higher_priority() {
        use crate::rt::Bytecode;
        use sys::UNWIND_PREFIX;

        let mut pc: usize = 0;
        let mut host = DummyHost{
            ext_err: Some(format!("{}mock ext recoverable fail", UNWIND_PREFIX)),
            ..Default::default()
        };
        let mut operands = Stack::new(256);
        let mut locals = Stack::new(256);
        let mut heap = Heap::new(64);
        let mut globals = GKVMap::new(20);
        let mut memorys = CtcKVMap::new(12);
        let cadr = ContractAddress::default();
        let codes = vec![Bytecode::EXTACTION as u8, 1, Bytecode::END as u8];
        operands.push(Value::Bytes(vec![7u8; 21])).unwrap();
        let gas_table = GasTable::new(1);
        let gas_extra = GasExtra::new(1);
        let expected = gas_table.gas(Bytecode::EXTACTION as u8) + gas_extra.extaction_bytes(21);
        let mut gas = expected - 1;

        let res = execute_code(
            &mut pc,
            &codes,
            ExecMode::Main,
            false,
            0,
            &mut gas,
            &gas_table,
            &gas_extra,
            &SpaceCap::new(1),
            &mut operands,
            &mut locals,
            &mut heap,
            &mut globals,
            &mut memorys,
            &mut host,
            &cadr,
            &cadr,
        );

        assert!(matches!(res, Err(ItrErr(ItrErrCode::OutOfGas, _))));
    }

    #[test]
    fn heapslice_can_be_used_as_ext_and_nt_call_param() {
        use crate::native::NativeFunc;
        use crate::rt::Bytecode;

        let cadr = ContractAddress::default();

        {
            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost::default();
            let mut operands = Stack::new(256);
            operands.push(Value::HeapSlice((1, 2))).unwrap();
            let mut locals = Stack::new(256);
            let mut heap = Heap::new(64);
            heap.grow(1).unwrap();
            heap.write(0, Value::Bytes(vec![9, 8, 7, 6])).unwrap();
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);
            let codes = vec![Bytecode::EXTACTION as u8, 1, Bytecode::END as u8];

            execute_code(
                &mut pc,
                &codes,
                ExecMode::Main,
                false,
                0,
                &mut gas,
                &GasTable::new(1),
                &GasExtra::new(1),
                &SpaceCap::new(1),
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            )
            .unwrap();
            assert_eq!(host.ext_body, vec![8, 7]);
        }

        {
            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost::default();
            let mut operands = Stack::new(256);
            operands.push(Value::HeapSlice((1, 2))).unwrap();
            let mut locals = Stack::new(256);
            let mut heap = Heap::new(64);
            heap.grow(1).unwrap();
            heap.write(0, Value::Bytes(vec![9, 8, 7, 6])).unwrap();
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);
            let codes = vec![Bytecode::NTFUNC as u8, NativeFunc::idx_sha2, Bytecode::END as u8];

            execute_code(
                &mut pc,
                &codes,
                ExecMode::Main,
                false,
                0,
                &mut gas,
                &GasTable::new(1),
                &GasExtra::new(1),
                &SpaceCap::new(1),
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            )
            .unwrap();
            assert_eq!(
                operands.pop().unwrap(),
                Value::Bytes(sys::sha2(&[8, 7]).to_vec())
            );
        }
    }

    #[test]
    fn hread_dynamic_gas_uses_read_length() {
        use crate::rt::Bytecode;

        let run = |len: u16| -> i64 {
            run_with_setup(
                vec![Bytecode::HREAD as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, heap, _globals, _memorys, _cadr| {
                    heap.grow(1).unwrap();
                    ops.push(Value::U16(0)).unwrap(); // start
                    ops.push(Value::U16(len)).unwrap(); // len
                },
            )
        };

        assert_eq!(run(17), run(16) + 1);
    }

    #[test]
    fn hwrite_dynamic_gas_uses_value_size() {
        use crate::rt::Bytecode;

        let run = |v: Value| -> i64 {
            run_with_setup(
                vec![Bytecode::HWRITE as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, heap, _globals, _memorys, _cadr| {
                    heap.grow(1).unwrap();
                    ops.push(Value::U16(0)).unwrap(); // start
                    ops.push(v).unwrap(); // value
                },
            )
        };

        // heap write byte/12
        let gas_u8 = run(Value::U8(1)); // 1/12 = 0
        let gas_16 = run(Value::U128(1)); // 16/12 = 1
        assert_eq!(gas_16, gas_u8 + 1);
    }

    #[test]
    fn log_dynamic_gas_uses_sum_of_value_bytes() {
        use crate::rt::Bytecode;

        let run = |data_len: usize| -> i64 {
            run_with_setup(
                vec![Bytecode::LOG2 as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, _globals, _memorys, _cadr| {
                    // LOG2 pops 3 values: topic1, topic2, data.
                    ops.push(Value::U8(1)).unwrap();
                    ops.push(Value::U8(2)).unwrap();
                    ops.push(Value::Bytes(vec![0u8; data_len])).unwrap();
                },
            )
        };

        // log byte/1
        assert_eq!(run(10), run(9) + 1);
        assert_eq!(run(100), run(99) + 1);
    }

    #[test]
    fn sload_dynamic_gas_uses_return_value_size() {
        use crate::rt::Bytecode;

        let run = |retv: Value| -> i64 {
            run_with_setup(
                vec![Bytecode::SLOAD as u8, Bytecode::END as u8],
                DummyHost {
                    sload_res: Some(retv),
                    ..Default::default()
                },
                |ops, _locals, _heap, _globals, _memorys, _cadr| {
                    ops.push(Value::U8(1)).unwrap(); // key placeholder
                },
            )
        };

        let gas_8 = run(Value::Bytes(vec![0u8; 8]));
        let gas_9 = run(Value::Bytes(vec![0u8; 9]));
        assert_eq!(gas_9, gas_8 + 1);
    }

    #[test]
    fn sdel_charges_storage_delete_min_dynamic_gas() {
        use crate::rt::Bytecode;
        use sys::Ret;

        struct SdelOkHost;
        impl VmHost for SdelOkHost {
            fn height(&mut self) -> u64 { 1 }
            fn ext_action_call(&mut self, _kid: u16, _body: Vec<u8>) -> Ret<(u32, Vec<u8>)> { unreachable!() }
            fn log_push(&mut self, _cadr: &ContractAddress, _items: Vec<Value>) -> VmrtErr { unreachable!() }
            fn srest(&mut self, _hei: u64, _cadr: &ContractAddress, _key: &Value) -> VmrtRes<Value> { unreachable!() }
            fn sload(&mut self, _hei: u64, _cadr: &ContractAddress, _key: &Value) -> VmrtRes<Value> { unreachable!() }
            fn sdel(&mut self, _cadr: &ContractAddress, _key: Value) -> VmrtErr { Ok(()) }
            fn ssave(
                &mut self,
                _gst: &GasExtra,
                _hei: u64,
                _cadr: &ContractAddress,
                _key: Value,
                _val: Value,
            ) -> VmrtRes<i64> { unreachable!() }
            fn srent(
                &mut self,
                _gst: &GasExtra,
                _hei: u64,
                _cadr: &ContractAddress,
                _key: Value,
                _period: Value,
            ) -> VmrtRes<i64> { unreachable!() }
        }

        let mut pc: usize = 0;
        let mut gas: i64 = 1000;
        let gas_table = GasTable::new(1);
        let gas_extra = GasExtra::new(1);
        let mut host = SdelOkHost;

        let mut operands = Stack::new(256);
        operands.push(Value::Bytes(vec![1u8])).unwrap();
        let mut locals = Stack::new(256);
        let mut heap = Heap::new(64);
        let mut globals = GKVMap::new(20);
        let mut memorys = CtcKVMap::new(12);
        let cadr = ContractAddress::default();
        let codes = vec![Bytecode::SDEL as u8, Bytecode::END as u8];

        execute_code(
            &mut pc,
            &codes,
            ExecMode::Main,
            false,
            0,
            &mut gas,
            &gas_table,
            &gas_extra,
            &SpaceCap::new(1),
            &mut operands,
            &mut locals,
            &mut heap,
            &mut globals,
            &mut memorys,
            &mut host,
            &cadr,
            &cadr,
        )
        .unwrap();

        let expect = gas_table.gas(Bytecode::SDEL as u8)
            + gas_table.gas(Bytecode::END as u8)
            + gas_extra.storage_del();
        assert_eq!(1000 - gas, expect);
        assert_eq!(gas_extra.storage_del(), 16);
    }

    #[test]
    fn keys_and_clone_compo_bytes_include_map_key_bytes() {
        use crate::rt::Bytecode;
        use std::collections::BTreeMap;

        let map_with_key_len = |klen: usize| -> Value {
            let mut m = BTreeMap::new();
            m.insert(vec![7u8; klen], Value::U8(1));
            Value::Compo(CompoItem::map(m).unwrap())
        };

        let gas_keys_40 = run_with_setup(
            vec![Bytecode::KEYS as u8, Bytecode::END as u8],
            DummyHost::default(),
            |ops, _locals, _heap, _globals, _memorys, _cadr| {
                ops.push(map_with_key_len(40)).unwrap();
            },
        );
        let gas_keys_41 = run_with_setup(
            vec![Bytecode::KEYS as u8, Bytecode::END as u8],
            DummyHost::default(),
            |ops, _locals, _heap, _globals, _memorys, _cadr| {
                ops.push(map_with_key_len(41)).unwrap();
            },
        );
        assert_eq!(gas_keys_41, gas_keys_40 + 1, "KEYS byte/40 should include key bytes");

        let gas_clone_39 = run_with_setup(
            vec![Bytecode::CLONE as u8, Bytecode::END as u8],
            DummyHost::default(),
            |ops, _locals, _heap, _globals, _memorys, _cadr| {
                ops.push(map_with_key_len(39)).unwrap(); // 39(key)+1(val)=40
            },
        );
        let gas_clone_40 = run_with_setup(
            vec![Bytecode::CLONE as u8, Bytecode::END as u8],
            DummyHost::default(),
            |ops, _locals, _heap, _globals, _memorys, _cadr| {
                ops.push(map_with_key_len(40)).unwrap(); // 40+1=41
            },
        );
        assert_eq!(gas_clone_40, gas_clone_39 + 1, "CLONE byte/40 should include key bytes");
    }

    #[test]
    fn alloc_uses_local_slot_cost() {
        use crate::rt::Bytecode;

        let run = |n: u8| -> i64 {
            run_with_setup(
                vec![Bytecode::ALLOC as u8, n, Bytecode::END as u8],
                DummyHost::default(),
                |_ops, _locals, _heap, _globals, _memorys, _cadr| {},
            )
        };

        // local alloc: 5 gas per slot
        assert_eq!(run(3), run(2) + 5);
        assert_eq!(run(10), run(3) + 35);
    }

    #[test]
    fn uplist_space_write_charges_like_put_per_item() {
        use crate::rt::Bytecode;
        use std::collections::VecDeque;

        let run = |item_len: usize| -> i64 {
            run_with_setup(
                vec![Bytecode::UPLIST as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, locals, _heap, _globals, _memorys, _cadr| {
                    locals.alloc(2).unwrap();
                    let list = Value::Compo(
                        CompoItem::list(VecDeque::from(vec![
                            Value::Bytes(vec![0u8; item_len]),
                            Value::Bytes(vec![0u8; item_len]),
                        ]))
                        .unwrap(),
                    );
                    ops.push(list).unwrap();
                    ops.push(Value::U8(0)).unwrap();
                },
            )
        };

        assert_eq!(run(25), run(24) + 2);
    }

    #[test]
    fn uplist_accepts_oversize_value_without_spacecap_validation() {
        use crate::rt::Bytecode;
        use std::collections::VecDeque;

        let mut pc: usize = 0;
        let mut gas: i64 = 1000;
        let mut host = DummyHost::default();

        let mut heap = Heap::new(64);
        let mut globals = GKVMap::new(20);
        let mut memorys = CtcKVMap::new(12);
        let cadr = ContractAddress::default();

        let mut locals = Stack::new(256);
        locals.alloc(2).unwrap();
        locals.save(0, Value::U8(7)).unwrap();
        locals.save(1, Value::U8(9)).unwrap();

        let mut operands = Stack::new(256);
        let list = Value::Compo(
            CompoItem::list(VecDeque::from(vec![
                Value::U8(1),
                Value::Bytes(vec![0u8; 1281]),
            ]))
            .unwrap(),
        );
        operands.push(list).unwrap();
        operands.push(Value::U8(0)).unwrap();

        let codes = vec![Bytecode::UPLIST as u8, Bytecode::END as u8];
        let exit = execute_code(
            &mut pc,
            &codes,
            ExecMode::Main,
            false,
            0,
            &mut gas,
            &GasTable::new(1),
            &GasExtra::new(1),
            &SpaceCap::new(1),
            &mut operands,
            &mut locals,
            &mut heap,
            &mut globals,
            &mut memorys,
            &mut host,
            &cadr,
            &cadr,
        )
        .unwrap();

        assert!(matches!(exit, crate::rt::CallExit::Finish));
        assert_eq!(locals.load(0).unwrap(), Value::U8(1));
        assert_eq!(locals.load(1).unwrap(), Value::Bytes(vec![0u8; 1281]));
    }

    #[test]
    fn mput_charges_memory_key_cost_only_for_new_key() {
        use crate::rt::Bytecode;

        let key = Value::Bytes(vec![0x33u8]);
        let run = |preload: bool| -> i64 {
            run_with_setup(
                vec![Bytecode::MPUT as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, _globals, memorys, cadr| {
                    if preload {
                        memorys
                            .entry_mut(cadr)
                            .unwrap()
                            .put(key.clone(), Value::U8(1))
                            .unwrap();
                    }
                    ops.push(key.clone()).unwrap();
                    ops.push(Value::U8(9)).unwrap();
                },
            )
        };

        assert_eq!(run(false), run(true) + 20);
    }

    #[test]
    fn gput_charges_global_key_cost_only_for_new_key() {
        use crate::rt::Bytecode;

        let key = Value::Bytes(vec![0x44u8]);
        let run = |preload: bool| -> i64 {
            run_with_setup(
                vec![Bytecode::GPUT as u8, Bytecode::END as u8],
                DummyHost::default(),
                |ops, _locals, _heap, globals, _memorys, _cadr| {
                    if preload {
                        globals.put(key.clone(), Value::U8(1)).unwrap();
                    }
                    ops.push(key.clone()).unwrap();
                    ops.push(Value::U8(9)).unwrap();
                },
            )
        };

        assert_eq!(run(false), run(true) + 32);
    }

    #[test]
    fn merge_rejects_self_merge_after_dup() {
        use crate::rt::Bytecode;

        for init in [Bytecode::NEWLIST, Bytecode::NEWMAP] {
            let mut pc: usize = 0;
            let mut gas: i64 = 1000;
            let mut host = DummyHost::default();

            let mut operands = Stack::new(256);
            let mut locals = Stack::new(256);
            let mut heap = Heap::new(64);
            let mut globals = GKVMap::new(20);
            let mut memorys = CtcKVMap::new(12);

            let cadr = ContractAddress::default();
            let codes = vec![
                init as u8,
                Bytecode::DUP as u8,
                Bytecode::MERGE as u8,
                Bytecode::END as u8,
            ];
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
                &mut operands,
                &mut locals,
                &mut heap,
                &mut globals,
                &mut memorys,
                &mut host,
                &cadr,
                &cadr,
            );
            assert!(matches!(res, Err(ItrErr(ItrErrCode::CompoOpInvalid, _))));
        }
    }
}



#[cfg(test)]
mod bounds_tests {
    use super::*;
    use crate::machine::VmHost;
    use crate::rt::{ExecMode, GasExtra, GasTable, ItrErr, ItrErrCode, SpaceCap, VmrtErr, VmrtRes};
    use crate::space::{CtcKVMap, GKVMap, Heap, Stack};
    use crate::value::{CompoItem, Value};
    use field::Address;
    use crate::ContractAddress;
    use sys::Ret;

    #[derive(Default)]
    struct DummyHost {
        ext_res: Vec<u8>,
        ext_gas: u32,
        srest_res: Option<Value>,
        sload_res: Option<Value>,
        log_calls: usize,
    }

    impl VmHost for DummyHost {
        fn height(&mut self) -> u64 {
            1
        }

        fn ext_action_call(&mut self, _kid: u16, _body: Vec<u8>) -> Ret<(u32, Vec<u8>)> {
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

        assert_eq!(gas_u128, gas_u8 + 1, "HREADUL dynamic read gas should charge +1 for 16-byte read");
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

        let gas_u8 = run(Value::U8(1)); // val_size=1 => 1/32 = 0
        let gas_32 = run(Value::Bytes(vec![0u8; 32])); // val_size=32 => 32/32 = 1
        assert_eq!(gas_32, gas_u8 + 1);
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

        // ITEMGET base=4, END base=1, item gas=1/4=0, value bytes=21 => 21/20=1
        assert_eq!(1000 - gas, 6);
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
                    memorys.entry(cadr).unwrap().put(key.clone(), v).unwrap();
                    ops.push(key).unwrap();
                },
            )
        };

        let gas_u8 = run(Value::U8(7)); // 1/32 = 0
        let gas_32 = run(Value::Bytes(vec![0u8; 32])); // 32/32 = 1
        assert_eq!(gas_32, gas_u8 + 1);
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

        let gas_u8 = run(Value::U8(7)); // 1/32 = 0
        let gas_32 = run(Value::Bytes(vec![0u8; 32])); // 32/32 = 1
        assert_eq!(gas_32, gas_u8 + 1);
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

        let gas_u8 = run(Value::U8(7)); // 1/32 = 0
        let gas_32 = run(Value::Bytes(vec![0u8; 32])); // 32/32 = 1
        assert_eq!(gas_32, gas_u8 + 1);
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

        let gas_u8 = run(Value::U8(9)); // 1/24 = 0
        let gas_24 = run(Value::Bytes(vec![0u8; 24])); // 24/24 = 1
        assert_eq!(gas_24, gas_u8 + 1);
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

        let gas_u8 = run(Value::U8(9)); // 1/24 = 0
        let gas_24 = run(Value::Bytes(vec![0u8; 24])); // 24/24 = 1
        assert_eq!(gas_24, gas_u8 + 1);
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
                        .entry(cadr)
                        .unwrap()
                        .put(key.clone(), Value::U8(1))
                        .unwrap();
                    ops.push(key.clone()).unwrap();
                    ops.push(v).unwrap();
                },
            )
        };

        let gas_u8 = run(Value::U8(9)); // 1/24 = 0
        let gas_24 = run(Value::Bytes(vec![0u8; 24])); // 24/24 = 1
        assert_eq!(gas_24, gas_u8 + 1);
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

        let gas_u8 = run(Value::U8(9)); // 1/24 = 0
        let gas_24 = run(Value::Bytes(vec![0u8; 24])); // 24/24 = 1
        assert_eq!(gas_24, gas_u8 + 1);
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
    fn cat_dynamic_gas_uses_stack_op_div_16() {
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

        // output bytes: 15 -> /16 = 0, 16 -> /16 = 1
        assert_eq!(run(8, 8), run(7, 8) + 1);
    }

    #[test]
    fn left_dynamic_gas_uses_stack_op_div_16() {
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

        assert_eq!(run(16), run(15) + 1);
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

        // input boundary: 15->16 adds +1
        assert_eq!(run(16, 0), run(15, 0) + 1);
        // return boundary: 15->16 adds +1
        assert_eq!(run(0, 16), run(0, 15) + 1);
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

        // extaction byte/10
        assert_eq!(run(10), run(9) + 1);
        assert_eq!(run(20), run(10) + 1);
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

        // heap read byte/16
        assert_eq!(run(16), run(15) + 1);
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

        // storage read byte/8
        let gas_u8 = run(Value::U8(1)); // 1/8 = 0
        let gas_u64 = run(Value::U64(1)); // 8/8 = 1
        assert_eq!(gas_u64, gas_u8 + 1);
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

        let gas_keys_19 = run_with_setup(
            vec![Bytecode::KEYS as u8, Bytecode::END as u8],
            DummyHost::default(),
            |ops, _locals, _heap, _globals, _memorys, _cadr| {
                ops.push(map_with_key_len(19)).unwrap();
            },
        );
        let gas_keys_20 = run_with_setup(
            vec![Bytecode::KEYS as u8, Bytecode::END as u8],
            DummyHost::default(),
            |ops, _locals, _heap, _globals, _memorys, _cadr| {
                ops.push(map_with_key_len(20)).unwrap();
            },
        );
        assert_eq!(gas_keys_20, gas_keys_19 + 1, "KEYS byte/20 should include key bytes");

        let gas_clone_18 = run_with_setup(
            vec![Bytecode::CLONE as u8, Bytecode::END as u8],
            DummyHost::default(),
            |ops, _locals, _heap, _globals, _memorys, _cadr| {
                ops.push(map_with_key_len(18)).unwrap(); // 18(key)+1(val)=19 -> /20 = 0
            },
        );
        let gas_clone_19 = run_with_setup(
            vec![Bytecode::CLONE as u8, Bytecode::END as u8],
            DummyHost::default(),
            |ops, _locals, _heap, _globals, _memorys, _cadr| {
                ops.push(map_with_key_len(19)).unwrap(); // 19+1=20 -> /20 = 1
            },
        );
        assert_eq!(gas_clone_19, gas_clone_18 + 1, "CLONE byte/20 should include key bytes");
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
                            .entry(cadr)
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
}

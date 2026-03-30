/*********************************/
/* VM entry request + guard state */

pub(crate) enum VmEntryReq {
    Main {
        code_type: CodeType,
        codes: Arc<[u8]>,
    },
    P2sh {
        code_type: CodeType,
        state_addr: Address,
        libs: Vec<ContractAddress>,
        codes: ByteView,
        intent_binding: IntentScope,
        param: Value,
    },
    Abst {
        kind: AbstCall,
        contract_addr: ContractAddress,
        intent_binding: IntentScope,
        param: Value,
    },
}

impl VmEntryReq {
    fn entry_kind(&self) -> EntryKind {
        match self {
            Self::Main { .. } => EntryKind::Main,
            Self::P2sh { .. } => EntryKind::P2sh,
            Self::Abst { .. } => EntryKind::Abst,
        }
    }

    fn min_call_cost(&self, gas_extra: &GasExtra) -> i64 {
        self.entry_kind().min_call_cost(gas_extra)
    }

    fn execute(
        self,
        machine: &mut Machine,
        runtime: &mut MachineRuntime,
        ctx: &mut dyn Context,
    ) -> XRet<Value> {
        match self {
            Self::Main { code_type, codes } => machine.main_call(runtime, ctx, code_type, codes),
            Self::P2sh {
                code_type,
                state_addr,
                libs,
                codes,
                intent_binding,
                param,
            } => machine.p2sh_call(runtime, ctx, code_type, state_addr, libs, codes, intent_binding, param),
            Self::Abst {
                kind,
                contract_addr,
                intent_binding,
                param,
            } => machine.abst_call(runtime, ctx, kind, contract_addr, intent_binding, param),
        }
    }
}

struct VmEntryFrame {
    kind: EntryKind,
    gas_base: VmGasBuckets,
    min_cost: i64,
}

#[derive(Clone, Copy)]
enum VmEntryMode {
    KeepCurrentExecFrom,
    ForceCall,
    AssumeAlreadyCall,
}

enum VmEntryFailure {
    Message(String),
    Runtime(crate::rt::ItrErr),
}

struct VmEntryGuard {
    entries: *mut Vec<VmEntryFrame>,
    entry: VmEntryFrame,
    index: usize,
    armed: bool,
}

impl VmEntryGuard {
    fn push<E>(
        entries: &mut Vec<VmEntryFrame>,
        max_reentry: u32,
        gas_base: VmGasBuckets,
        kind: EntryKind,
        min_cost: i64,
        map: fn(VmEntryFailure) -> E,
    ) -> Result<Self, E> {
        let next_level = entries
            .len()
            .checked_add(1)
            .ok_or_else(|| map(VmEntryFailure::Message("re-entry level overflow".to_owned())))?;
        if next_level as u32 > max_reentry + 1 {
            return Err(map(VmEntryFailure::Message(format!(
                "re-entry level {} exceeded limit {}",
                next_level - 1,
                max_reentry
            ))));
        }
        let entry = VmEntryFrame {
            kind,
            gas_base,
            min_cost,
        };
        let index = entries.len();
        entries.push(VmEntryFrame {
            kind: entry.kind,
            gas_base: entry.gas_base,
            min_cost: entry.min_cost,
        });
        Ok(Self {
            entries,
            entry,
            index,
            armed: true,
        })
    }

    fn entry(&self) -> VmEntryFrame {
        VmEntryFrame {
            kind: self.entry.kind,
            gas_base: self.entry.gas_base,
            min_cost: self.entry.min_cost,
        }
    }

    fn disarm(mut self) -> VmEntryFrame {
        let entries = unsafe { &mut *self.entries };
        let popped = entries
            .pop()
            .unwrap_or_else(|| panic!("vm entry frame missing during disarm"));
        debug_assert_eq!(entries.len(), self.index);
        debug_assert_eq!(popped.kind as u8, self.entry.kind as u8);
        self.armed = false;
        self.entry()
    }
}

impl Drop for VmEntryGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let entries = unsafe { &mut *self.entries };
        let Some(popped) = entries.pop() else {
            debug_assert!(false, "vm entry frame missing during guard drop");
            return;
        };
        debug_assert_eq!(entries.len(), self.index);
        debug_assert_eq!(popped.kind as u8, self.entry.kind as u8);
    }
}

/*********************************/
/* Host-facing entry helpers */

fn ensure_vm_run_ready(ctx: &dyn Context) -> Rerr {
    const TY3: u8 = TransactionType3::TYPE;
    let txty = ctx.env().tx.ty;
    if txty < TY3 {
        return errf!(
            "current transaction type {} too low to setup vm, requires at least {}",
            txty,
            TY3
        );
    }
    Ok(())
}

/// Falsy return => success. Non-falsy or object return => recoverable (XError::revert). Runtime-only values crossing the VM boundary are unrecoverable (XError::fault).
pub fn check_vm_return_value(rv: &Value, err_msg: &str) -> XRerr {
    rv.check_vm_boundary_retv()
        .map_err(|e| XError::fault(format!("{} return cannot cross VM boundary: {}", err_msg, e)))?;
    use Value::*;
    let detail: Option<String> = match rv {
        Nil => None,
        Bool(false) => None,
        Bool(true) => Some("code 1".to_owned()),
        U8(n) => (*n != 0).then(|| format!("code {}", n)),
        U16(n) => (*n != 0).then(|| format!("code {}", n)),
        U32(n) => (*n != 0).then(|| format!("code {}", n)),
        U64(n) => (*n != 0).then(|| format!("code {}", n)),
        U128(n) => (*n != 0).then(|| format!("code {}", n)),
        Bytes(buf) => maybe!(
            buf_is_empty_or_all_zero(buf),
            None,
            Some(match ascii_show_string(buf) {
                Some(s) => format!("bytes {:?}", s),
                None => format!("bytes 0x{}", buf.to_hex()),
            })
        ),
        Value::Address(a) => maybe!(
            buf_is_empty_or_all_zero(a.as_bytes()),
            None,
            Some(format!("address {}", a.to_readable()))
        ),
        HeapSlice(_) | Handle(_) => never!(),
        Tuple(_) | Compo(_) => Some(format!("object {}", rv.to_json())),
    };
    match detail {
        None => Ok(()),
        Some(d) => Err(XError::revert(format!("{} return error {}", err_msg, d))),
    }
}

pub fn setup_vm_run_main(
    ctx: &mut dyn Context,
    codeconf: u8,
    payload: Vec<u8>,
) -> Ret<(VmGasBuckets, Value)> {
    // Bytecode verification is intentionally handled by upper-layer action validators before calling setup_vm_run_main.
    ensure_vm_run_ready(ctx)?;
    let (cost, rv) = ctx.vm_call(Box::new(VmEntryReq::Main {
        code_type: CodeConf::parse(codeconf)?.code_type(),
        codes: Arc::from(payload),
    }))?;
    let Ok(rv) = rv.downcast::<Value>() else {
        return errf!("vm call return type mismatch");
    };
    Ok((cost, *rv))
}

pub fn setup_vm_run_p2sh(
    ctx: &mut dyn Context,
    codeconf: u8,
    payload: Vec<u8>,
    param: Value,
) -> Ret<(VmGasBuckets, Value)> {
    // Lock script verification is intentionally handled by upper-layer action validators before calling setup_vm_run_p2sh.
    ensure_vm_run_ready(ctx)?;
    let payload = ByteView::from_arc(Arc::from(payload));
    let payload_ref = payload.as_slice();
    let (state_addr, mv1) = Address::create(payload_ref)?;
    let (libs, mv2) = ContractAddressW1::create(&payload_ref[mv1..])?;
    let mv = mv1 + mv2;
    let intent_binding = ctx.vm_current_intent_scope();
    let (cost, rv) = ctx.vm_call(Box::new(VmEntryReq::P2sh {
        code_type: CodeConf::parse(codeconf)?.code_type(),
        state_addr,
        libs: libs.into_list(),
        codes: payload.slice(mv, payload.len())?,
        intent_binding,
        param,
    }))?;
    let Ok(rv) = rv.downcast::<Value>() else {
        return errf!("vm call return type mismatch");
    };
    Ok((cost, *rv))
}

pub fn setup_vm_run_abst(
    ctx: &mut dyn Context,
    target: AbstCall,
    payload: Address,
    param: Value,
) -> Ret<(VmGasBuckets, Value)> {
    ensure_vm_run_ready(ctx)?;
    let intent_binding = ctx.vm_current_intent_scope();
    let (cost, rv) = ctx.vm_call(Box::new(VmEntryReq::Abst {
        kind: target,
        contract_addr: ContractAddress::from_addr(payload)?,
        intent_binding,
        param,
    }))?;
    let Ok(rv) = rv.downcast::<Value>() else {
        return errf!("vm call return type mismatch");
    };
    Ok((cost, *rv))
}

/*********************************/
/* MachineBox entry orchestration */

impl MachineBox {
    fn entry_failure_to_ret(err: VmEntryFailure) -> String {
        match err {
            VmEntryFailure::Message(msg) => msg,
            VmEntryFailure::Runtime(err) => err.to_string(),
        }
    }

    fn entry_failure_to_xret(err: VmEntryFailure) -> XError {
        match err {
            VmEntryFailure::Message(msg) => XError::fault(msg),
            VmEntryFailure::Runtime(err) => XError::from(err),
        }
    }

    fn append_secondary_message(primary: String, secondary: String) -> String {
        if secondary.is_empty() || primary.contains(&secondary) {
            primary
        } else {
            format!("{} | secondary: {}", primary, secondary)
        }
    }

    fn merge_ret_entry_failure(exec_err: String, settle_err: String) -> String {
        Self::append_secondary_message(exec_err, settle_err)
    }

    fn merge_xret_entry_failure(exec_err: XError, settle_err: XError) -> XError {
        let exec_is_revert = exec_err.is_revert();
        let settle_is_revert = settle_err.is_revert();
        if exec_is_revert && !settle_is_revert {
            match settle_err {
                sys::ExecError::Revert(msg) => XError::fault(Self::append_secondary_message(msg, exec_err.to_string())),
                sys::ExecError::Fault(msg) => XError::fault(Self::append_secondary_message(msg, exec_err.to_string())),
            }
        } else {
            match exec_err {
                sys::ExecError::Revert(msg) => XError::revert(Self::append_secondary_message(msg, settle_err.to_string())),
                sys::ExecError::Fault(msg) => XError::fault(Self::append_secondary_message(msg, settle_err.to_string())),
            }
        }
    }

    fn with_entry_exec_from<T>(
        ctx: &mut dyn Context,
        mode: VmEntryMode,
        f: impl for<'a> FnOnce(&'a mut dyn Context) -> T,
    ) -> T {
        match mode {
            VmEntryMode::KeepCurrentExecFrom => f(ctx),
            VmEntryMode::ForceCall => basis::interface::with_exec_from(
                ctx,
                basis::component::ExecFrom::Call,
                f,
            ),
            VmEntryMode::AssumeAlreadyCall => f(ctx),
        }
    }

    fn run_vm_entry<T, E>(
        &mut self,
        ctx: &mut dyn Context,
        kind: EntryKind,
        min_cost: i64,
        mode: VmEntryMode,
        execute: impl FnOnce(&mut Machine, &mut MachineRuntime, &mut dyn Context) -> Result<T, E>,
        map: fn(VmEntryFailure) -> E,
        merge: fn(E, E) -> E,
    ) -> Result<(VmGasBuckets, T), E> {
        let (max_reentry, gas_base) = {
            let runtime = self
                .runtime_ref()
                .map_err(|e| map(VmEntryFailure::Message(e)))?;
            (runtime.warm.space_cap.reentry_level, runtime.gas_use())
        };
        let guard = VmEntryGuard::push(&mut self.entries, max_reentry, gas_base, kind, min_cost, map)?;
        let result = Self::with_entry_exec_from(ctx, mode, |ctx| {
            let machine = &mut self.machine;
            let Some(runtime) = self.runtime.as_mut() else {
                return Err(map(VmEntryFailure::Message("machine runtime missing".to_owned())));
            };
            execute(machine, runtime, ctx)
        });
        let entry = guard.disarm();
        let settle = match self.runtime_mut() {
            Ok(runtime) => {
                let mut cost = runtime
                    .gas_use()
                    .checked_sub(entry.gas_base)
                    .ok_or_else(|| {
                        map(VmEntryFailure::Message(format!(
                            "gas cost underflow: total={:?}, base={:?}",
                            runtime.gas_use(),
                            entry.gas_base
                        )))
                    })?;
                if cost.total() < entry.min_cost {
                    let delta = entry.min_cost - cost.total();
                    runtime
                        .settle_compute_gas(ctx, delta)
                        .map_err(|e| map(VmEntryFailure::Runtime(e)))?;
                    cost.compute += delta;
                }
                if cost.total() <= 0 {
                    Err(map(VmEntryFailure::Message(format!(
                        "{:?} gas cost invalid: {}",
                        entry.kind,
                        cost.total()
                    ))))
                } else {
                    Ok(cost)
                }
            }
            Err(e) => Err(map(VmEntryFailure::Message(e))),
        };
        match (result, settle) {
            // `VmGasBuckets` returned here are VM-side reporting values for this entry delta.
            // They are not the source of truth for protocol billing: final HAC burn/refund is
            // driven by host/context gas charges that were already applied during execution.
            (Err(exec_err), Err(settle_err)) => Err(merge(exec_err, settle_err)),
            (Err(exec_err), _) => Err(exec_err),
            (Ok(_), Err(settle_err)) => Err(settle_err),
            (Ok(retv), Ok(cost)) => Ok((cost, retv)),
        }
    }

    pub(crate) fn sandbox_main_call_raw_with_gas(
        &mut self,
        ctx: &mut dyn Context,
        ctype: CodeType,
        codes: Arc<[u8]>,
    ) -> Ret<(VmGasBuckets, Value)> {
        let min_cost = {
            let runtime = self.runtime_ref().map_err(|e| e.to_string())?;
            EntryKind::Main.min_call_cost(&runtime.warm.gas_extra)
        };
        self.run_vm_entry(
            ctx,
            EntryKind::Main,
            min_cost,
            VmEntryMode::KeepCurrentExecFrom,
            move |machine, runtime, ctx| machine.main_call_raw(runtime, ctx, ctype, codes),
            Self::entry_failure_to_ret,
            Self::merge_ret_entry_failure,
        )
    }

    fn execute_entry_req(
        &mut self,
        ctx: &mut dyn Context,
        req: VmEntryReq,
    ) -> XRet<(VmGasBuckets, Value)> {
        let (kind, min_cost) = {
            let runtime = self.runtime_ref().map_err(XError::fault)?;
            (req.entry_kind(), req.min_call_cost(&runtime.warm.gas_extra))
        };
        self.run_vm_entry(
            ctx,
            kind,
            min_cost,
            VmEntryMode::AssumeAlreadyCall,
            move |machine, runtime, ctx| req.execute(machine, runtime, ctx),
            Self::entry_failure_to_xret,
            Self::merge_xret_entry_failure,
        )
    }

    fn dispatch_entry_call(
        &mut self,
        ctx: &mut dyn Context,
        req: Box<dyn Any>,
    ) -> XRet<(VmGasBuckets, Box<dyn Any>)> {
        let Ok(req) = req.downcast::<VmEntryReq>() else {
            return xerrf!("vm call request type mismatch");
        };
        let (cost, resv) = self.execute_entry_req(ctx, *req)?;
        Ok((cost, Box::new(resv)))
    }

    fn run_deferred_entries(&mut self, ctx: &mut dyn Context) -> Rerr {
        let callbacks = {
            let r = self.runtime_mut().map_err(|e| e.to_string())?;
            // Deferred phase currently uses strict one-shot consumption: once drained, callbacks are consumed
            // even if a later deferred callback fails. This keeps deferred dispatch non-reentrant and matches the
            // existing transaction semantics where warmups/gas remain monotonic after the phase begins.
            r.volatile.deferred_registry.drain_lifo()
        };
        for caddr in callbacks {
            let _ = self
                .run_vm_entry(
                    ctx,
                    EntryKind::Abst,
                    {
                        let runtime = self.runtime_ref().map_err(|e| e.to_string())?;
                        EntryKind::Abst.min_call_cost(&runtime.warm.gas_extra)
                    },
                    VmEntryMode::ForceCall,
                    move |machine, runtime, ctx| {
                        VmEntryReq::Abst {
                            kind: AbstCall::Deferred,
                            contract_addr: caddr.addr,
                            intent_binding: Some(caddr.intent_id),
                            param: Value::Nil,
                        }
                        .execute(machine, runtime, ctx)
                        .map_err(|e| e.to_string())
                    },
                    Self::entry_failure_to_ret,
                    Self::merge_ret_entry_failure,
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/*********************************/
/* Entry boundary validation */

impl Machine {
    fn validate_vm_entry_param(
        runtime: &MachineRuntime,
        exec: &ExecCtx,
        param: Option<&Value>,
    ) -> XRerr {
        exec.ensure_call_depth(&runtime.warm.space_cap).map_err(XError::from)?;
        if let Some(param) = param {
            param.check_vm_boundary_argv().map_err(XError::from)?;
            param
                .check_container_cap(&runtime.warm.space_cap)
                .map_err(XError::from)?;
        }
        Ok(())
    }

    fn validate_vm_entry_return(kind: EntryKind, rv: &Value, err_msg: &str) -> XRerr {
        match kind {
            EntryKind::Main | EntryKind::P2sh | EntryKind::Abst => check_vm_return_value(rv, err_msg),
        }
    }
}

#[cfg(test)]
mod entry_tests {
    use super::*;

    #[test]
    fn check_vm_return_value_faults_handle_inside_tuple() {
        let rv = Value::Tuple(
            TupleItem::new(vec![Value::U8(1), Value::handle(7u32)]).unwrap(),
        );
        let err = check_vm_return_value(&rv, "main call").unwrap_err();
        assert!(err.is_fault());
        assert!(err.to_string().contains("cannot cross VM boundary"));
    }
}

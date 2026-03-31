impl CallFrame {
    pub(crate) fn start_call<H: VmHost + ?Sized>(
        &mut self,
        r: &mut Runtime,
        host: &mut H,
        exec: ExecCtx,
        code: &FnObj,
        bindings: FrameBindings,
        param: Option<Value>,
    ) -> VmrtRes<Value> {
        use CallExit::*;
        macro_rules! curr { () => { self.frames.last().unwrap() }; }
        macro_rules! curr_mut { () => { self.frames.last_mut().unwrap() }; }
        macro_rules! prepare_and_push {
            ($frame:ident, $prepare:expr) => {{
                if let Err(e) = $prepare {
                    $frame.reclaim(r);
                    return Err(e);
                }
                self.push($frame);
            }};
        }
        macro_rules! settle_return {
            ($retv:expr) => {{
                let mut retv = $retv;
                curr!().check_output_type(&mut retv, &r.warm.space_cap)?;
                self.pop().unwrap().reclaim(r);
                loop {
                    let is_tail = match self.frames.last() {
                        Some(f) => f.pc == f.codes.len(),
                        None => return Ok(retv),
                    };
                    if !is_tail {
                        curr_mut!().push_value(retv)?;
                        break;
                    }
                    self.frames
                        .last()
                        .unwrap()
                        .check_output_type(&mut retv, &r.warm.space_cap)?;
                    self.pop().unwrap().reclaim(r);
                }
            }};
        }

        assert!(self.len() == 0);
        let height = host.height();

        exec.ensure_call_depth(&r.warm.space_cap)?;
        let mut root = self.increase(r)?;
        prepare_and_push!(
            root,
            root.prepare(exec, bindings, code, height, &r.warm.gas_extra, param, &r.warm.space_cap)
        );

        loop {
            let exit = curr_mut!().execute(r, host)?;
            match exit {
                Call(spec) => {
                    let curr_exec = curr!().exec;
                    let curr_bindings = curr!().bindings.clone();
                    let next_effect = spec.callee_effect(curr_exec.effect);
                    let next_exec = curr_exec.enter_call(next_effect, &r.warm.space_cap)?;
                    // Validate local argv boundary before resolving/loading any callee so
                    // malformed input cannot warm caches via either Invoke or Splice.
                    curr_mut!().oprnds.peek()?.check_func_argv()?;
                    curr_mut!().oprnds.peek()?.check_container_cap(&r.warm.space_cap)?;
                    let mut plan = r.plan_user_call(host, &spec, &curr_bindings)?;
                    plan.next_bindings.intent_binding = curr!().intent_state.current();
                    plan.inherited_intent_scope = plan.next_bindings.intent_binding;

                    match spec {
                        CallSpec::Splice { .. } => {
                            let mut param = curr_mut!().pop_value()?;
                            if let Some(vtys) = plan.fnobj.agvty.as_ref() {
                                vtys.check_params(&mut param)?;
                            }
                            curr_mut!().push_value(param.clone())?;
                            curr_mut!().prepare_splice(
                                next_exec,
                                plan.next_bindings,
                                plan.fnobj.as_ref(),
                                height,
                                &r.warm.gas_extra,
                                param,
                                &r.warm.space_cap,
                            )?;
                            continue;
                        }
                        CallSpec::Invoke { .. } => {
                            let param = curr_mut!().pop_value()?;
                            let mut next = self.increase(r)?;
                            prepare_and_push!(
                                next,
                                next.prepare_invoke_unchecked_shape(
                                    next_exec,
                                    plan.next_bindings,
                                    plan.fnobj.as_ref(),
                                    height,
                                    &r.warm.gas_extra,
                                    param,
                                    &r.warm.space_cap,
                                )
                            );
                        }
                    }
                }
                #[cfg(feature = "calcfunc")]
                CalcCall(selector) => {
                    let owner = curr!()
                        .bindings
                        .code_owner
                        .clone()
                        .ok_or_else(|| ItrErr::code(ItrErrCode::CallInvalid))?;
                    let calcfn = r.resolve_local_calcfn(host, &owner, selector)?;
                    let input = {
                        let frame = curr_mut!();
                        let param = frame.pop_value()?;
                        param.extract_call_data(&frame.heap)?
                    };
                    let gas_limit = r.calc_resource_gas_limit(host)?;
                    let (gas_used, output) =
                        host.calc_call(&owner, selector, calcfn.as_ref(), input, gas_limit)?;
                    if gas_used > gas_limit {
                        return itr_err_fmt!(
                            OutOfGas,
                            "calc resource gas {} exceeds limit {}",
                            gas_used,
                            gas_limit
                        );
                    }
                    r.settle_calc_resource_gas(host, gas_used)?;
                    curr_mut!().push_value(Value::Bytes(output).valid(&r.warm.space_cap)?)?;
                    if curr!().pc == curr!().codes.len() {
                        let retv = curr_mut!().pop_value()?;
                        settle_return!(retv);
                    }
                    continue;
                }

                Abort | Throw | Finish | Return => {
                    let mut retv = Value::Nil;
                    if matches!(exit, Return | Throw) {
                        retv = curr_mut!().pop_value()?;
                    }
                    if matches!(exit, Abort | Throw) {
                        return itr_err_fmt!(ThrowAbort, "VM return failed: {}", retv);
                    }
                    settle_return!(retv);
                }
            }
        }
    }
}

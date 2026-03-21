impl CallFrame {
    pub fn start_call<H: VmHost + ?Sized>(
        &mut self,
        r: &mut Resoure,
        host: &mut H,
        exec: ExecCtx,
        code: &FnObj,
        bindings: FrameBindings,
        param: Option<Value>,
    ) -> VmrtRes<Value> {
        use CallExit::*;
        macro_rules! curr { () => { self.frames.last().unwrap() }; }
        macro_rules! curr_mut { () => { self.frames.last_mut().unwrap() }; }

        let height = host.height();

        exec.ensure_call_depth(&r.space_cap)?;
        let mut root = self.increase(r)?;
        root.prepare(exec, bindings, code, height, param, &r.space_cap)?;
        self.push(root);

        loop {
            let exit = curr_mut!().execute(r, host)?;
            match exit {
                Call(spec) => {
                    let curr_exec = curr!().exec;
                    let curr_bindings = curr!().bindings.clone();
                    let next_effect = spec.callee_effect(curr_exec.effect);
                    let next_exec = curr_exec.enter_call(next_effect, &r.space_cap)?;
                    // Validate local argv boundary before resolving/loading any callee so
                    // malformed input cannot warm caches via either Invoke or Splice.
                    curr_mut!().oprnds.peek()?.check_func_argv()?;
                    curr_mut!().oprnds.peek()?.check_container_cap(&r.space_cap)?;
                    let plan = r.plan_user_call(host, &spec, &curr_bindings)?;

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
                                param,
                                &r.space_cap,
                            )?;
                            continue;
                        }
                        CallSpec::Invoke { .. } => {
                            let param = curr_mut!().pop_value()?;
                            let mut next = self.increase(r)?;
                            next.prepare_invoke_unchecked_shape(
                                next_exec,
                                plan.next_bindings,
                                plan.fnobj.as_ref(),
                                height,
                                param,
                                &r.space_cap,
                            )?;
                            self.push(next);
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
                    curr_mut!().push_value(Value::Bytes(output).valid(&r.space_cap)?)?;
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
                    curr!().check_output_type(&mut retv, &r.space_cap)?;
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
                        let tail = self.pop().unwrap();
                        tail.check_output_type(&mut retv, &r.space_cap)?;
                        tail.reclaim(r);
                    }
                }
            }
        }
    }
}

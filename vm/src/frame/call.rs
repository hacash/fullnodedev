impl CallFrame {
    pub fn start_call(
        &mut self,
        r: &mut Resoure,
        env: &mut ExecEnv,
        exec: ExecCtx,
        code: &FnObj,
        bindings: FrameBindings,
        param: Option<Value>,
    ) -> VmrtRes<Value> {
        use CallExit::*;
        macro_rules! curr { () => { self.frames.last().unwrap() }; }
        macro_rules! curr_mut { () => { self.frames.last_mut().unwrap() }; }

        let height = env.ctx.env().block.height;

        exec.ensure_call_depth(&r.space_cap)?;
        let mut root = self.increase(r)?;
        root.prepare(exec, bindings, code, height, param)?;
        self.push(root);

        loop {
            let exit = curr_mut!().execute(r, env)?;
            match exit {
                Call(spec) => {
                    let curr_exec = curr!().exec;
                    let curr_bindings = curr!().bindings.clone();
                    let next_effect = spec.callee_effect(curr_exec.effect);
                    let next_exec = curr_exec.enter_call(next_effect, &r.space_cap)?;
                    if matches!(spec, CallSpec::Invoke { .. }) {
                        curr_mut!().oprnds.peek()?.canbe_func_argv()?;
                    }
                    let plan = r.plan_user_call(env.ctx, &mut *env.gas, &spec, &curr_bindings)?;

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
                            )?;
                            self.push(next);
                        }
                    }
                }

                Abort | Throw | Finish | Return => {
                    let mut retv = Value::Nil;
                    if matches!(exit, Return | Throw) {
                        retv = curr_mut!().pop_value()?;
                    }
                    if matches!(exit, Abort | Throw) {
                        return itr_err_fmt!(ThrowAbort, "VM return failed: {}", retv);
                    }
                    curr!().check_return_value(&mut retv)?;
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
                        tail.check_return_value(&mut retv)?;
                        tail.reclaim(r);
                    }
                }
            }
        }
    }
}

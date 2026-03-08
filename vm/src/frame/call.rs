impl CallFrame {
    fn prepare_frame(
        frame: &mut Frame,
        exec: ExecCtx,
        bindings: FrameBindings,
        fnobj: &FnObj,
        height: u64,
        param: Option<Value>,
    ) -> VmrtErr {
        frame.prepare(exec, bindings, fnobj, height, param)
    }

    pub fn start_call(
        &mut self,
        r: &mut Resoure,
        env: &mut ExecEnv,
        exec: ExecCtx,
        code: &FnObj,
        bindings: FrameBindings,
        param: Option<Value>,
    ) -> VmrtRes<Value> {
        macro_rules! curr {
            () => {
                self.frames.last_mut().unwrap()
            };
        }
        macro_rules! curr_ref {
            () => {
                self.frames.last().unwrap()
            };
        }

        use CallExit::*;

        let height = env.ctx.env().block.height;

        exec.ensure_call_depth(&r.space_cap)?;
        let mut root = self.increase(r)?;
        Self::prepare_frame(&mut root, exec, bindings, code, height, param)?;
        self.push(root);

        loop {
            let exit = curr!().execute(r, env)?;
            match exit {
                Call(spec) => {
                    let next_exec = curr_ref!()
                        .exec
                        .enter_call(spec.next_effect(curr_ref!().exec.effect), &r.space_cap)?;
                    let invokes = matches!(spec, CallSpec::Invoke { .. });
                    if invokes {
                        curr!().oprnds.peek()?.canbe_func_argv()?;
                    }
                    let curr_bindings = curr_ref!().bindings.clone();
                    let plan = r.plan_user_call(
                        env.ctx,
                        &mut *env.gas,
                        CallPlanReq {
                            call: &spec,
                            bindings: &curr_bindings,
                        },
                    )?;

                    if !invokes {
                        curr!().prepare_splice(
                            next_exec,
                            plan.next_bindings,
                            plan.fnobj.as_ref(),
                            height,
                        )?;
                        continue;
                    }

                    let param = Some(curr!().pop_value()?);
                    let next = self.increase(r)?;
                    self.push(next);
                    Self::prepare_frame(
                        curr!(),
                        next_exec,
                        plan.next_bindings,
                        plan.fnobj.as_ref(),
                        height,
                        param,
                    )?;
                }

                Abort | Throw | Finish | Return => {
                    let mut retv = Value::Nil;
                    if matches!(exit, Return | Throw) {
                        retv = curr!().pop_value()?;
                    }
                    if matches!(exit, Abort | Throw) {
                        return itr_err_fmt!(ThrowAbort, "VM return error: {}", retv);
                    }
                    curr_ref!().check_return_value(&mut retv)?;
                    self.pop().unwrap().reclaim(r);

                    loop {
                        let is_tail = match self.frames.last() {
                            Some(f) => f.pc == f.codes.len(),
                            None => return Ok(retv),
                        };
                        if !is_tail {
                            curr!().push_value(retv)?;
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

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
        use CallExit::*;

        let height = env.ctx.env().block.height;

        exec.ensure_call_depth(&r.space_cap)?;
        let mut root = self.increase(r)?;
        Self::prepare_frame(&mut root, exec, bindings, code, height, param)?;
        self.push(root);

        loop {
            let exit = self.frames.last_mut().unwrap().execute(r, env)?;
            match exit {
                Call(spec) => {
                    let (curr_exec, curr_bindings) = {
                        let curr = self.frames.last().unwrap();
                        (curr.exec, curr.bindings.clone())
                    };
                    let next_effect = spec.callee_effect(curr_exec.effect);
                    let next_exec = curr_exec.enter_call(next_effect, &r.space_cap)?;
                    if matches!(spec, CallSpec::Invoke { .. }) {
                        self.frames
                            .last_mut()
                            .unwrap()
                            .oprnds
                            .peek()?
                            .canbe_func_argv()?;
                    }
                    let plan = r.plan_user_call(env.ctx, &mut *env.gas, &spec, &curr_bindings)?;

                    match spec {
                        CallSpec::Splice { .. } => {
                            let splice_argv = match plan.fnobj.agvty.as_ref() {
                                Some(types) if types.param_count() > 0 => {
                                    self.frames.last().unwrap().call_argv.clone()
                                }
                                _ => Value::Nil,
                            };
                            let curr = self.frames.last_mut().unwrap();
                            curr.push_value(splice_argv)?;
                            curr.prepare_splice(
                                next_exec,
                                plan.next_bindings,
                                plan.fnobj.as_ref(),
                                height,
                            )?;
                            continue;
                        }
                        CallSpec::Invoke { .. } => {
                            let param = Some(self.frames.last_mut().unwrap().pop_value()?);
                            let mut next = self.increase(r)?;
                            Self::prepare_frame(
                                &mut next,
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
                        retv = self.frames.last_mut().unwrap().pop_value()?;
                    }
                    if matches!(exit, Abort | Throw) {
                        return itr_err_fmt!(ThrowAbort, "VM return error: {}", retv);
                    }
                    self.frames.last().unwrap().check_return_value(&mut retv)?;
                    self.pop().unwrap().reclaim(r);

                    loop {
                        let is_tail = match self.frames.last() {
                            Some(f) => f.pc == f.codes.len(),
                            None => return Ok(retv),
                        };
                        if !is_tail {
                            self.frames.last_mut().unwrap().push_value(retv)?;
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

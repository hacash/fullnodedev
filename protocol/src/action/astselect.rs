action_define! { AstSelect, 25,
    ActLv::Ast, // level
    // burn 90 fee, check any sub child action
    self.actions.as_list().iter().any(|a|a.burn_90()),
    self.collect_req_sign(),
    {
        exe_min: Uint1
        exe_max: Uint1
        actions: DynListActionW1
    },
    (self, format!("Execute select {} to {} in {} actions",
        *self.exe_min, *self.exe_max, self.actions.length())),
    (self, ctx, gas {
        gas = 0; // control-flow node: all gas consumed via ctx
        let mut guard = ast_enter(ctx)?;
        let ctx = guard.ctx();
        let slt_min = *self.exe_min as usize;
        let slt_max = *self.exe_max as usize;
        let slt_num = self.actions.length();
        // NOTE: `slt_min == 0` is intentionally allowed.
        // Empty-select semantics (e.g. `0/0` or `0/N`) are part of the AST design,
        // and are used by higher-level control flow/tests as a legal no-op success path.
        validate_ast_select(slt_min, slt_max, slt_num)?;
        let node = AstNodeTxn::begin(ctx)?;
        let res = self.execute_select_core(ctx, slt_min, slt_max);
        node.finish(ctx, res)
    })
}

impl AstSelect {
    pub fn nop() -> Self {
        Self::new()
    }

    pub fn create_list(acts: Vec<Box<dyn Action>>) -> Self {
        let num = acts.len();
        assert!(num < u8::MAX as usize);
        let num = num as u8;
        Self {
            exe_min: Uint1::from(num),
            exe_max: Uint1::from(num),
            actions: DynListActionW1::from_list(acts).unwrap(),
            ..Self::new()
        }
    }

    pub fn create_by(min: u8, max: u8, acts: Vec<Box<dyn Action>>) -> Self {
        Self {
            exe_min: Uint1::from(min),
            exe_max: Uint1::from(max),
            actions: DynListActionW1::from_list(acts).unwrap(),
            ..Self::new()
        }
    }

    pub(crate) fn collect_req_sign(&self) -> Vec<AddrOrPtr> {
        let mut req = vec![];
        for act in self.actions.as_list() {
            if let Some(sub) = AstSelect::downcast(act) {
                req.extend(sub.collect_req_sign());
                continue;
            }
            if let Some(sub) = AstIf::downcast(act) {
                req.extend(sub.collect_req_sign());
                continue;
            }
            req.extend(act.req_sign());
        }
        req
    }

    fn execute_select_core(
        &self,
        ctx: &mut dyn Context,
        slt_min: usize,
        slt_max: usize,
    ) -> Ret<Vec<u8>> {
        let mut ok = 0usize;
        let mut last_ok_ret: Option<Vec<u8>> = None;
        for act in self.actions.as_list() {
            if ok >= slt_max {
                break; // reached max success limit
            }
            if let Some(ret) = ast_unwind_continue(ast_try_item!(ctx, act.execute(ctx), act.burn_90()))? {
                last_ok_ret = Some(ret);
                ok += 1;
            }
        }
        if ok < slt_min {
            return xerr_rf!("action ast select must succeed at least {} but only {}", slt_min, ok);
        }
        Ok(last_ok_ret.unwrap_or_default())
    }
}

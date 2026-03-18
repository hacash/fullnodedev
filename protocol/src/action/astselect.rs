action_define! { AstSelect, 25,
    ActScope::AST, 3, false,
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
        let mut exec_from = enter_exec_from(ctx, ExecFrom::Ast);
        let ctx = exec_from.ctx();
        let slt_min = *self.exe_min as usize;
        let slt_max = *self.exe_max as usize;
        let slt_num = self.actions.length();
        // NOTE: `slt_min == 0` is intentionally allowed.
        // Empty-select semantics (e.g. `0/0` or `0/N`) are part of the AST design,
        // and are used by higher-level control flow/tests as a legal no-op success path.
        validate_ast_select(slt_min, slt_max, slt_num)?;
        // Failed AST nodes do not restore prior successful siblings here; upper execution layers own whole-tx rollback.
        self.execute_select_core(ctx, slt_min, slt_max)
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
        fn collect_from(req: &mut Vec<AddrOrPtr>, act: &dyn Action) {
            if let Some(sub) = act.as_any().downcast_ref::<AstSelect>() {
                for child in sub.actions.as_list() {
                    collect_from(req, child.as_ref());
                }
                return;
            }
            if let Some(sub) = act.as_any().downcast_ref::<AstIf>() {
                for child in sub.cond.actions.as_list() {
                    collect_from(req, child.as_ref());
                }
                for child in sub.br_if.actions.as_list() {
                    collect_from(req, child.as_ref());
                }
                for child in sub.br_else.actions.as_list() {
                    collect_from(req, child.as_ref());
                }
                return;
            }
            req.extend(act.req_sign());
        }

        let mut req = vec![];
        for act in self.actions.as_list() {
            collect_from(&mut req, act.as_ref());
        }
        req
    }

    fn execute_select_core(
        &self,
        ctx: &mut dyn Context,
        slt_min: usize,
        slt_max: usize,
    ) -> XRet<Vec<u8>> {
        let mut ok = 0usize;
        let mut last_ok_ret: Option<Vec<u8>> = None;
        for act in self.actions.as_list() {
            if ok >= slt_max {
                break; // reached max success limit
            }
            match ast_exec_item(ctx, act.extra9(), |ctx| act.execute(ctx)) {
                Ok(ret) => {
                    last_ok_ret = Some(ret);
                    ok += 1;
                }
                Err(XError::Revert(_)) => {}
                Err(e) => return Err(e),
            }
        }
        if ok < slt_min {
            return xerr_rf!(
                "action ast select must succeed at least {} but only {}",
                slt_min,
                ok
            );
        }
        Ok(last_ok_ret.unwrap_or_default())
    }
}

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
        #[cfg(not(feature = "ast"))]
        if true {
            return errf!("ast select not open")
        }
        let mut guard = ast_enter(ctx)?;
        let ctx = guard.ctx();
        let slt_min = *self.exe_min as usize;
        let slt_max = *self.exe_max as usize;
        let slt_num = self.actions.length();
        validate_ast_select(slt_min, slt_max, slt_num)?;
        // execute
        let mut ok = 0;
        let mut rv = vec![];
        ast_with_whole_snapshot(ctx, |ctx| {
            for act in self.actions.as_list() {
                if ok >= slt_max {
                    break // ok full
                }
                match ast_run_item_snapshot(ctx, &mut gas, |ctx| act.execute(ctx))? {
                    Ok((_child_gas, r)) => {
                        rv = r;
                        ok += 1;
                    }
                    Err(e) => {
                        if e.is_interrupt() {
                            return ast_rethrow(e)
                        }
                    }
                }
            }
            // check at least
            if ok < slt_min {
                return erruf!("action ast select must succeed at least {} but only {}", slt_min, ok)
            }
            Ok(rv)
        })
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
}

#[allow(dead_code)]
impl Syntax {
    pub fn with_params(mut self, params: Vec<(String, ValueTy)>) -> Self {
        self.injected.ext_params = Some(params);
        self
    }

    pub fn with_libs(mut self, libs: Vec<(String, u8, Option<FieldAddress>)>) -> Self {
        self.injected.ext_libs = Some(libs);
        self
    }

    /// Inject external constants into the syntax context
    pub fn with_consts(mut self, consts: Vec<(String, Box<dyn IRNode>)>) -> Self {
        self.injected.ext_consts = Some(consts);
        self
    }

    pub fn with_ircode(mut self, is_ircode: bool) -> Self {
        self.mode.is_ircode = is_ircode;
        self
    }

    pub fn parse(mut self) -> Ret<(IRNodeArray, SourceMap)> {
        use Bytecode::*;
        // reserve head for ALLOC
        self.emit.irnode.push(push_empty());

        // External Libs
        if let Some(libs) = self.injected.ext_libs.take() {
            for (name, idx, addr) in libs {
                self.bind_lib(name, idx, addr)?;
            }
        }
        // External Consts
        if let Some(consts) = self.injected.ext_consts.take() {
            for (name, node) in consts {
                if self.symbols.contains_key(&name) {
                    return errf!("symbol '{}' already defined", name);
                }
                self.symbols.insert(name.clone(), SymbolEntry::Const(node));
            }
        }
        // External Params
        if let Some(params) = self.injected.ext_params.take() {
            let mut param_names = Vec::new();
            for (i, (name, _ty)) in params.iter().enumerate() {
                if i > u8::MAX as usize {
                    return errf!("param index {} overflow", i);
                }
                let idx = i as u8;
                self.bind_local(name.clone(), idx, SlotKind::Var)?;
                param_names.push(name.clone());
            }
            if !param_names.is_empty() {
                self.emit.source_map.register_param_names(param_names)?;
            }
            self.emit
                .source_map
                .register_param_prelude_count(params.len() as u8)?;
            self.emit.irnode
                .push(Self::build_param_prelude(params.len(), true)?);
        }

        let mut terminated = false;
        loop {
            if self.try_skip_redundant_terminal_end(terminated) {
                continue;
            }
            let Some(item) = self.item_may()? else {
                break;
            };
            if terminated {
                return errf!("unreachable code after terminal statement");
            }
            terminated = Self::is_strong_terminator(&*item);
            if let Some(..) = item.as_any().downcast_ref::<IRNodeEmpty>() {
            } else {
                self.emit.irnode.push(item);
            };
        }
        let subs = &mut self.emit.irnode.subs;
        if self.local_alloc > 0 {
            let allocs = Box::new(IRNodeParam1 {
                hrtv: false,
                inst: ALLOC,
                para: self.local_alloc,
                text: s!(""),
            });
            let mut exist = false;
            if subs.len() > 1 && subs[1].bytecode() == ALLOC as u8 {
                exist = true;
            }
            if exist {
                subs[1] = allocs;
            } else {
                subs[0] = allocs;
            }
        }
        let block = self.emit.irnode;
        let source_map = self.emit.source_map;
        Ok((block, source_map))
    }
}

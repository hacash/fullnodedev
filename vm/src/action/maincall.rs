
/* default to spend 32 gas each call */
action_define!{ContractMainCall, 100, 
    ActLv::Ast, // level
    false, [],
    {
        marks: Fixed3
        ctype: Uint1
        codes: BytesW2
    },
    (self, format!("Run main codes with type {}", *self.ctype)),
    (self, ctx, _gas {
        if self.marks.not_zero() {
            return errf!("marks bytes format error")
        }
        // check codes
        let hei = ctx.env().block.height;
        let cap = SpaceCap::new(hei);
        let cty = CodeType::parse(self.ctype.to_uint())?;
        convert_and_check(&cap, cty, &self.codes, hei)?;
        setup_vm_run(
            ctx,
            ExecMode::Main as u8,
            *self.ctype,
            self.codes.as_vec().clone().into(),
            Value::Nil,
        )?;
        Ok(vec![])
    })
}


impl ContractMainCall {
    pub fn from_bytecode(codes: Vec<u8>) -> Ret<Self> {
        let mut s = Self::new();
        s.ctype = Uint1::from(CodeType::Bytecode as u8);
        s.codes = BytesW2::from(codes)?;
        Ok(s)
    }
}


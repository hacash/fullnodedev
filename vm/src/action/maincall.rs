

action_define!{ ContractMainCall, 44, 
    ActScope::AST, false, [],
    {
        marks: Fixed3
        codeconf: Uint1
        codes: BytesW2
    },
    (self, format!("Run main codes with conf {}", *self.codeconf)),
    (self, ctx, _gas {
        if self.marks.not_zero() {
            return errf!("marks bytes format invalid")
        }
        // check codes
        let hei = ctx.env().block.height;
        let cap = SpaceCap::new(hei);
        let codeconf = CodeConf::parse(self.codeconf.to_uint())?;
        convert_and_check(&cap, codeconf.code_type(), &self.codes, hei)?;
        let _ = setup_vm_run_main(ctx, codeconf.raw(), self.codes.as_vec().clone().into())?;
        Ok(vec![])
    })
}


impl ContractMainCall {
    pub fn from_bytecode(codes: Vec<u8>) -> Ret<Self> {
        let mut s = Self::new();
        s.codeconf = Uint1::from(CodeConf::from_type(CodeType::Bytecode).raw());
        s.codes = BytesW2::from(codes)?;
        Ok(s)
    }
}

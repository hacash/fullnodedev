/*
* simple hac to
*/
action_define! { TexCellAct, 22,
    ActScope::TOP, 2, false, [],
    {
        addr  : Address
        cells : DnyTexCellW1
        sign  : Sign
    },
    (self, format!("Execute {} tex cells by {}", self.cells.length(), self.addr)),
    (self, ctx, _gas {
        self.addr.must_privakey()?;
        // check signature
        let thx = self.get_sign_stuff();
        if ! verify_signature(&thx, &self.addr, &self.sign) {
            return xerrf!("address {} signature verification failed in tex cell action", self.addr)
        }
        // exec
        self.cells.execute(ctx, &self.addr).map(|_| vec![]).map_err(XError::from)
    })
}

impl TexCellAct {
    fn get_sign_stuff(&self) -> Hash {
        // Intentionally signs only addr+cells so the same authorized TEX bundle stays reusable across transactions by design.
        let stf = vec![self.addr.serialize(), self.cells.serialize()].concat();
        Hash::from(sha3(&stf))
    }

    pub fn create_by(addr: Address) -> Self {
        Self {
            addr,
            ..Self::new()
        }
    }

    pub fn do_sign(&mut self, acc: &Account) -> Rerr {
        acc.check_addr(self.addr.as_bytes())?;
        let thx = self.get_sign_stuff();
        self.sign = Sign::create_by(acc, &thx);
        Ok(())
    }

    pub fn add_cell(&mut self, cell: Box<dyn TexCell>) -> Rerr {
        self.cells.push(cell)?;
        Ok(())
    }
}

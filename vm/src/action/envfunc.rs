


/**************** env *****************/



action_define!{ EnvHeight, 0x0701, 
    ActLv::AnyInCall, false, [], {},
    (self, "Syscall: Get block height".to_owned()),
    (self, ctx, _gas {
        Ok(ctx.env().block.height.to_be_bytes().to_vec())
    })
}


action_define!{ EnvMainAddr, 0x0702, 
    ActLv::AnyInCall, false, [], {},
    (self, "Syscall: Get main address".to_owned()),
    (self, ctx, _gas {
        Ok(ctx.env().tx.main.to_vec())
    })
}


action_define!{ EnvCoinbaseAddr, 0x0703, 
    ActLv::AnyInCall, false, [], {},
    (self, "Syscall: Get coinbase address".to_owned()),
    (self, ctx, _gas {
        let cbadr = ctx.env().block.coinbase.clone();
        Ok(cbadr.to_vec())
    })
}



/**************** view *****************/



action_define!{ ViewCheckSign, 0x0601, 
    ActLv::AnyInCall, false, [], {
        addr: Address
    },
    (self, format!("Syscall: Check signature for {}", self.addr)),
    (self, ctx, _gas {
        match ctx.check_sign(&self.addr) {
            Ok(..) => Ok(vec![1]), // yes
            _ => Ok(vec![0]) // no
        }
    })
}


action_define!{ ViewBalance, 0x0602, 
    ActLv::AnyInCall, false, [], {
        addr: Address
    },
    (self, format!("Syscall: Get balance for {}", self.addr)),
    (self, ctx, _gas {
        let bls = CoreState::wrap(ctx.state()).balance(&self.addr).unwrap_or_default();
        let mut res = Vec::with_capacity(4+8+8);
        res.append(&mut Vec::from((bls.diamond.uint() as u32).to_be_bytes()));
        res.append(&mut Vec::from(bls.satoshi.uint().to_be_bytes()));
        res.append(&mut bls.hacash.serialize());
        Ok(res)
    })
}


action_define!{ ViewDiamondInscNum, 0x0603, 
    ActLv::AnyInCall, false, [], {
        diamond: DiamondName
    },
    (self, format!("Syscall: Get diamond inscription number for <{}>", self.diamond.to_readable())),
    (self, ctx, _gas {
        let Some(diaobj) = CoreStateRead::wrap(ctx.state()).diamond(&self.diamond) else {
            return errf!("diamond {} not find", self.diamond.to_readable())
        };
        let num = diaobj.inscripts.length();
        if num > u8::MAX as usize {
            return errf!("diamond {} inscripts number error", self.diamond.to_readable())
        }
        // ok
        Ok(vec![num as u8])
    })
}


action_define!{ ViewDiamondInscGet, 0x0604, 
    ActLv::AnyInCall, false, [], {
        diamond: DiamondName
        inscidx: Uint1
    },
    (self, format!("Syscall: Get diamond inscription data for <{}>", self.diamond.to_readable())),
    (self, ctx, _gas {
        let Some(diaobj) = CoreStateRead::wrap(ctx.state()).diamond(&self.diamond) else {
            return errf!("diamond {} not find", self.diamond.to_readable())
        };
        let num = diaobj.inscripts.length();
        let idx = self.inscidx.uint() as usize ;
        if idx >= num {
            return errf!("diamond {} inscripts number overflow", self.diamond.to_readable())
        }
        let insc = &diaobj.inscripts.as_list()[idx];
        // ok
        Ok(insc.to_vec())
    })
}



/*********************************/


#[allow(dead_code)]
#[derive(Default, Clone)]
pub struct TexLedger {
    pub zhu: i64,
    pub sat: i64,
    pub dia: i32,
    pub diamonds: DiamondNameListMax60000,
    // By design TEX diamond receipts claim only a quantity from the shared pool, and exact names are assigned later by settlement order.
    pub diatrs:   Vec<(Address, usize)>,
    pub assets:   HashMap<Fold64, i128>,
    pub asset_checked: HashSet<Fold64>,
}


impl TexLedger {

    pub fn record_diamond_pay(&mut self, dias: DiamondNameListMax200) -> Rerr {
        let Some(newdia) = self.dia.checked_add(dias.length() as i32) else {
            return errf!("cell state diamond record overflow")
        };
        let mut diamonds = self.diamonds.clone();
        diamonds.checked_append(dias.into_list())?;
        self.diamonds = diamonds;
        self.dia = newdia;
        Ok(())
    }
    
    pub fn record_diamond_get(&mut self, addr: &Address, num: usize) -> Rerr {
        if num > 200 {
            return errf!("Tex state diamond trs num cannot exceed 200")
        }
        let Some(diares) = self.dia.checked_sub(num as i32) else {
            return errf!("cell state diamond overflow")
        };
        self.dia = diares;
        self.diatrs.push((addr.clone(), num));
        Ok(())
    }

}

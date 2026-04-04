
combi_struct!{ VmLog,
    // height: BlockHeight
    addr: Address
    topic0:  Value
    topic1:  Value
    topic2:  Value
    topic3:  Value
    data:    Value
}

impl VmLog {

    pub fn render(&self) -> String {
        format!(r#""address":"{}","topic0":"{}","topic1":"{}","topic2":"{}","topic3":"{}","data":"{}""#,
            self.addr,
            self.topic0.raw().to_hex(),
            self.topic1.raw().to_hex(),
            self.topic2.raw().to_hex(),
            self.topic3.raw().to_hex(),
            self.data.raw().to_hex(),
        )
    }

    pub fn new(addr: Address, tds: Vec<Value>) -> VmrtRes<Self> {
        let tl = tds.len();
        if tl < 2 {
            return itr_err_fmt!(LogError, "argv num must be at least 2")
        }
        if tl > 5 {
            return itr_err_fmt!(LogError, "argv num must be at most 5")
        }
        // check can store
        for a in &tds {
            a.check_scalar()?;
        }
        let mut log = Self {
            addr,
            topic0: tds[0].clone(),
            topic1: Value::nil(),
            topic2: Value::nil(),
            topic3: Value::nil(),
            data: tds[tl - 1].clone(),
        };
        match tl {
            2 => {},
            3 => { log.topic1 = tds[1].clone(); }
            4 => { log.topic1 = tds[1].clone(); log.topic2 = tds[2].clone(); }
            5 => { log.topic1 = tds[1].clone(); log.topic2 = tds[2].clone(); log.topic3 = tds[3].clone(); }
            _ => unreachable!()
        };
        // finish
        Ok(log)
    }


}

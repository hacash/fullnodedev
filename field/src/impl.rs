


impl Parse for Vec<u8> {
    fn parse(&mut self, s: &[u8]) -> Ret<usize> {
        let sl = self.len();
        if sl > s.len() {
            return errf!("buffer too short")
        } 
        Ok(sl)
    }
}


impl Serialize for Vec<u8> {
    fn serialize(&self) -> Vec<u8> {
        self.clone()
    }
    fn size(&self) -> usize { 
        self.len()
    }
}


impl Field for Vec<u8> {
    fn new() -> Self {
        Vec::new()
    }
}

impl ToJSON for Vec<u8> {
    fn to_json_fmt(&self, _fmt: &JSONFormater) -> String {
        format!("\"0x{}\"", hex::encode(self))
    }
}

impl FromJSON for Vec<u8> {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let b = json_decode_binary(json)?;
        *self = b;
        Ok(())
    }
}




#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Empty {}


impl Serialize for Empty {

    fn serialize(&self) -> Vec<u8> {
        Vec::new()
    }

    fn size(&self) -> usize {
        0
    }

}


impl Parse for Empty {

    fn parse(&mut self, _: &[u8]) -> Ret<usize> {
        Ok(0)    
    }

}

impl Field for Empty {
    fn new() -> Self {
        Self{}
    }
}

impl ToJSON for Empty {
    fn to_json_fmt(&self, _: &JSONFormater) -> String {
        "{}".to_string()
    }
}

impl FromJSON for Empty {
    fn from_json(&mut self, _: &str) -> Ret<()> {
        Ok(())
    }
}



///////////////////////



#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct VecWrap {
    pub data: Vec<u8>,
}


impl Serialize for VecWrap {

    fn serialize(&self) -> Vec<u8> {
        self.data.clone()
    }

    fn size(&self) -> usize {
        self.data.len()
    }

}


impl Parse for VecWrap {

    fn parse(&mut self, s: &[u8]) -> Ret<usize> {
        self.data.parse(s)
    }

}



impl Field for VecWrap {
    fn new() -> Self {
        Self {
            data: Vec::new()
        }
    }
}

impl ToJSON for VecWrap {
    fn to_json_fmt(&self, _: &JSONFormater) -> String {
        format!("\"0x{}\"", hex::encode(&self.data))
    }
}

impl FromJSON for VecWrap {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let b = json_decode_binary(json)?;
        self.data = b;
        Ok(())
    }
}

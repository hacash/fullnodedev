#[repr(transparent)]
#[derive(Default, Debug, Hash, Copy, Clone, PartialEq, Eq)]
pub struct Bool(Fixed1);

impl Deref for Bool {
    type Target = Fixed1;
    fn deref(&self) -> &Fixed1 {
        &self.0
    }
}

impl AsRef<[u8]> for Bool {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Parse for Bool {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        let sk = self.0.parse(buf)?;
        if self.0[0] > 1 {
            return errf!("Bool value {} invalid, expect 0 or 1", self.0[0]);
        }
        Ok(sk)
    }
}

impl Serialize for Bool {
    fn serialize_to(&self, out: &mut Vec<u8>) {
        self.0.serialize_to(out);
    }
    fn size(&self) -> usize {
        self.0.size()
    }
}

impl_field_only_new!{Bool}

impl ToJSON for Bool {
    fn to_json_fmt(&self, _: &JSONFormater) -> String {
        maybe!(self.check(), "1".to_string(), "0".to_string())
    }
}

impl FromJSON for Bool {
    fn from_json(&mut self, json: &str) -> Ret<()> {
        let s = json_expect_unquoted(json)?.trim();
        self.0 = match s {
            "1" | "true" => Fixed1::from([1u8]),
            "0" | "false" => Fixed1::from([0u8]),
            _ => return errf!("cannot parse Bool from: {}", s),
        };
        Ok(())
    }
}

impl Bool {
    pub fn check(&self) -> bool {
        self.0[0] != 0
    }

    pub fn new(v: bool) -> Self
    where
        Self: Sized,
    {
        Self(Fixed1::from([maybe!(v, 1, 0)]))
    }
}

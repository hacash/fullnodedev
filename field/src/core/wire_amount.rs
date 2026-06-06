
/// Wire-preserving amount for fields that must keep historical non-canonical
/// semantic-zero encodings in tx/action hash while executing with canonical [`Amount`].
#[derive(Clone, PartialEq, Eq)]
pub struct WireAmount {
    amount: Amount,
    wire: Vec<u8>,
}

impl Default for WireAmount {
    fn default() -> Self {
        Self::from_amount(Amount::zero())
    }
}

impl std::fmt::Display for WireAmount {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}", self.amount)
    }
}

impl Debug for WireAmount {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "WireAmount({:?}, wire={:?})", self.amount, self.wire)
    }
}

impl WireAmount {
    pub fn from_amount(amount: Amount) -> Self {
        let wire = amount.serialize();
        Self { amount, wire }
    }

    pub fn amount(&self) -> &Amount {
        &self.amount
    }

    pub fn wire(&self) -> &[u8] {
        &self.wire
    }

    pub fn is_canonical_wire(&self) -> bool {
        self.wire == self.amount.serialize()
    }

    pub fn require_canonical_wire(&self) -> Ret<()> {
        if self.is_canonical_wire() {
            Ok(())
        } else {
            errf!("amount wire encoding is not canonical")
        }
    }
}

impl Deref for WireAmount {
    type Target = Amount;
    fn deref(&self) -> &Amount {
        &self.amount
    }
}

impl From<Amount> for WireAmount {
    fn from(amount: Amount) -> Self {
        Self::from_amount(amount)
    }
}

fn try_parse_non_canonical_semantic_zero(buf: &[u8]) -> Ret<(Amount, usize)> {
    if buf.len() < 2 {
        return errf!("buffer too short");
    }
    let unit = buf[0];
    let dist_raw = buf[1];
    if dist_raw == i8::MIN as u8 {
        return errf!("dist cannot be {}", i8::MIN);
    }
    let dist = dist_raw as i8;
    let btlen = dist.unsigned_abs() as usize;
    if buf.len() < 2 + btlen {
        return errf!("buffer too short");
    }
    let byte = &buf[2..2 + btlen];
    if btlen != byte.len() {
        return errf!("dist and byte len mismatch");
    }
    if dist != 0 && byte.is_empty() {
        return errf!("dist and byte zero mismatch");
    }
    let rbtl = byte.len();
    if rbtl > 1 && bytes_is_zero(byte) {
        return errf!("multi-byte amount cannot be all zero");
    }
    if rbtl > 1 && byte[0] == 0 {
        return errf!("amount leading zero byte is not canonical");
    }
    if !bytes_is_zero(byte) {
        return errf!("wire amount fallback only accepts semantic zero");
    }
    if unit == 0 && dist == 0 && byte.is_empty() {
        return errf!("canonical zero must use Amount parse");
    }
    Ok((Amount::zero(), 2 + btlen))
}

impl Parse for WireAmount {
    fn parse(&mut self, buf: &[u8]) -> Ret<usize> {
        if let Ok((amt, n)) = Amount::create(buf) {
            self.amount = amt;
            self.wire = buf[..n].to_vec();
            return Ok(n);
        }
        let (amt, n) = try_parse_non_canonical_semantic_zero(buf)?;
        self.amount = amt;
        self.wire = buf[..n].to_vec();
        Ok(n)
    }
}

impl Serialize for WireAmount {
    fn serialize_to(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.wire);
    }
    fn size(&self) -> usize {
        self.wire.len()
    }
}

impl_field_only_new! { WireAmount }

impl ToJSON for WireAmount {
    fn to_json_fmt(&self, fmt: &JSONFormater) -> String {
        self.amount.to_json_fmt(fmt)
    }
}

impl FromJSON for WireAmount {
    fn from_json(&mut self, json_str: &str) -> Ret<()> {
        let mut amt = Amount::default();
        amt.from_json(json_str)?;
        *self = Self::from_amount(amt);
        Ok(())
    }
}

#[cfg(test)]
mod wire_amount_tests {
    use super::*;

    #[test]
    fn canonical_zero_roundtrip() {
        let wa = WireAmount::from_amount(Amount::zero());
        assert_eq!(wa.wire(), &[0u8, 0]);
        let ser = wa.serialize();
        let mut parsed = WireAmount::default();
        parsed.parse(&ser).unwrap();
        assert_eq!(parsed.wire(), wa.wire());
        assert!(parsed.amount().is_zero());
    }

    #[test]
    fn non_canonical_semantic_zero_preserves_wire() {
        let bytes = vec![0u8, 1, 0];
        let mut parsed = WireAmount::default();
        parsed.parse(&bytes).unwrap();
        assert_eq!(parsed.wire(), bytes.as_slice());
        assert!(parsed.amount().is_zero());
        assert_eq!(parsed.serialize(), bytes);

        assert!(Amount::build(&bytes).is_err(), "strict Amount must reject [0,1,0]");
    }

    #[test]
    fn positive_amount_roundtrip() {
        let amt = Amount::mei(123);
        let wa = WireAmount::from_amount(amt.clone());
        let ser = wa.serialize();
        let mut parsed = WireAmount::default();
        parsed.parse(&ser).unwrap();
        assert!(parsed.amount().equal(&amt));
        assert_eq!(parsed.wire(), ser);
    }

    #[test]
    fn require_canonical_wire_rejects_non_canonical_semantic_zero() {
        let bytes = vec![0u8, 1, 0];
        let mut parsed = WireAmount::default();
        parsed.parse(&bytes).unwrap();
        assert!(!parsed.is_canonical_wire());
        assert!(parsed.require_canonical_wire().is_err());
    }
}



#[macro_export]
macro_rules! ctx_state{
    ($ctx:expr, $state:ident) => (
        let _s1_db = $ctx.engine.state();
        let _s1_db = _s1_db.as_ref();
        let $state = CoreStateRead::wrap(_s1_db.as_ref());
    )
}

#[macro_export]
macro_rules! ctx_mint_state{
    ($ctx:expr, $state:ident) => (
        let _s1_db = $ctx.engine.state();
        let _s1_db = _s1_db.as_ref();
        let $state = MintStateRead::wrap(_s1_db.as_ref());
    )
}

#[macro_export]
macro_rules! ctx_store{
    ($ctx:expr, $sto:ident) => (
        let $sto = $ctx.engine.store();
    )
}

#[macro_export]
macro_rules! ctx_mintcnf{
    ($ctx:expr) => (
        $ctx.engine.minter().config().downcast::<MintConf>().unwrap()
    )
}

#[derive(Clone, Debug)]
pub struct CoinKind {
    pub hacash: bool,
    pub satoshi: bool,
    pub diamond: bool,
    pub assets: Vec<u64>,
    pub assets_all: bool,
}
impl CoinKind {
    pub fn new(s: String) -> CoinKind {
        let sl = s.to_lowercase();
        match sl.as_str() {
            "all" | "hsda" => CoinKind {
                hacash: true,
                satoshi: true,
                diamond: true,
                assets: vec![],
                assets_all: true,
            },
            _ => CoinKind {
                hacash: sl.contains("h"),
                satoshi: sl.contains("s"),
                diamond: sl.contains("d"),
                assets: vec![],
                assets_all: sl.contains("a"),
            }
        }
    }

    pub fn parse(s: String) -> Ret<CoinKind> {
        let lower = s.to_lowercase();
        let compact: String = lower.chars().filter(|c| !c.is_whitespace()).collect();
        let mut kind_part = compact.as_str();
        let mut assets_part = "";
        if let Some(start) = compact.find('(') {
            let Some(end) = compact.rfind(')') else {
                return errf!("coinkind assets list format error")
            };
            if end <= start {
                return errf!("coinkind assets list format error")
            }
            assets_part = &compact[start + 1..end];
            kind_part = compact[..start].trim_matches(|c: char| c == ',' || c == '|' || c == ';');
        }
        let mut ck = CoinKind::new(kind_part.to_string());
        if !assets_part.trim().is_empty() {
            if !kind_part.contains('a') {
                return errf!("coinkind assets list requires 'a'")
            }
            ck.assets = CoinKind::parse_assets_list(assets_part)?;
            if ck.assets.is_empty() {
                return errf!("coinkind assets list empty")
            }
            ck.assets_all = false;
        }
        Ok(ck)
    }

    pub fn parse_assets_list(s: &str) -> Ret<Vec<u64>> {
        let mut end = s.len();
        for (i, c) in s.char_indices() {
            if !(c.is_ascii_digit() || c == ',' || c == '|' || c.is_whitespace()) {
                end = i;
                break;
            }
        }
        let list = s[..end].trim();
        if list.is_empty() {
            return Ok(vec![])
        }
        let mut out = Vec::new();
        for part in list.split(|c| c == ',' || c == '|') {
            let p = part.trim();
            if p.is_empty() {
                continue;
            }
            let v = p.parse::<u64>().map_err(|_| format!("asset serial {} format error", p))?;
            let _ = Fold64::from(v)?;
            out.push(v);
        }
        Ok(out)
    }
}

#[macro_export]
macro_rules! q_coinkind {
    ( $q: ident, $k: ident ) => (
        q_must!($q, $k, s!("hsda"));
        let $k = match CoinKind::parse($k) {
            Ok(v) => v,
            Err(e) => return api_error(&e),
        };
    )
}

#[macro_export]
macro_rules! q_unit {
    ( $q: ident, $k: ident ) => (
        q_must!($q, $k, s!("fin"));
    )
}

#[macro_export]
macro_rules! q_must {
    ( $q: ident, $k: ident, $dv: expr ) => (
        #[allow(unused_mut)]
        let mut $k = match $q.$k.clone() 
        {
            Some(v) => v,
            _ => $dv,
        };
    )
}

#[macro_export]
macro_rules! q_body_data_may_hex {
    ( $q: ident, $d: expr) => (
        { 
            q_must!($q, hexbody, false);
            let bddt = $d.to_vec();
            maybe!(hexbody, {
                let res = hex::decode(&bddt);
                if let Err(_) = res {
                    return api_error("hex format error")
                }
                res.unwrap()
            }, bddt)
        }
    )
}

#[macro_export]
macro_rules! q_hex {
    ( $d: expr) => (
        {
            let res = hex::decode(&$d);
            if let Err(_) = res {
                return api_error("hex format error")
            }
            res.unwrap()
        }
    )
}

#[macro_export]
macro_rules! q_addr {
    ($adr: expr) => ({
        let adr = Address::from_readable(&$adr);
        if let Err(e) = adr {
            return api_error(&format!("address {} format error: {}", &$adr, &e))
        }
        adr.unwrap()
    })
}

#[macro_export]
macro_rules! q_data_addr {
    ( $q: ident, $adr: ident) => (
        q_addr!(&$q.$adr)
    )
}

#[macro_export]
macro_rules! q_amt {
    ( $amt: expr) => ({
        let amt = Amount::from(&$amt);
        if let Err(e) = amt {
            return api_error(&format!("amount {} format error: {}", &$amt, &e))
        }
        amt.unwrap()
    })
}

#[macro_export]
macro_rules! q_data_amt {
    ( $q: ident, $amt: ident) => (
        q_amt!($q.$amt)
    )
}

#[macro_export]
macro_rules! q_data_acc_from {
    ( $acc: expr) => ({
        let acc = Account::create_by(&$acc);
        if let Err(e) = acc {
            return api_error(&format!("prikey error: {}", &e))
        }
        acc.unwrap()
    })
}

#[macro_export]
macro_rules! q_data_acc {
    ( $q: ident, $acc: ident) => (
        q_data_acc_from!($q.$acc)
    )
}

#[macro_export]
macro_rules! q_data_hash {
    ( $hxstr: ident) => ({
        let hx = hex::decode($hxstr);
        if let Err(e) = hx {
            return api_error(&format!("hash parse error: {}", &e))
        }
        let hx = hx.unwrap();
        if hx.len() != Hash::SIZE {
            return api_error(&format!("hash size error"))
        }
        Hash::from(hx.try_into().unwrap())
    })
}




#[macro_export]
macro_rules! api_querys_define {
    ( $name: ident, $( $item: ident, $ty: ty, $dv: expr,)+ ) => (

        #[derive(serde::Deserialize)]
        #[allow(dead_code)]
        struct $name {
            $(
                $item: $ty,
            )+
            unit: Option<String>,
            coinkind: Option<String>,
            hexbody: Option<bool>,
            base64body: Option<bool>,
            hex: Option<bool>,
            base64: Option<bool>,
            extendpath: Option<String>,
        }

        impl Default for $name {
            fn default() -> Self {
                Self { 
                    $(
                        $item: $dv,
                    )+
                    unit: None,
                    coinkind: None,
                    hexbody: None,
                    base64body: None,
                    hex: None,
                    base64: None,
                    extendpath: None,
                }
            }
        }

    )
}



/*******************************/


#[macro_export]
macro_rules! s {
    ("") => { String::new() };
    ($v:expr) => { ($v).to_string() };
}


pub fn start_with_char(s: &str, c: char) -> bool {
    maybe!(!s.is_empty(), s.as_bytes()[0] == c as u8, false)
}

pub fn bytes_to_readable_string(bts: &[u8]) -> String {
    let mut s = String::with_capacity(bts.len());
    for &b in bts {
        s.push(if (32..=126).contains(&b) { b as char } else { ' ' });
    }
    s.trim_end().to_owned()
}


pub fn bytes_from_readable_string(stuff: &[u8], len: usize) -> Vec<u8> {
    let mut bts = vec![b' '; len];
    for (dst, &src) in bts.iter_mut().zip(stuff.iter()) {
        *dst = if (32..=126).contains(&src) { src } else { b' ' };
    }
    bts
}

pub fn bytes_try_to_readable_string(bts: &[u8]) -> Option<String> {
    if !check_readable_string(bts) {
        return None
    }
    Some(std::str::from_utf8(bts).ok()?.to_owned())
}


pub fn bytes_to_readable_string_or_hex(bts: &[u8]) -> String {
    maybe!(
        check_readable_string(bts),
        std::str::from_utf8(bts).ok().unwrap().to_owned(),
        hex::encode(bts)
    )
}


pub fn check_readable_string(bts: &[u8]) -> bool {
    bts.iter().all(|a| (32..=126).contains(a))
}


pub fn left_readable_string(bts: &[u8]) -> String {
    let end = bts
        .iter()
        .position(|a| !(32..=126).contains(a))
        .unwrap_or(bts.len());
    std::str::from_utf8(&bts[..end]).ok().unwrap().trim_end().to_owned()
}

#[cfg(test)]
mod string_macro_tests {
    #[test]
    fn s_macro_empty_literal_is_empty_string() {
        let s = s!("");
        assert!(s.is_empty());
    }

    #[test]
    fn s_macro_nonempty_literal_keeps_content() {
        assert_eq!(s!("abc"), "abc");
    }
}

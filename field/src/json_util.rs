



pub enum JSONBinaryFormat {
    Hex,
    Base58Check,
    Base64,
}

pub struct JSONFormater {
    pub binary: JSONBinaryFormat,
    pub unit: String,
}

impl Default for JSONFormater {
    fn default() -> Self {
        Self {
            binary: JSONBinaryFormat::Hex,
            unit: String::new(),
        }
    }
}

impl JSONFormater {
    pub fn new_unit(unit: &str) -> Self {
        Self {
            binary: JSONBinaryFormat::Hex,
            unit: unit.to_owned(),
        }
    }
}


// No imports needed, as this file is included via include! in lib.rs
// which already has the necessary imports.


pub fn json_unquote(s: &str) -> &str {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        return &s[1..s.len()-1];
    }
    s
}

pub fn json_expect_quoted(s: &str) -> Ret<&str> {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        return Ok(&s[1..s.len()-1]);
    }
    errf!("json string must be quoted")
}

fn json_quote_is_escaped(s: &str, quote_idx: usize) -> bool {
    let bts = s.as_bytes();
    let mut n = 0usize;
    let mut i = quote_idx;
    while i > 0 {
        i -= 1;
        if bts[i] != b'\\' {
            break;
        }
        n += 1;
    }
    n % 2 == 1
}

fn json_u16_from_hex4(h: &[u8]) -> Ret<u16> {
    if h.len() != 4 {
        return errf!("invalid unicode escape");
    }
    let mut v: u16 = 0;
    for c in h {
        let n = match c {
            b'0'..=b'9' => (c - b'0') as u16,
            b'a'..=b'f' => (c - b'a' + 10) as u16,
            b'A'..=b'F' => (c - b'A' + 10) as u16,
            _ => return errf!("invalid unicode escape"),
        };
        v = (v << 4) | n;
    }
    Ok(v)
}

pub fn json_unescape_str(raw: &str) -> Ret<String> {
    let bts = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0usize;
    while i < bts.len() {
        if bts[i] != b'\\' {
            let ch = raw[i..]
                .chars()
                .next()
                .ok_or_else(|| "invalid utf-8".to_string())?;
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        i += 1;
        if i >= bts.len() {
            return errf!("invalid escape sequence");
        }
        match bts[i] {
            b'"' => out.push('"'),
            b'\\' => out.push('\\'),
            b'/' => out.push('/'),
            b'b' => out.push('\u{0008}'),
            b'f' => out.push('\u{000C}'),
            b'n' => out.push('\n'),
            b'r' => out.push('\r'),
            b't' => out.push('\t'),
            b'u' => {
                if i + 4 >= bts.len() {
                    return errf!("invalid unicode escape");
                }
                let mut cp = json_u16_from_hex4(&bts[i + 1..i + 5])? as u32;
                i += 4;
                if (0xD800..=0xDBFF).contains(&cp) {
                    if i + 6 >= bts.len() || bts[i + 1] != b'\\' || bts[i + 2] != b'u' {
                        return errf!("invalid unicode surrogate pair");
                    }
                    let lo = json_u16_from_hex4(&bts[i + 3..i + 7])? as u32;
                    if !(0xDC00..=0xDFFF).contains(&lo) {
                        return errf!("invalid unicode surrogate pair");
                    }
                    cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                    i += 6;
                } else if (0xDC00..=0xDFFF).contains(&cp) {
                    return errf!("invalid unicode surrogate pair");
                }
                let ch = char::from_u32(cp).ok_or_else(|| "invalid unicode scalar".to_string())?;
                out.push(ch);
            }
            _ => return errf!("invalid escape sequence"),
        }
        i += 1;
    }
    Ok(out)
}

pub fn json_expect_quoted_decoded(s: &str) -> Ret<String> {
    json_unescape_str(json_expect_quoted(s)?)
}

pub fn json_expect_unquoted(s: &str) -> Ret<&str> {
    let s = s.trim();
    if s.starts_with('"') || s.ends_with('"') {
        return errf!("json value must not be quoted");
    }
    Ok(s)
}

pub fn json_split(s: &str, start_char: char, end_char: char) -> Ret<Vec<&str>> {
    let s = s.trim();
    if !s.starts_with(start_char) || !s.ends_with(end_char) {
        return errf!("json root must be wrapped by '{}' and '{}'", start_char, end_char);
    }
    let content = &s[1..s.len()-1];
    let mut items = Vec::new();
    let mut depth = 0;
    let mut last_start = 0;
    let mut in_quote = false;

    for (i, c) in content.char_indices() {
        if c == '"' && !json_quote_is_escaped(content, i) {
            in_quote = !in_quote;
        }
        if !in_quote {
            if c == '{' || c == '[' {
                depth += 1;
            } else if c == '}' || c == ']' {
                depth -= 1;
            } else if c == ',' && depth == 0 {
                items.push(content[last_start..i].trim());
                last_start = i + 1;
            }
        }
    }
    let last_item = content[last_start..].trim();
    if !last_item.is_empty() {
        items.push(last_item);
    }
    Ok(items)
}

pub fn json_decode_object(s: &str) -> Ret<HashMap<String, String>> {
    Ok(json_split(s, '{', '}')?.into_iter().filter_map(|pair| {
        let mut depth = 0;
        let mut in_quote = false;
        for (i, c) in pair.char_indices() {
             if c == '"' && !json_quote_is_escaped(pair, i) {
                 in_quote = !in_quote;
             }
             if !in_quote {
                 if c == '{' || c == '[' {
                     depth += 1;
                 } else if c == '}' || c == ']' {
                     depth -= 1;
                 } else if c == ':' && depth == 0 {
                     let key = json_unquote(pair[..i].trim());
                     let val = pair[i+1..].trim();
                     return Some((key.to_string(), val.to_string()));
                 }
             }
        }
        None
    }).collect())
}

pub fn json_decode_array(s: &str) -> Ret<(Vec<String>, usize)> {
    let items = json_split(s, '[', ']')?
        .into_iter()
        .map(|x| x.to_string())
        .collect::<Vec<String>>();
    let n = items.len();
    Ok((items, n))
}

pub fn json_split_array(s: &str) -> Ret<Vec<&str>> {
    json_split(s, '[', ']')
}

pub fn json_split_object(s: &str) -> Ret<Vec<(&str, &str)>> {
    Ok(json_split(s, '{', '}')?.into_iter().filter_map(|pair| {
        let mut depth = 0;
        let mut in_quote = false;
        for (i, c) in pair.char_indices() {
            if c == '"' && !json_quote_is_escaped(pair, i) {
                in_quote = !in_quote;
            }
            if !in_quote {
                if c == '{' || c == '[' {
                    depth += 1;
                } else if c == '}' || c == ']' {
                    depth -= 1;
                } else if c == ':' && depth == 0 {
                    let key = json_unquote(pair[..i].trim());
                    let val = pair[i+1..].trim();
                    return Some((key, val));
                }
            }
        }
        None
    }).collect())
}

pub fn json_decode_binary(s: &str) -> Ret<Vec<u8>> {
    let raw = json_expect_quoted_decoded(s)?;
    let trimmed = raw.trim();
    // 0x / 0X: hex (trim content)
    if trimmed.len() >= 2 && (trimmed.starts_with("0x") || trimmed.starts_with("0X")) {
        let hx = trimmed[2..].trim();
        if hx.len() % 2 != 0 || !hx.chars().all(|c| c.is_ascii_hexdigit()) {
            return errf!("invalid hex string");
        }
        let b = hex::decode(hx).map_err(|e| e.to_string())?;
        return Ok(b);
    }
    // b64: / B64:: base64 (trim content)
    if trimmed.len() >= 4 && (trimmed.starts_with("b64:") || trimmed.starts_with("B64:")) {
        let rest = trimmed[4..].trim();
        let b = BASE64_STANDARD.decode(rest).map_err(|e| e.to_string())?;
        return Ok(b);
    }
    // b58: / B58:: base58check (trim content)
    if trimmed.len() >= 4 && (trimmed.starts_with("b58:") || trimmed.starts_with("B58:")) {
        let rest = trimmed[4..].trim();
        if rest.is_empty() {
            return Ok(vec![]);
        }
        let (_ver, b) = rest.from_base58check().map_err(|e| format!("base58check failed: {:?}", e))?;
        let mut full = vec![_ver];
        full.extend(b);
        return Ok(full);
    }
    // no prefix: plain string (UTF-8 bytes, no trim)
    Ok(raw.as_bytes().to_vec())
}

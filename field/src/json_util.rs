



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

pub fn json_expect_unquoted(s: &str) -> Ret<&str> {
    let s = s.trim();
    if s.starts_with('"') || s.ends_with('"') {
        return errf!("json value must not be quoted");
    }
    Ok(s)
}

pub fn json_split(s: &str, start_char: char, end_char: char) -> Vec<&str> {
    let s = s.trim();
    if !s.starts_with(start_char) || !s.ends_with(end_char) {
        return vec![];
    }
    let content = &s[1..s.len()-1];
    let mut items = Vec::new();
    let mut depth = 0;
    let mut last_start = 0;
    let mut in_quote = false;
    let mut last_char = ' ';

    for (i, c) in content.char_indices() {
        if c == '"' && last_char != '\\' {
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
        last_char = c;
    }
    let last_item = content[last_start..].trim();
    if !last_item.is_empty() {
        items.push(last_item);
    }
    items
}

pub fn json_decode_object(s: &str) -> Ret<HashMap<String, String>> {
    Ok(json_split(s, '{', '}').into_iter().filter_map(|pair| {
        let mut depth = 0;
        let mut in_quote = false;
        let mut last_char = ' ';
        for (i, c) in pair.char_indices() {
             if c == '"' && last_char != '\\' {
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
             last_char = c;
        }
        None
    }).collect())
}

pub fn json_decode_array(s: &str) -> Ret<(Vec<String>, usize)> {
    let items = json_split(s, '[', ']')
        .into_iter()
        .map(|x| x.to_string())
        .collect::<Vec<String>>();
    let n = items.len();
    Ok((items, n))
}

pub fn json_split_array(s: &str) -> Vec<&str> {
    json_split(s, '[', ']')
}

pub fn json_split_object(s: &str) -> Vec<(&str, &str)> {
    json_split(s, '{', '}').into_iter().filter_map(|pair| {
        let mut depth = 0;
        let mut in_quote = false;
        let mut last_char = ' ';
        for (i, c) in pair.char_indices() {
            if c == '"' && last_char != '\\' {
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
            last_char = c;
        }
        None
    }).collect()
}

pub fn json_decode_binary(s: &str) -> Ret<Vec<u8>> {
    let s = json_expect_quoted(s)?;
    // 0. hex must be prefixed with 0x / 0X
    if s.starts_with("0x") || s.starts_with("0X") {
        let hx = &s[2..];
        if hx.len() % 2 != 0 || !hx.chars().all(|c| c.is_ascii_hexdigit()) {
            return errf!("invalid hex string");
        }
        let b = hex::decode(hx).map_err(|e| e.to_string())?;
        return Ok(b);
    }
    // 0. Hacash specific: if it looks like an address (starts with '1' or '3')
    // or has length around 34, try base58check first
    if (s.starts_with('1') || s.starts_with('3')) && s.len() >= 26 && s.len() <= 35 {
        if let Ok((_ver, b)) = s.from_base58check() {
            let mut full = vec![_ver];
            full.extend(b);
            return Ok(full);
        }
    }

    // 1. try base58check again (for non-address data)
    if let Ok((_ver, b)) = s.from_base58check() {
        let mut full = vec![_ver];
        full.extend(b);
        return Ok(full);
    }

    // 2. try base64
    if let Ok(b) = BASE64_STANDARD.decode(s) {
        return Ok(b);
    }
    errf!("cannot decode binary string: {}", s)
}

#[allow(dead_code)]
#[derive(Default)]
pub struct Tokenizer<'a> {
    texts: &'a [u8],
    idx: usize,
    tokens: Vec<Token>,
}

#[allow(dead_code)]
impl Tokenizer<'_> {
    pub fn new<'a>(texts: &'a [u8]) -> Tokenizer<'a> {
        Tokenizer {
            texts,
            ..Default::default()
        }
    }

    fn parse_num_bytes_or_address(s: &str) -> Ret<Token> {
        if s.starts_with("0x") {
            // Ox1AE23F
            let v = s.to_owned().split_off(2);
            return Ok(match hex::decode(v) {
                Ok(d) => Bytes(d),
                _ => return errf!("hex data format error '{}'", s),
            });
        } else if s.starts_with("0b") && s.len() >= 10 {
            // 0b11110000
            let e = errf!("binary data '{}' format error ", s);
            let v = s.to_owned().split_off(2);
            let vl = v.len();
            if vl % 8 != 0 {
                return e;
            }
            let n = vl / 8;
            return Ok(match u128::from_str_radix(&v, 2) {
                Ok(d) => Bytes(d.to_be_bytes()[16 - n..].to_vec()),
                _ => return e,
            });
        } else if let Some(addr) = Self::parse_address(s) {
            // address
            return Ok(addr);
        }
        // maybe uint
        Ok(Integer(match s.parse::<u128>() {
            Ok(u) => u,
            _ => return errf!("parse Integer type error for '{}'", s),
        }))
    }

    fn parse_address(s: &str) -> Option<Token> {
        let sl = s.len();
        if sl < 30 || sl > 34 {
            return None;
        }
        match field::Address::from_readable(s) {
            Ok(a) => Some(Address(a)),
            _ => None,
        }
    }

    pub fn parse_comments(&mut self, max: usize) -> Ret<bool> {
        let c = self.texts[self.idx] as char;
        macro_rules! gtc {
            ($n: expr) => {
                self.texts[self.idx + $n] as char
            };
        }
        match c {
            '/' => {
                // single line comments
                self.idx += 1;
                while self.idx < max && gtc!(0) != '\n' {
                    self.idx += 1;
                }
                Ok(true)
            }
            '*' => {
                // multiple line comments
                self.idx += 1;
                let mut closed = false;
                while self.idx < max - 1 {
                    if gtc!(0) == '*' && gtc!(1) == '/' {
                        self.idx += 2;
                        closed = true;
                        break;
                    }
                    self.idx += 1;
                }
                if !closed {
                    return errf!("unterminated block comment");
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub fn parse_identifier(&mut self, max: usize, c: char) -> Rerr {
        let mut s = String::new();
        s.push(c);
        while self.idx < max {
            let c = self.texts[self.idx] as char;
            match c {
                '0'..='9' | 'a'..='z' | 'A'..='Z' | '$' | '_' => s.push(c),
                _ => break,
            }
            self.idx += 1;
        }
        let ok = Ok(());
        if let Ok(k) = KwTy::build(&s) {
            self.tokens.push(Keyword(k));
            return ok;
        }
        self.tokens.push(match Self::parse_address(&s) {
            Some(addr) => addr,
            _ => Identifier(s.clone()),
        });
        ok
    }

    pub fn parse_number(&mut self, max: usize, c: char) -> Rerr {
        let mut s = String::new();
        s.push(c);
        while self.idx < max {
            let c = self.texts[self.idx] as char;
            match c {
                '0'..='9' | 'a'..='z' | 'A'..='Z' | '_' => s.push(c),
                _ => break,
            }
            self.idx += 1;
        }

        // Check for type suffix format: number + type (e.g., 100u8, 1000_u64)
        let type_suffixes = ["u8", "u16", "u32", "u64", "u128"];
        let mut num_part = s.clone();
        let mut suffix_kw: Option<KwTy> = None;

        for suffix in &type_suffixes {
            if s.ends_with(suffix) {
                let without_suffix = &s[..s.len() - suffix.len()];
                // Remove underscores from number part
                let without_underscores: String =
                    without_suffix.chars().filter(|c| *c != '_').collect();
                if !without_underscores.is_empty() && without_underscores.parse::<u128>().is_ok() {
                    num_part = without_suffix.to_string();
                    if let Ok(kw) = KwTy::build(suffix) {
                        suffix_kw = Some(kw);
                    }
                    break;
                }
            }
        }

        // If no suffix, remove underscores from the entire string
        if suffix_kw.is_none() {
            let without_underscores: String = s.chars().filter(|c| *c != '_').collect();
            if !without_underscores.is_empty() {
                num_part = without_underscores;
            }
        }

        // Parse the number part first
        let token = Self::parse_num_bytes_or_address(&num_part)?;
        self.tokens.push(token);

        // Then push the type suffix keyword (if any)
        if let Some(kw) = suffix_kw {
            self.tokens.push(Keyword(kw));
        }
        Ok(())
    }

    pub fn parse_symbol(&mut self, max: usize, c: char) -> Rerr {
        let mut s = String::new();
        s.push(c);
        while self.idx < max {
            let c = self.texts[self.idx] as char;
            match c {
                '+' | '-' | '*' | '/' | '=' | '!' | '.' | ':' | '>' | '<' | '|' | '&' | '%'
                | '^' => s.push(c),
                _ => break,
            }
            self.idx += 1;
        }
        let ok = Ok(());
        if let Ok(k) = KwTy::build(&s) {
            self.tokens.push(Keyword(k));
            return ok;
        }
        if let Ok(o) = OpTy::build(&s) {
            self.tokens.push(Operator(o));
            return ok;
        }
        errf!("unsupport symbol '{}'", s)
    }

    pub fn parse_bytes(&mut self, max: usize, _c: char) -> Rerr {
        let e = errf!("bytes format error");
        let mut s = vec![];
        let mut closed = false;
        while self.idx < max {
            let c = self.texts[self.idx] as char;
            if c == '\"' {
                closed = true;
                self.idx += 1;
                break;
            }
            // print!("{}", c);
            if c == '\\' {
                let nxt = self.idx + 1;
                if nxt >= max {
                    return e;
                }
                let a = self.texts[nxt] as char;
                s.push(match a {
                    't' => '\t',
                    'n' => '\n',
                    'r' => '\r',
                    '\\' => '\\',
                    b => b,
                } as u8);
                self.idx += 1;
            } else {
                s.push(c as u8);
            }
            self.idx += 1;
        }
        if !closed {
            return e;
        }
        self.tokens.push(Bytes(s));
        Ok(())
    }

    pub fn parse_char(&mut self, max: usize, _c: char) -> Rerr {
        let e = errf!("char format error");
        // 需要确保有字符内容和闭引号: idx < max - 1
        if self.idx >= max - 1 {
            return e;
        }

        let byte = if self.texts[self.idx] == b'\\' {
            // 转义序列
            let nxt = self.idx + 1;
            if nxt >= max {
                return e;
            }
            let esc = self.texts[nxt] as char;
            self.idx += 2; // 跳过转义字符
            match esc {
                't' => b'\t',
                'n' => b'\n',
                'r' => b'\r',
                '\\' => b'\\',
                '\'' => b'\'',
                _ => return e,
            }
        } else {
            // 普通字符
            let byte = self.texts[self.idx];
            self.idx += 1;
            if byte == b'\'' {
                return e; // 不能是开引号
            }
            byte
        };

        // 检查闭引号
        if self.idx >= max || self.texts[self.idx] != b'\'' {
            return e;
        }
        self.idx += 1;

        self.tokens.push(Character(byte));
        Ok(())
    }

    pub fn parse(mut self) -> Ret<Vec<Token>> {
        // use TokenType::*;
        let max = self.texts.len();
        while self.idx < max {
            let c = self.texts[self.idx] as char;
            self.idx += 1;
            if c == '/' && self.idx < max {
                if self.parse_comments(max)? {
                    continue;
                }
            }
            match c {
                '0'..='9' => self.parse_number(max, c)?,
                'A'..='Z' | 'a'..='z' | '$' | '_' => self.parse_identifier(max, c)?,
                '{' | '}' | '(' | ')' | '[' | ']' => self.tokens.push(Partition(c)),
                '+' | '-' | '*' | '/' | '=' | '!' | '.' | ':' | '>' | '<' | '|' | '&' | '%'
                | '^' => self.parse_symbol(max, c)?,
                '"' => self.parse_bytes(max, c)?,
                '\'' => self.parse_char(max, c)?,
                ' ' | ',' | ';' | '\n' | '\r' | '\t' => {} // ignore
                _ => return errf!("unsupport char [{}]", c),
            }
        }
        Ok(self.tokens)
    }
}

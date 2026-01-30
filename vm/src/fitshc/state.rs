use sys::{Ret, errf};
use sys::*;
use crate::rt::*;
use crate::contract::Contract;
use crate::rt::SourceMap;
use crate::Token::*;
use super::parse_deploy::DeployInfo;

pub struct ParseState {
    pub tokens: Vec<Token>,
    pub idx: usize,
    pub max: usize,
    pub contract: Contract,
    pub contract_name: String,
    pub deploy: Option<DeployInfo>,
    pub source_maps: Vec<(String, SourceMap)>,
}

impl ParseState {
    pub fn new(tokens: Vec<Token>) -> Self {
        let max = tokens.len();
        Self {
            tokens,
            idx: 0,
            max,
            contract: Contract::new(),
            contract_name: String::new(),
            deploy: None,
            source_maps: Vec::new(),
        }
    }

    pub fn current(&self) -> Option<&Token> {
        if self.idx >= self.max { None } else { Some(&self.tokens[self.idx]) }
    }

    pub fn advance(&mut self) {
        self.idx += 1;
    }

    pub fn eat_partition(&mut self, char: char) -> Ret<()> {
        if self.idx >= self.max {
             return errf!("expected '{}' but got EOF", char)
        }
        if let Partition(c) = self.tokens[self.idx] {
            if c == char {
                self.idx += 1;
                return Ok(())
            }
        }
        errf!("expected '{}' but got {:?}", char, self.tokens[self.idx])
    }
}

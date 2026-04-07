use std::fmt::{Debug, Display, Formatter, Result};

use sys::ToHex;

use crate::rt::ItrErrCode::*;
use crate::rt::*;
use crate::value::*;

macro_rules! read_be_value {
    ($bytes:expr, $len:expr, $ty:ty, $ctor:ident) => {{
        let mut buf = [0u8; $len];
        buf.copy_from_slice($bytes);
        Value::$ctor(<$ty>::from_be_bytes(buf))
    }};
}

#[allow(dead_code)]
#[derive(Default)]
pub struct Heap {
    // bsgas: i64,   // 1 2 4 8 16 32 64 128 256 512 1024 2048 4096 8192 segln: usize, // 256
    limit: usize, // 64 seg
    datas: Vec<u8>,
}

impl Display for Heap {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "0x{}", self.datas.to_hex())
    }
}

impl Debug for Heap {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "heap({}):0x{}", self.datas.len(), self.datas.to_hex())
    }
}

impl Heap {
    pub const SEGLEN: usize = 256;

    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            ..Default::default()
        }
    }

    pub fn reset(&mut self, limit: usize) {
        self.limit = limit;
        self.datas.clear()
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}

impl Heap {
    #[inline(always)]
    fn checked_right(&self, start: usize, len: usize, tip: &'static str) -> VmrtRes<usize> {
        let right = start
            .checked_add(len)
            .ok_or_else(|| ItrErr::new(HeapError, tip))?;
        if right > self.datas.len() {
            return Err(ItrErr::new(HeapError, tip));
        }
        Ok(right)
    }

    #[inline(always)]
    fn read_uint(&self, uty: u16, seg: u16) -> VmrtRes<Value> {
        let len = match uty {
            0 => 1usize,
            1 => 2,
            2 => 4,
            3 => 8,
            4 => 16,
            _ => return itr_err_fmt!(HeapError, "uint type {} not supported", uty),
        };
        let idx = len
            .checked_mul(seg as usize)
            .ok_or_else(|| ItrErr::new(HeapError, "read overflow"))?;
        let right = self.checked_right(idx, len, "read overflow")?;
        let bytes = &self.datas[idx..right];
        Ok(match uty {
            0 => Value::U8(bytes[0]),
            1 => read_be_value!(bytes, 2, u16, U16),
            2 => read_be_value!(bytes, 4, u32, U32),
            3 => read_be_value!(bytes, 8, u64, U64),
            4 => read_be_value!(bytes, 16, u128, U128),
            _ => return itr_err_fmt!(HeapError, "uint type {} not supported", uty),
        })
    }

    fn calc_grow_gas(&self, seg: usize) -> VmrtRes<i64> {
        let oldseg = self.datas.len() / Self::SEGLEN;
        if oldseg + seg > self.limit {
            return itr_err_code!(OutOfHeap);
        }
        // Gas is an abstraction of space usage: the first 8 segments are charged exponentially (2,4,8,16,32,64,128,256), then linear 256 per segment. Price is based on existing heap size so multiple HGROW(1) cannot bypass.
        let mut gas: u64 = 0;
        for s in oldseg..(oldseg + seg) {
            let add = if s < 8 {
                1u64.checked_shl((s + 1) as u32).unwrap_or(u64::MAX)
            } else {
                Self::SEGLEN as u64
            };
            gas = gas
                .checked_add(add)
                .ok_or_else(|| ItrErr::new(HeapError, "heap grow gas overflow"))?;
        }
        Ok(gas as i64)
    }

    pub fn grow(&mut self, seg: u8) -> VmrtRes<i64> {
        let seg = seg as usize;
        if seg < 1 {
            return itr_err_fmt!(HeapError, "heap grow cannot be empty");
        }
        if seg > 16 {
            return itr_err_fmt!(HeapError, "heap grow cannot exceed 16");
        }
        let gas = self.calc_grow_gas(seg)?;
        let newsz = self.datas.len() + seg * Self::SEGLEN;
        self.datas.resize(newsz, 0u8);
        Ok(gas)
    }

    fn do_write(&mut self, start: usize, v: Value) -> VmrtErr {
        let data = v
            .extract_bytes()
            .map_err(|ItrErr(_, msg)| ItrErr::new(HeapError, &msg))?;
        let right = self.checked_right(start, data.len(), "write overflow")?;
        self.datas[start..right].copy_from_slice(&data);
        Ok(())
    }

    /* pub fn write(&mut self, k: Value, v: Value) -> VmrtErr { let start = k.extract_u32()? as usize; self.do_write(start, v) } pub fn write_x(&mut self, start: u8, v: Value) -> VmrtErr { self.do_write(start as usize, v) } */

    pub fn write(&mut self, start: u16, v: Value) -> VmrtErr {
        self.do_write(start as usize, v)
    }

    pub fn do_read(&self, start: usize, len: usize) -> VmrtRes<Value> {
        let max = self.checked_right(start, len, "read overflow")?;
        let data = &self.datas[start..max];
        Ok(Value::Bytes(data.to_vec()))
    }

    // return Value::bytes
    pub fn read(&self, i: &Value, n: Value) -> VmrtRes<Value> {
        let start = i.extract_u32()? as usize;
        let length = n.extract_u16()? as usize;
        self.do_read(start, length)
    }

    pub fn slice(&self, l: Value, s: &Value) -> VmrtRes<Value> {
        let start = s.extract_u32()?;
        let length = l.extract_u32()?;
        self.checked_right(start as usize, length as usize, "create slice overflow")?;
        Ok(Value::HeapSlice((start, length)))
    }

    /* 2 bit = u8 u16 u32 u64 6 bit = seg max 64 (u8:64, u16:128, u32:256, u64:512) */
    pub fn read_u(&self, mark: u8) -> VmrtRes<Value> {
        let uty = mark >> 6;
        let seg = mark & 0b00111111;
        self.read_uint(uty as u16, seg as u16)
    }

    /* 3 bit = u8 u16 u32 u64 u128; remaining 13 bits encode the segment */
    pub fn read_ul(&self, mark: u16) -> VmrtRes<Value> {
        // upper 3 bits indicate uint type; remaining 13 bits indicate segment shift by 13 (5+8) explicitly to avoid precedence ambiguity
        let uty = mark >> 13;
        let seg = mark & 0b0001111111111111;
        self.read_uint(uty, seg)
    }
}

#[cfg(test)]
mod heaptest {
    use super::*;

    #[test]
    fn calc_grow_gas_matches_doc_examples() {
        let mut heap = Heap::default();
        heap.limit = 64;
        assert_eq!(heap.calc_grow_gas(1).unwrap(), 2);
        assert_eq!(heap.calc_grow_gas(8).unwrap(), 510);
        assert_eq!(heap.calc_grow_gas(10).unwrap(), 1022);

        // price depends on existing heap size (cannot bypass by splitting calls)
        assert_eq!(heap.grow(1).unwrap(), 2);
        assert_eq!(heap.grow(1).unwrap(), 4);
        assert_eq!(heap.grow(1).unwrap(), 8);
    }

    #[test]
    fn slice_overflow_is_rejected() {
        let heap = Heap::new(64);
        let start = Value::U32(u32::MAX);
        let len = Value::U32(1);
        let err = heap.slice(len, &start).unwrap_err().to_string();
        assert!(err.contains("create slice overflow"));
    }
}

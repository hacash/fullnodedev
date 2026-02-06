Gas Cost
===


## Base cost every opcode

These are fixed and can be regarded as the basic overhead of instructions. Opcodes not appearing in the list are all 2.

- 1: PU8, P0, P1, P2, P3, PNBUF, PNIL, 
    CU8, CU16, CU32, CU64, CU128, CBUF, CTO, TID, TIS, TNIL, TMAP, TLIST, 
    POP, NOP, NT, END, RET, ABT, ERR, AST, PRT
- 2: `all other opcode`
- 3: BRL, BRS, BRSL, BRSLN, XLG, PUT, CHOISE
- 4: DUPN, POPN, PICK,
    PBUF, PBUFL,
    MOD, MUL, DIV, XOP, 
    HREAD, HREADU, HREADUL, HSLICE, HGROW,
    ITEMGET, HEAD, TAIL, HASKEY, LENGTH,
- 5: POW
- 6: HWRITE, HWRITEX, HWRITEXL, 
    INSERT, REMOVE, CLEAR, APPEND
- 8: CAT, BYTE, CUT, LEFT, RIGHT, LDROP, RDROP,
    MGET, JOIN, REV, 
    NEWLIST, NEWMAP,
    NTCALL
- 12: EXTENV, MPUT, CALLTHIS, CALLSELF, CALLSUPER,
    PACKLIST, PACKMAP, UPLIST, CLONE, MERGE, KEYS, VALUES
- 16: EXTFUNC, GGET, CALLCODE 
- 20: LOG1, CALLPURE
- 24: LOG2, GPUT, CALLVIEW
- 28: LOG3, SDEL, EXTACTION
- 32: LOG4, SLOAD, SREST, CALL
- 64: SSAVE, SRENT


## Gas limits

- max_gas_of_tx: 8192 (= 65536 / 8). This is a hard cap for per-tx gas.
- min_of_main_call: 48. The VM call will consume at least this gas from the shared counter.
- min_of_p2sh_call: 72. The VM call will consume at least this gas from the shared counter.
- min_of_abst_call: 96. The VM call will consume at least this gas from the shared counter.


## Extra cost every behavior

These are dynamic and depend on the size of the resources or the scale of usage. `byte` does not include key or any type/meta data, only the pure bytes of the value. Except for Compo map total size, key_byte is not included in any other calculation. `item` refers to the number of elements in a Compo value. Operation settings: 1 gas = 1 byte = 1 item, byte/10 = byte*0.1. `byte/N` uses integer truncation. Gas is an `i64`, so divisions are integer divisions with truncation.

#### Space alloc

- 5:   every local stack slot
- 20:  every memory key
- 32:  every global key
- 32:  every contract load
- 256: every heap segment ( the first 8 segments of the HGROW command grow exponentially, and after 8 segments they change to linear growth. start from 2, 4, 8, 16, 32, 64, 128 to 256, the linear part is fixed at 256 gas per segment. the actual size of one heap segment is 256 bytes )
- 256: every storage key


#### Stack buffer copy

Only for opcode that generate new values on the stack.

`byte` is of type i64, and calculations like byte/12 lead to a normal side effect: free for value bytes < 12

- byte/12: DUP, GET*, MGET, GGET

#### Extend call handle

EXTENV has no parameter passing, so there is no dynamic billing.

NTCALL has three kinds of charges: 1-base, 2-extra, 3-func. The func fee is defined in the NTCALL function code and is returned as a fixed value after the call.

- byte/16: NTCALL    (op base is 8)
- byte/16: EXTFUNC   (op base is 16)
- byte/10: EXTACTION (op base is 28)

#### Heap read and write

- byte/16: HREAD* (byte = read length)
- byte/12: HWRITE* (byte = written value_byte)

#### Compo value handle

Some opcode should be charged based on both the number of items and the total size. item/4 is item*0.25, item/2 is item*0.5

KEYS, VALUES byte refers to the total bytes output (create on stack). map byte = all key_byte + all value_byte, list byte = all value_byte

The byte in ITEMGET/HEAD/TAIL refers to outputting value_byte; list types do not include key_byte.

- item/4:  ITEMGET, HEAD, TAIL, HASKEY, UPLIST, APPEND
- item/2:  KEYS, VALUES, INSERT, REMOVE
- item/1:  CLONE, MERGE
- byte/20: CLONE, KEYS, VALUES, ITEMGET, HEAD, TAIL

#### Log render

- byte/1: LOG*

#### Storage read and write

The key_byte is not calculated here.

- byte/8: SLOAD
- byte/6: SSAVE (free make-up for one period)

#### Storage rent

1 period = 100 blocks

- (32+byte)/1: every period (32 is base bytes, byte = value_byte)

#### Contract load

byte = value_byte = ContractSto.size(), not include key_byte. The formula is as follows:


- 32 + byte/64: every new contract load


## Examples (visualized)

Assume `1 gas = 1 byte`. The following examples show how to compute total cost:

1) SLOAD (value_byte = 40)
```
base = 32
dynamic = 40 / 8 = 5
total = 37 gas
```

2) SSAVE (value_byte = 80, 1 period rent)
```
base = 64
dynamic_write = 80 / 6 = 13
rent = (32 + 80) * 1 = 112
total = 64 + 13 + 112 = 189 gas
```

3) LOG2 (value_byte = 100)
```
base = 24
dynamic = 100 / 1 = 100
total = 124 gas
```

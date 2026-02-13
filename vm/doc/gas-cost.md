Gas Cost
===


## Base cost every opcode

These are fixed and can be regarded as the basic overhead of instructions. Opcodes not appearing in the list are all 2.

- 1: PU8, P0, P1, P2, P3, PNBUF, PNIL, PTRUE, PFALSE,
    CU8, CU16, CU32, CU64, CU128, CBUF, CTO, TID, TIS, TNIL, TMAP, TLIST, 
    POP, NOP, NT, END, RET, ABT, ERR, AST, PRT
- 2: `all other opcode`
- 3: BRL, BRS, BRSL, BRSLN, XLG, PUT, CHOOSE
- 4: DUPN, POPN, PICK,
    PBUF, PBUFL,
    MOD, MUL, DIV, XOP, 
    HREAD, HREADU, HREADUL, HSLICE, HGROW,
    ITEMGET, HEAD, BACK, HASKEY, LENGTH,
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

## tx.gas_max (1-byte) encoding

`TransactionType3.gas_max` is a 1-byte lookup key. The VM reads it via `TransactionRead::fee_extend()` and decodes it with `decode_gas_budget()` (see `vm/src/rt/gas.rs`).

- `gas_max=0`: no VM gas budget (non-contract tx); any contract/p2sh call will fail to initialize gas.
- `gas_max>0`: decoded budget is read from `GAS_BUDGET_LOOKUP_1P07_FROM_138` and then clamped by `max_gas_of_tx` (chain hard cap).

Decode (byte → gas):

- `b=0` → `0`
- `b in 1..=255`:
  - `gas = GAS_BUDGET_LOOKUP_1P07_FROM_138[b]`
  - table generation rule: `table[i] = floor(138 * 1.07^i)`, `i in 0..=255`
  - table max: `table[255] = 4,292,817,207` (fits in `u32`)

Practical notes on current L1 parameters (`max_gas_of_tx=8192`):

- The lookup table is dense and strictly increasing (1-byte full scale), suitable for future L2/high-cap settings.
- Under current L1 cap, bytes above the clamp threshold are still equivalent after clamping.


## Extra cost every behavior

These are dynamic and depend on the size of the resources or the scale of usage. `byte` does not include key or any type/meta data, only the pure bytes of the value. Except for Compo map total size, key_byte is not included in any other calculation. `item` refers to the number of elements in a Compo value. Operation settings: 1 gas = 1 byte = 1 item, byte/10 = byte*0.1. `byte/N` uses integer truncation. Gas is an `i64`, so divisions are integer divisions with truncation.

#### Space alloc

- 5:   every local stack slot
- 20:  every memory key
- 32:  every global key
- 32:  every contract load
- 256: every heap segment in linear ( the first 8 segments of the HGROW command grow exponentially, and after 8 segments they change to linear growth. start from 2, 4, 8, 16, 32, 64, 128 to 256, the linear part is fixed at 256 gas per segment. the actual size of one heap segment is 256 bytes )
  - Heap segment growth:
    - First 8 segments: exponential growth (2, 4, 8, 16, 32, 64, 128, 256 gas per segment)
    - After 8 segments: linear growth (256 gas per segment)
    - Example: HGROW 1 = 2 gas, HGROW 8 = 510 gas, HGROW 10 = 1022 gas
- 256: every (re)created storage key


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

KEYS, VALUES byte refers to the total bytes output (create on stack). map byte = all key_byte + all value_byte, list byte = all value_byte, total_gas = base + item_gas + byte_gas

The byte in ITEMGET/HEAD/BACK refers to outputting value_byte; list types do not include key_byte.

- item/4:  ITEMGET, HEAD, BACK, HASKEY, UPLIST, APPEND
- item/2:  KEYS, VALUES, INSERT, REMOVE
- item/1:  CLONE, MERGE
- byte/20: CLONE, KEYS, VALUES, ITEMGET, HEAD, BACK

#### Log render

byte = sum(all_param_bytes) = topic_bytes + sum(all_data_bytes)

- byte/1: LOG*

#### Storage read and write

The key_byte is not calculated here.

SSAVE pricing semantics:
- writing an expired key is treated as (re)create: charge 1-period rent and key-create fee.
- writing a valid key with remaining lease `< 1 period` triggers auto-renew to 1 period and charges 1-period rent.

- byte/8: SLOAD
- byte/6: SSAVE (write bytes; rent/key-create fee may also apply depending on key state)

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

2) SSAVE (value_byte = 80, renew/new-key includes 1 period rent; if new-key add +256)
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


## Expected Usage (DeFi Scenarios)

This section provides a 3-tier “DeFi operation” **expected resource usage** and a rough gas estimate table, intended to help contract developers quickly gauge whether a workflow may approach `max_gas_of_tx=8192`.

Assumptions and notes:

- These are estimation templates. Real costs depend on opcode mix, contract sizes, host-returned gas from `EXTACTION/EXTFUNC`, and whether `SSAVE` triggers renewals or (re)creates a storage key.
- The “Other compute opcodes” column is meant to count opcodes **excluding** those already accounted for as IO/heavy ops in this table (e.g. `SLOAD/SSAVE/LOG*/HGROW/HREAD/HWRITE/EXT*/NTCALL`), to avoid double counting.
- Contract load cost: `32 * new_loads + sum(contract_bytes/64)` (integer truncation).
- `SSAVE` has two typical pricing cases:
  - **Normal write**: `64 + value_byte/6`
  - **New / expired-recreate / auto-renew triggered**: on top of normal write, add `rent_one_period = 32 + value_byte`; for new/recreate also add `storage_key_cost=256`.

> In the table, `value_byte=8` corresponds to `U64`-like balances/reserves. If you store `Bytes(32)` (e.g. hash/commitment), replace `8` with `32` and recompute.

| Scenario | Contract Loads | EXTACTION | NTCALL | Other Compute Opcodes (rough) | Heap (rough) | Storage (rough) | Log (rough) | Estimated Total Gas (range) |
|---|---|---|---|---|---|---|---|---|
| Light DeFi op (e.g. single-pool fee settle / single-contract balance move) | 1 new load; contract ~8KB | 0 | 0 | ~300 opcodes (avg 2 gas/op → ~600) | none | `SLOAD*2` (value_byte=8) + `SSAVE*2` (existing, value_byte=8) | `LOG2*1` (total value_byte ~24) | ~1000–~1100 (example: `160(load)+600(code)+196(storage)+48(log)=1004`; if SSAVE renew triggers +80; if new key is created cost rises significantly) |
| Typical DeFi op (e.g. AMM swap / lending borrow/repay step) | 3 new loads; each contract ~8KB | 1; body ~96B; host returned gas ~200 (example) | 3 hash calls (e.g. sha2/sha3/ripemd160; input ~32B) | ~1200 opcodes (~2400) | `HGROW 2 seg` + `HWRITE 512B` + `HREAD 512B` | `SLOAD*10` (8B) + `SSAVE*6` (existing, 8B) | `LOG3*2` (~48B each) | ~4200–~5200 (example: `480(load)+2400(code)+237(ext)+119(nt)+94(heap)+720(storage)+152(log)=4202`; depends on host gas and whether SSAVE renew/new happens) |
| Heavy DeFi op (e.g. liquidation / multi-hop routing / batch distribution step; near gas cap) | 6 new loads; each contract ~12KB | 2; body ~256B; host returned gas ~400 (example) | 6 hash calls (input ~32B) | ~1800 opcodes (~3600) | `HGROW 4 seg` + `HWRITE 1024B` + `HREAD 1024B` | `SLOAD*20` (8B) + `SSAVE*12` (existing, 8B) | `LOG4*2` (~64B each) | ~7900–~8600 (example: `1344(load)+3600(code)+906(ext)+264(nt)+193(heap)+1440(storage)+192(log)=7939`; easy to exceed 8192 due to SSAVE renew/new or high host-returned gas) |

Suggested workflow (quick estimate):

1. List contracts/libraries touched, estimate new contract loads and contract sizes.
2. List counts of `SLOAD/SSAVE/SRENT` and their `value_byte` (store `U64` → 8B; common hashes → 32B).
3. If you assemble large parameters in heap (HeapSlice feeding `NTCALL/EXT*`), list `HGROW` segments and read/write byte sizes.
4. Treat `EXTACTION/EXTFUNC` body bytes and host-returned gas as variables; start conservative, then calibrate with on-chain measurements.

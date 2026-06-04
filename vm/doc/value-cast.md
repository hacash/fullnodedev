# Value Conversion and Normalization Spec

This document is the single source of truth for `Value` conversion, normalization, and comparison behavior.

## 1. Scope

`Value` variants:

- Scalar/value domain: `Nil`, `Bool`, `U8`, `U16`, `U32`, `U64`, `U128`, `Bytes`, `Address`
- Special domain: `HeapSlice`, `Compo`

Unless explicitly stated, all conversion and comparison rules only apply to the scalar/value domain.

## 2. API Orthogonality

- `canbe_*`: convert to Rust/native type, does not mutate `self`
- `to_*`: convert to normalized `Value`, does not mutate `self`
- `cast_*`: explicit cast in place, mutates `self`

This separation is strict and must remain stable.

## 3. Canonical Normalization Primitives

## 3.1 Byte normalization (`extract_bytes_ec`)

Output type: `Vec<u8>`.

Implementation: `Value::extract_bytes_with_error_code` in `vm/src/value/canbe.rs`. Do not use `Value::scalar_bytes` for this path.

- `Bool` -> `[0x01]` or `[0x00]`
- `U8/U16/U32/U64/U128` -> fixed-width big-endian bytes (leading zero bytes preserved)
- `Bytes` -> raw bytes (including `Bytes([])`; empty bytes is a valid value, not absence)
- `Address` -> raw address bytes
- `Nil` -> error (typed absence; not equivalent to `Bytes([])`)
- `HeapSlice`, `Compo` -> error

Field serialization (`Value::serialize` / `scalar_bytes`) is separate: `Nil` encodes as type tag with zero payload and round-trips as `Nil`, not `Bytes([])`.

## 3.2 Bool normalization (`extract_bool`)

Output type: `bool`.

- `Bool(b)` -> `b`
- `Nil` -> `false`
- `U*` -> `n != 0`
- `Bytes` -> any non-zero byte => `true`, otherwise `false`
- `Address` -> any non-zero byte => `true`, otherwise `false`
- `HeapSlice`, `Compo` -> error

## 3.3 Stack arithmetic uint gate (`to_uint`)

Output type: `Value` (`U8/U16/U32/U64/U128`).

Implementation: `Value::to_uint` in `vm/src/value/convert.rs`.

Runtime **arithmetic / bit-op** paths (`ADD`, `BSHL`, `INC`, etc.) call this gate before width promotion. It is **strict**:

- `U8/U16/U32/U64/U128` -> keep current uint value/type
- All other types (`Nil`, `Bool`, `Bytes`, `Address`, `HeapSlice`, `Compo`, ...) -> error

Operands must already be uint variants or the instruction fails immediately. Use explicit `CU*` / `CTO U*` on the stack when a wider scalar domain is needed.

## 3.4 Explicit uint cast normalization (`to_u128`)

Output type: `u128`.

Used by explicit uint casts (`CU*`, `CTO U*`) when the source is not already `Bytes` with width-aware rules. This is **broader** than stack arithmetic `to_uint` (section 3.3):

- `U*` -> value widening to `u128`
- `Nil` -> `0`
- `Bool` -> `0|1`
- `Bytes` / `Address`:
  - drop leading zero bytes
  - remaining length `<=16` => parse as big-endian `u128`
  - remaining length `>16` => error
- `HeapSlice`, `Compo` -> error

## 4. Explicit Cast Rules (`cast_*` / `CU*` / `CTO`)

## 4.1 `cast_bool` / `CTO Bool`

- Uses `extract_bool`
- Same acceptance/failure set as section 3.2

## 4.2 `cast_u8/u16/u32/u64/u128` / `CU8..CU128`

For target width `W` bits (`N = W/8` bytes):

- If source is `Bytes`:
  - if `len <= N`: left-pad zeros to `N` and parse
  - if `len > N`: only allowed when truncated prefix bytes are all zero; otherwise error
- If source is not `Bytes`:
  - normalize by `to_u128`
  - check target range (`u8/u16/u32/u64/u128`)
  - overflow => error

Explicit uint cast uses section 3.4 (`to_u128`) plus width/range checks. Stack arithmetic uses section 3.3 (`to_uint`, uint-only). `Bytes` on explicit cast keeps width-aware cast semantics; `Bytes` does not implicitly enter stack arithmetic.

## 4.3 `cast_buf` / `CBYTES`

- Uses byte normalization from section 3.1 (`extract_bytes_ec`)
- `Nil`, `HeapSlice`, `Compo` are rejected
- `Bytes([])` is accepted (empty bytes is a value, not absence)

## 4.4 `cast_addr`

- First `cast_buf` / `cast_bytes()` (section 3.1 / 4.3)
- Then parse exact `Address` byte length via `Address::from_bytes`
- Length mismatch or invalid address bytes => error
- Implementation: `Value::cast_addr` in `vm/src/value/cast.rs`

## 4.5 `cast_to` / `CTO <type>`

- Dispatches to `cast_bool`, `cast_u*`, `cast_buf`, `cast_addr`
- `CTO` target type param must be in the explicit-cast set: `Bool/U8/U16/U32/U64/U128/Bytes/Address`
- `CTO` target outside this set (`Nil`, `HeapSlice`, `Compo`) => `InstParamsErr`
- `TIS` type param accepts all declared `ValueTy` (including `Nil/HeapSlice/Compo`) for type checking
- `TIS` unknown or reserved type id => `InstParamsErr`
- `CTO` unknown or reserved type id => `InstParamsErr`

## 5. Implicit Conversion Trigger Map

## 5.1 Bool context (implicit bool normalization)

Uses section 3.2 (`extract_bool`):

- Logic ops: `AND`, `OR`, `NOT`
- Control-flow condition: `CHOOSE`, `BRL`, `BRS`, `BRSL`, `BRSLN`, `AST`
- `CTO Bool`

## 5.2 Numeric arithmetic context (uint-only gate)

Uses section 3.3 (`to_uint`): **both operands must already be uint variants** (`U8`..`U128`), then width promotion (`cast_arithmetic`) to the wider uint type.

Applied by:

- Arithmetic: `ADD`, `SUB`, `MUL`, `DIV`, `MOD`, `POW`, `MAX`, `MIN`
- Bit ops: `BSHL`, `BSHR`, `BAND`, `BOR`, `BXOR`
- Unary numeric: `INC`, `DEC` (operand must already be uint; no implicit `to_uint` widening from `Nil`/`Bytes`)

`Nil`, `Bool`, `Bytes`, and `Address` do **not** silently enter this path. Cast with `CU*` / `CTO U*` first when needed.

Failure in either operand gate or width/range checks => immediate error.

## 5.3 Equality context (`EQ`, `NEQ`)

- If both operands are uint types (`U8/U16/U32/U64/U128`):
  - convert both to `u128`
  - compare numeric value
- Otherwise, operands must have the same runtime type; cross-type compare fails.
- Same-type compare rules:
  - `Nil == Nil`
  - `Bool` compares bool value
  - `Bytes` compares raw bytes
  - `Address` compares raw address bytes
  - `HeapSlice` compares `(start, len)`
  - `Compo` compares pointer identity (`ptr_eq`)

## 5.4 Ordered compare context (`LT`, `LE`, `GT`, `GE`)

- Ordered comparison is `uint`-only.
- Both operands must already be uint types: `U8/U16/U32/U64/U128`.
- No implicit `to_u128` conversion is applied for `Nil/Bool/Bytes/Address`.
- Operands are compared as numeric uint values.
- Any non-uint operand => immediate type error.

## 5.5 Byte-handle context

Uses section 3.1 (`extract_bytes_ec`):

- `CAT`, `JOIN`, `BYTE`, `CUT`, `LEFT`, `RIGHT`, `LDROP`, `RDROP`

`Nil`, `HeapSlice`, `Compo` are rejected in this path.

## 5.6 External/native call data context

Uses `extract_call_data`:

- `Nil` => `[]`
- `HeapSlice(start, len)` => read bytes from heap slice
- Otherwise => `extract_bytes_ec`

This is the only allowed conversion path where `HeapSlice` participates.

## 6. Function Param/Return Type Check Rules

## 6.1 Signature-level constraints (`ValueTy`)

- Param type cannot be `Nil`, `HeapSlice`, `Tuple`
- Return type cannot be `Nil`, `HeapSlice`

## 6.2 Runtime checked cast (`check_param_type` / `cast_param`)

Function boundaries are **stricter than stack-level explicit casts** (`CTO` / `CU*` / `CBYTES`), except for one heap bridge:

| Context | Allowed conversions |
|---------|---------------------|
| Stack `CTO` / `CU*` / `CBYTES` | Full explicit-cast family (sections 4.1–4.4) |
| Function param / return boundary | **`HeapSlice → Bytes` unconditionally first**; then identical type and uint-family narrowing/widening via `cast_param` |

`HeapSlice → Bytes` at function boundary (params and returns):

- Runs before shape/type checks via `materialize_boundary_heap_slices`.
- Reads bytes from the **current frame heap** (same source as `extract_call_data` for slices).
- Replaces the slot with owned `Bytes`.
- Validates materialized length against `SpaceCap.value_size`.
- Does **not** add gas (boundary check only).

Rules after materialization:

- If source and target are identical: pass
- Allowed uint casts: `U8/U16/U32/U64/U128` ↔ same family with fit check
- All other source/target combinations: `CallArgvTypeFail`

Note: `check_param_type(v, ty, heap, cap)` is the non-mutating wrapper (includes materialization). See `vm/doc/heapslice-func-boundary.md` and `vm/doc/call-standard.md` §16.

## 7. Deterministic Edge Cases (Normative)

- `U16(1) == U8(1)` is `true` (both uint; compare as `u128`)
- `U32(5) != U8(4)` is `true` (both uint; compare as `u128`)
- `Nil == Bool(false)` fails (cross-type compare is not allowed)
- `Bytes([0,1]) == U8(1)` fails (cross-type compare is not allowed)
- `Bytes([0,1]) == Bytes([1])` is `false` (same-type raw-bytes compare)
- Ordered compare with non-uint values (`Nil`, `Bool`, `Bytes`, `Address`, `HeapSlice`, `Compo`) always fails
- `Nil` and `Bool(false)` are both falsy in `extract_bool` / `CHOOSE`, but `Nil == Bool(false)` fails (cross-type `EQ`)

## 8. Compo Map Semantics (Normative)

Map duplicate-key policy is classified by operation intent:

| Intent | Operation | Duplicate key behavior |
|--------|-----------|------------------------|
| **Materialize** (multi-key commit) | `PACKMAP` / `pack_map`, `map { ... }` literal | **Reject** with `CompoPackError` |
| **Materialize** (multi-key commit) | `MERGE` / `merge` (map + map) | **Reject** with `CompoPackError` if source key already exists in destination |
| **Upsert** (single-key write) | `INSERT` / `insert` on map | **Overwrite** value for existing key |

Multi-key commits (`PACKMAP`, `MERGE`) require disjoint key sets: duplicate keys are contract errors and must fail as an unrecoverable **Fault** (`CompoPackError` → `XError::Fault`), not a recoverable Revert. Single-key upserts (`INSERT`) model ledger field updates and intentionally overwrite.

List `MERGE` appends elements; it has no key domain and does not deduplicate elements.

## 9. U256 Reservation Contract (Forward Compatibility)

This section defines mandatory constraints before enabling `U256`, to prevent compatibility breaks and normalization drift.

- Type id `7` and keyword `u256` are reserved and currently disabled.
- Any decode/build path receiving type id `7` must fail explicitly (not silently downgraded).
- Any type parser receiving `u256` must fail explicitly with a reserved/not-enabled error.
- Activation must be version-gated (e.g. codeconf/height fork switch), not implicit by parser/runtime fallback.
- `U256` activation is all-or-nothing for conversion paths:
  - update scalar domain (`Value`, `ValueTy`, serialization/parse)
  - update explicit cast family (`cast_u256`, `CU256`, `CTO U256`)
  - update implicit numeric normalization (`to_uint`, arithmetic width promotion, ordered compare)
  - update equality uint numeric-compare path to include `U256` as uint operand
- While `U256` is disabled, numeric scalar normalization remains bounded by current active max width (`u128` / 16 bytes).
- No mixed mode is allowed: enabling parser keyword without runtime normalization support is forbidden.

# AST Cost Model (Ctx-Gas + Child Return-Gas)

## 1. Scope

This document defines the current AST gas model implemented by:

1. `ast_try_item!` in `protocol/src/action/asthelper.rs`
2. `AstSelect` in `protocol/src/action/astselect.rs`
3. `AstIf` in `protocol/src/action/astif.rs`
4. snapshot helpers in `protocol/src/context/sub.rs`

Current model summary:

1. all actual charging is reflected in `ctx.gas_remaining()`
2. `AstSelect`/`AstIf` always return gas `0`
3. each AST item attempt charges fixed snapshot try cost (`40`)
4. on child success, AST charges child returned gas (`u32`) via `ctx.gas_consume`
5. child internal runtime gas (including VM runtime gas) is charged by child logic itself

---

## 2. Core Invariants

1. Gas monotonicity:
   rollback never refunds gas.
2. Control-flow node return gas:
   `AstSelect` and `AstIf` returned gas is always `0`.
3. Child return-gas handling:
   only `ast_try_item!` consumes child returned gas in AST path.
4. Item snapshot policy:
   every child attempt goes through `ast_item_snapshot(...)` (`+40`).
5. Top-level tx policy:
   transaction main loop still discards action returned gas.

---

## 3. Execution Semantics

### 3.1 AstSelect

For each attempted child action:

1. `ast_item_snapshot(ctx)`:
   consume fixed `40` gas, then snapshot state/vm/log/context volatile.
2. execute child action (`act.execute(ctx)`).
3. success path:
   - `ctx.gas_consume(child_gas)` where `child_gas` is child's returned `u32`
   - merge snapshot.
4. failure path:
   - recover snapshot.
   - `Unwind` means "skip and continue".
   - `Interrupt` means abort current node.

`AstSelect` returns `(0, last_success_ret_or_empty)`.

### 3.2 AstIf

Execution order:

1. create whole-node snapshot (`AstNodeTxn`) for if-node rollback.
2. cond attempt via `ast_try_item!(ctx, self.cond.execute(ctx))`.
3. select branch (`br_if` or `br_else`).
4. branch attempt via `ast_try_item!(ctx, branch.execute(ctx))`.
5. finalize node snapshot:
   - success => merge
   - failure => recover

`AstIf` returns `(0, branch_ret_bytes)`.

---

## 4. Gas Consumption Inventory

AST path gas consists of the following channels:

1. **snapshot try cost**
   - source: `ast_item_snapshot(...)`
   - amount: `40` per item attempt
   - refunded on rollback: no

2. **child returned gas (size/channel gas)**
   - source: child `execute()` returned `u32`
   - charged at: `ast_try_item!` success path (`ctx.gas_consume(child_gas)`)
   - refunded on rollback: no

3. **child internal runtime gas**
   - source: any `ctx.gas_consume(...)` done during child execution
   - includes VM opcode/runtime/min-call and other dynamic runtime charges
   - refunded on rollback: no

4. **whole-node snapshot operations (`AstNodeTxn`)**
   - source: `ctx_snapshot`/`ctx_merge`/`ctx_recover`
   - fixed gas: none
   - effect: state/log/vm-volatile rollback only, not gas rollback

5. **AST control-flow node static size gas**
   - `AstSelect` / `AstIf` local `gas` is explicitly set to `0`
   - so control-flow node size itself does not flow through returned-gas channel

---

## 5. Formulas

### 5.1 Generic AST Item Attempt

For one attempt `i`:

`G_attempt_i = 40 + G_run_i + I_success_i * G_ret_i`

where:

1. `40` is snapshot try cost
2. `G_run_i` is child runtime gas charged during execution
3. `G_ret_i` is child returned gas (`u32`)
4. `I_success_i` is `1` on success, `0` on failure

### 5.2 AstSelect Node

For `N` attempted children:

`G_select = sum_{i=1..N} (40 + G_run_i + I_success_i * G_ret_i)`

`AstSelect` returned gas is always `0`.

### 5.3 AstIf Node

Let cond attempt be `c`, branch attempt be `b`:

`G_if = (40 + G_run_c + I_success_c * G_ret_c) + (40 + G_run_b + I_success_b * G_ret_b)`

`AstIf` returned gas is always `0`.

Note:

1. when `cond`/`branch` is itself an AST node, its own returned gas is `0`, but its internal child attempts still charge normally.

---

## 6. Worked Examples

All numbers below are examples for auditing and reasoning.

### 6.1 Nested AST + Plain Actions

Structure:

1. outer `AstSelect` (min=2, max=2)
2. child A: plain action, returned gas `30`, runtime gas `0`
3. child B: inner `AstSelect` (min=2, max=2)
4. inner child B1: plain action, returned gas `20`, runtime gas `0`
5. inner child B2: plain action, returned gas `10`, runtime gas `0`

Charge breakdown:

1. outer child A attempt: `40 + 0 + 30 = 70`
2. outer child B attempt (inner select node): `40 + 0 + 0 = 40`
3. inner child B1 attempt: `40 + 0 + 20 = 60`
4. inner child B2 attempt: `40 + 0 + 10 = 50`

Total:

`70 + 40 + 60 + 50 = 220`

### 6.2 Nested AST + VM Call

Structure:

1. outer `AstSelect` has two children:
   - child A: plain action, returned gas `30`, runtime gas `0`
   - child B: `AstIf`
2. `AstIf.cond`: one-child `AstSelect`, inner plain action returned gas `12`, runtime gas `0`
3. selected branch: one-child `AstSelect`, inner `ContractMainCall`
   - returned gas (action-size channel) `52`
   - VM runtime dynamic charge `480`

Charge breakdown:

1. outer child A attempt: `40 + 0 + 30 = 70`
2. outer child B (`AstIf`) attempt at outer level: `40 + 0 + 0 = 40`
3. `AstIf` cond wrapper attempt: `40 + 0 + 0 = 40`
4. cond inner child attempt: `40 + 0 + 12 = 52`
5. `AstIf` branch wrapper attempt: `40 + 0 + 0 = 40`
6. branch inner VM action attempt: `40 + 480 + 52 = 572`

Total:

`70 + 40 + 40 + 52 + 40 + 572 = 814`

Interpretation:

1. VM runtime `480` is charged by VM runtime path directly in shared context gas.
2. VM action returned gas (`52`) is charged once at AST item success.
3. no extra "VM dynamic gas through returned-gas channel" double charge.

---

## 7. Error and Recovery

1. `ast_try_item!` recover failure message keeps both recover error and original child error.
2. `AstNodeTxn::finish` recover failure message keeps both recover error and original node error.
3. `Unwind` is recoverable control flow in AST select/if policy; `Interrupt` is non-recoverable for current node.

---

## 8. Settlement Notes

1. tx settlement still uses context gas usage and normal refund/burn flow.
2. top-level tx loop still ignores returned gas from actions.
3. AST path charging correctness should always be validated against `ctx.gas_remaining()` deltas, not returned gas of `AstSelect`/`AstIf`.

---

## 9. One-Line Summary

AST charging is now: `snapshot try cost + child runtime gas + child returned gas(on success)`, all paid through context gas; control-flow node returned gas stays `0`.

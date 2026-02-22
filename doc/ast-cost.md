# AST / VM Return-Gas Cost Model

## 1. Overall Goals

This specification defines a unified cost model for combined AST and VM execution, with the following goals:

1. Historical compatibility: do not change top-level transaction charging behavior before AST/VM introduction.
2. Verifiability: every cost category has a clear ownership and settlement path, directly convertible into test assertions.
3. Anti-abuse: failed branches, rolled-back branches, and repeated probing must still pay cost, preventing “free branch search”.
4. No double charge: overlapping shared dynamic consumption and returned gas must not be charged twice.

---

## 2. Charging Architecture (Four Layers)

### A. Transaction-Level Cost (HAC)

- `tx.fee`: fixed transaction fee, settled on the transaction success path.
- `gas_max` budget: enabled only for gas-enabled transactions (AST transactions require `gas_max > 0`), pre-charged at max and refunded by actual usage.

### B. Static Size Cost (returned domain)

- Every action execution produces returned gas (base comes from action size plus action-internal aggregation).
- Returned gas itself is not an automatic burn signal; the caller decides whether and how to fold it into shared budget.

### C. Dynamic Execution Cost (shared domain)

- VM opcodes, storage, per-call minimum cost, snapshot try cost, etc. are deducted directly from the shared gas budget.
- Shared is a transaction-global monotonic counter and is not restored by branch rollback.

### D. Snapshot / Recovery Cost (AST-specific)

- AST per-item execution attempts have fixed snapshot try cost (current policy: fixed charge per attempt).
- AST whole-node savepoint is used only as a consistency rollback boundary and does not add extra whole-snapshot fee.

---

## 3. Top-Level Transaction Loop Rule (Compatibility Critical)

- In top-level `for action in tx.actions`, action returned gas is intentionally discarded.
- Consequence: top-level regular actions do not charge extra through returned gas.
- Meaning: returned gas semantics are active only in AST internal nesting and VM `EXTACTION` path.

This rule preserves historical block compatibility and avoids retroactively applying new returned-gas semantics to legacy execution.

---

## 4. AST Charging Standard

## 4.1 AstSelect

For each attempted child action:

1. Charge one “attempt snapshot fee” (shared deduction).
2. Execute child action and measure shared consumption during child execution: `shared_child`.
3. Read child returned gas `ret_child`, and only supplement the non-shared portion:
   - `extra_child = max(ret_child - shared_child, 0)`.
4. Node-level accumulated cost increases by:
   - `snapshot_try + shared_child + extra_child`.

If a child fails, state is rolled back but consumed gas is not rolled back; if a child succeeds, state is merged.

## 4.2 AstIf

- Execute cond branch first (including cond snapshot try fee), then execute if/else branch according to cond result.
- Branch charging uses the same `shared + extra(max(ret-shared,0))` aggregation.
- If final branch fails, roll back whole-node state; gas consumption remains monotonic.

## 4.3 AST Core Invariants

- Rollback reverts state only, not gas.
- For one child execution, shared part is counted once; extra only supplements the uncovered returned part.
- Whole savepoint is not charged separately and does not overlap with item snapshot try fee.

---

## 5. VM / EXTACTION Charging Standard

### 5.1 VM Dynamic Cost

- VM runs on shared budget; opcode-level and storage-level costs are directly deducted from shared.
- Every VM call has a minimum-call-cost floor; if actual cost is lower, shortfall is additionally deducted, ensuring each call has a minimum cost.

### 5.2 EXTACTION (VM -> action)

- `EXTACTION` invokes protocol action and receives action returned gas.
- This returned gas is included in VM-side call cost and finally reflected as shared deduction.
- Therefore EXTACTION is one returned-gas active path (the other is AST internal nested aggregation).

---

## 6. Double-Charge vs De-dup Rules

## 6.1 Explicitly Repeated Charges (by attempt / call count)

1. AST child attempt snapshot fee: charged once per attempt (charged for both success and failure).
2. Multiple AST branch attempts: each actual child execution is charged independently.
3. VM minimum call cost: applied independently per VM call.

> These repetitions are intentional policy behavior, not accounting bugs, and are used to resist probing attacks.

## 6.2 Explicit De-dup (avoid double charge)

1. AST child aggregation uses `extra = max(ret - shared, 0)`, so shared-returned overlap is not charged twice.
2. Top-level transaction loop discards returned gas, preventing returned from being layered again at top-level shared settlement.

## 6.3 Explicitly Not De-duplicated (business-intended additive charging)

1. Static size costs at different levels/nodes are additive whenever execution happens.
2. Shared consumption produced by failed branches is not refunded or offset.

---

## 7. User Cost View

1. User-visible cap: defined by `gas_max`, pre-charged at cap and refunded by actual use.
2. Actual user payment: fixed `tx.fee` + actual gas burn (used shared mapped to HAC).
3. Higher AST complexity (more branches, more rollbacks) implies higher probing cost.
4. High-frequency/deep VM calls are constrained by both minimum call cost and dynamic gas.

---

## 8. Anti-Attack Design Intent

1. No free probing: failed branches do not refund gas.
2. Branch explosion control: fixed AST snapshot try cost per attempt.
3. Low-cost call spam resistance: VM minimum-call-cost floor per call.
4. Accounting double-charge prevention: AST internal `shared + extra` model de-duplicates overlap.
5. Historical replay/fork safety: top-level keeps discarding returned gas to preserve legacy semantics.

---

## 9. Test Checklist (Recommended Assertion Template)

1. When top-level action returned gas changes, non-AST top-level transaction charging must remain unchanged.
2. After AST child failure, state must roll back while gas remaining must decrease.
3. AST node returned gas must satisfy:
   - `node_ret = Σ(snapshot_try + shared + extra)`.
4. For one child execution, `extra` must never be negative; shared must not be double-counted.
5. Whole savepoint must not introduce extra fixed snapshot charging.
6. VM call must satisfy minimum-call-cost floor even for very short execution.
7. Returned gas from EXTACTION must enter VM cost and be reflected in shared deduction.
8. Gas settlement must satisfy: max pre-charge - used charge = refund, and refund must never be negative.

---

## 10. One-Line Summary

- Top-level discards returned; AST/EXTACTION use returned; dynamic cost goes through shared; AST uses `shared + max(ret-shared,0)` to avoid double charge; failed branches roll back state but never roll back gas.

---

## 11. AST Item Cost Composition (Formal)

For each attempted AST child item, define:

- `S_snap`: shared gas spent by `ast_item_snapshot`.
- `S_exec`: shared gas spent while executing the child action.
- `R_child`: child returned gas from `action.execute`.
- `E_child`: non-overlap supplement, `E_child = max(R_child - S_exec, 0)`.

Then the AST node-level increment contributed by this child is:

- `C_child = S_snap + S_exec + E_child`.

Equivalent piecewise form:

- If `R_child >= S_exec`, then `C_child = S_snap + R_child`.
- If `R_child < S_exec`, then `C_child = S_snap + S_exec`.

This is exactly what `AstSelect`/`AstIf` implement by adding shared delta first, then adding only `max(ret-shared,0)`.

## 11.1 Why this has no double-charge bug

For one child execution, overlap between returned and shared is `min(R_child, S_exec)`.

- Without de-dup, naive sum would be `S_exec + R_child`, double-counting the overlap.
- Current model uses `S_exec + max(R_child-S_exec,0)`.

So effective counted part is:

- shared-covered part counted once,
- returned-only tail counted once,
- no negative tail when `R_child < S_exec`.

Therefore the model is both:

1. No over-charge (no overlap counted twice).
2. No under-charge (returned-only part is still charged).

## 11.2 Failure path consistency

- Child failure still consumes `S_snap` and already-burned shared gas.
- State rollback does not imply gas rollback.
- This prevents free probing and keeps branch-search costly.

---

## 12. Static Size Repeated Charge Semantics

Static size cost is additive by execution count, not by semantic deduplication.

If the same action type is executed twice in two AST attempts/calls, its static size contribution is charged twice because two independent executions occurred.

This is intentional and belongs to anti-probing economics.

## 12.1 Practical check pattern

For two scenarios where the only extra operation is one more successful action execution:

- `Δret = ret_case2 - ret_case1`
- `Δshared = shared_case2 - shared_case1`

Then for a pure static-size child (no extra shared dynamic behavior inside action body):

- `Δret - Δshared = child_static_size`

This pattern proves static size is charged once per extra execution and is not hidden by shared snapshot overhead.

## 12.2 Test mapping

- De-dup invariant (`max(ret-shared,0)`): assert no doubling when child both consumes shared and reports returned.
- Non-negative supplement clamp: when `shared > returned`, AST must not subtract or underflow.
- Repeated size additive: extra identical action execution adds exactly one more static size unit.

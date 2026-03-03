# VM Not Bug Note

1. `BUG 2 | M | vm/src/space/kvmap.rs:82`
   This is not a bug because one transaction has an upper bound on how many contracts it can reach, so the number of per-contract memory buckets is naturally bounded by transaction-level reachability constraints.

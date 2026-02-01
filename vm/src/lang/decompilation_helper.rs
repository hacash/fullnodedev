use super::ir::IRNodeArray;
use super::rt::Bytecode;

/// Encapsulates PrintOption-driven tweaks applied during decompilation.
pub struct DecompilationHelper<'a> {
    opt: &'a PrintOption<'a>,
}

impl<'a> DecompilationHelper<'a> {
    pub fn new(opt: &'a PrintOption<'a>) -> Self {
        Self { opt }
    }

    pub fn block_prefix(&self, arr: &IRNodeArray) -> String {
        let _ = arr;
        // NOTE: lib prelude emission is handled explicitly at the formatter top-level
        // (see `Formater::print`). Keeping this empty avoids injecting file-level
        // declarations into nested/inline contexts.
        String::new()
    }

    pub fn should_trim_root_block(&self, arr: &IRNodeArray) -> bool {
        use Bytecode::*;
        self.opt.trim_root_block && self.opt.tab == 0 && arr.inst == IRBLOCK
    }

    pub fn prepare_root_block(&self, arr: &IRNodeArray) -> (usize, Option<String>) {
        use Bytecode::*;
        // Skip leading empty placeholders (they serialize to nothing but may exist
        // in the IR array to reserve positions for later ALLOC injection).
        let mut first_real_idx: usize = 0;
        while first_real_idx < arr.subs.len()
            && arr.subs[first_real_idx]
                .as_any()
                .downcast_ref::<super::ir::IRNodeEmpty>()
                .is_some()
        {
            first_real_idx += 1;
        }

        // Locate alloc index if present (alloc could be at first_real_idx or first_real_idx+1
        // due to placeholder insertion patterns).
        let mut alloc_index: Option<usize> = None;
        for (i, s) in arr
            .subs
            .iter()
            .enumerate()
            .skip(first_real_idx)
            .take(2)
        {
            if s.bytecode() == ALLOC as u8 {
                alloc_index = Some(i);
                break;
            }
        }

        if self.opt.trim_param_unpack {
            let param_idx = match alloc_index {
                Some(ai) => ai + 1,
                None => first_real_idx,
            };
            if let Some(line) = self.build_param_line(arr, param_idx) {
                if let Some(names) = self.infer_param_names(arr, param_idx) {
                    for i in 0..names.len() as u8 {
                        self.opt.mark_slot_put(i);
                    }
                }
                return (param_idx + 1, Some(line));
            }
        }

        if self.opt.trim_head_alloc {
            if let Some(ai) = alloc_index {
                return (ai + 1, None);
            }
        }

        (first_real_idx, None)
    }

    /// If the block contains a canonical param-unpack node at `start_idx`, return the
    /// corresponding `param { ... }` source line.
    pub fn try_build_param_line(&self, arr: &IRNodeArray, start_idx: usize) -> Option<String> {
        self.build_param_line(arr, start_idx)
    }

    /// Infer parameter names for a canonical param-unpack node.
    /// - If SourceMap has parameter names, use them.
    /// - Otherwise, generate placeholder `$0, $1, ...` with an inferred count.
    ///
    /// Returns `None` if the node at `start_idx` is not the canonical param-unpack form.
    pub fn infer_param_names(&self, arr: &IRNodeArray, start_idx: usize) -> Option<Vec<String>> {
        use Bytecode::*;
        if start_idx >= arr.subs.len() {
            return None;
        }
        let node = &arr.subs[start_idx];
        // Canonical param-unpack IR form for stable roundtrip: UPLIST(PICK0, P0)
        let is_param_unpack = if let Some(double) = node.as_any().downcast_ref::<IRNodeDouble>() {
            if double.inst != UPLIST {
                false
            } else {
                let subx_is_pick0 = double
                    .subx
                    .as_any()
                    .downcast_ref::<IRNodeLeaf>()
                    .is_some_and(|leaf| leaf.inst == PICK0);
                let suby_is_p0 = double
                    .suby
                    .as_any()
                    .downcast_ref::<IRNodeLeaf>()
                    .is_some_and(|leaf| leaf.inst == P0);
                subx_is_pick0 && suby_is_p0
            }
        } else {
            false
        };
        if !is_param_unpack {
            return None;
        }

        if let Some(names) = self
            .opt
            .map
            .and_then(|m| m.param_names().cloned())
            .filter(|n| !n.is_empty())
        {
            return Some(names);
        }

        // Fallback: infer a param count without SourceMap.
        // We must avoid binding non-param locals (which would conflict with later `var $i $i = ...`).
        // Heuristic:
        // - Read total alloc slots from the nearest preceding ALLOC.
        // - Look for an early PUT to a slot >= 1 shortly after UPLIST; its slot index
        //   typically equals the first non-param local slot, so it approximates param count.
        // - If no such early PUT exists, fall back to alloc_count (or 1).
        let mut alloc_count: usize = 0;
        if start_idx > 0 {
            for i in start_idx.saturating_sub(2)..start_idx {
                if let Some(p1) = arr.subs[i].as_any().downcast_ref::<IRNodeParam1>() {
                    if p1.inst == ALLOC {
                        alloc_count = p1.para as usize;
                        break;
                    }
                }
            }
        }
        let mut first_local_slot: Option<usize> = None;
        let max_scan = 32usize;
        for s in arr.subs.iter().skip(start_idx + 1).take(max_scan) {
            if let Some(p1s) = s.as_any().downcast_ref::<IRNodeParam1Single>() {
                if p1s.inst == PUT {
                    let slot = p1s.para as usize;
                    if slot > 0 {
                        first_local_slot = Some(first_local_slot.map_or(slot, |cur| cur.min(slot)));
                    }
                }
            }
        }

        let mut count = first_local_slot.unwrap_or(alloc_count.max(1));
        if alloc_count > 0 {
            count = count.min(alloc_count);
        }
        if count == 0 {
            count = 1;
        }

        Some((0..count).map(|i| format!("${}", i)).collect())
    }

    fn build_param_line(&self, arr: &IRNodeArray, start_idx: usize) -> Option<String> {
        let names = self.infer_param_names(arr, start_idx)?;
        let indent = self.opt.indent.repeat(self.opt.tab);
        let params = names.join(", ");
        Some(format!("{}param {{ {} }}", indent, params))
    }

    pub fn should_flatten_syscall_cat(&self) -> bool {
        self.opt.flatten_syscall_cat
    }
}

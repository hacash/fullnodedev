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
        use Bytecode::*;
        if self.opt.tab != 0 || arr.inst != IRBLOCK {
            return String::new();
        }
        let mut prefix = String::new();
        if let Some(map) = self.opt.map {
            for (idx, info) in map.lib_entries() {
                let line = match &info.address {
                    Some(addr) => format!("lib {} = {}: {}\n", info.name, idx, addr.readable()),
                    None => format!("lib {} = {}\n", info.name, idx),
                };
                prefix.push_str(&line);
            }
        }
        prefix
    }

    pub fn should_trim_root_block(&self, arr: &IRNodeArray) -> bool {
        use Bytecode::*;
        self.opt.trim_root_block && self.opt.tab == 0 && arr.inst == IRBLOCK
    }

    pub fn prepare_root_block(&self, arr: &IRNodeArray) -> (usize, Option<String>) {
        use Bytecode::*;
        // locate alloc index if present (alloc could be at index 0 or 1 due to placeholder)
        let mut alloc_index: Option<usize> = None;
        for (i, s) in arr.subs.iter().enumerate().take(2) {
            if s.bytecode() == ALLOC as u8 {
                alloc_index = Some(i);
                break;
            }
        }

        if self.opt.trim_param_unpack {
            let param_idx = match alloc_index {
                Some(ai) => ai + 1,
                None => 0,
            };
            if let Some(line) = self.build_param_line(arr, param_idx) {
                return (param_idx + 1, Some(line));
            }
        }

        if self.opt.trim_head_alloc {
            if let Some(ai) = alloc_index {
                return (ai + 1, None);
            }
        }

        (0, None)
    }

    fn build_param_line(&self, arr: &IRNodeArray, start_idx: usize) -> Option<String> {
        use Bytecode::*;
        if start_idx >= arr.subs.len() {
            return None;
        }
        let map = self.opt.map?;
        let names = map.param_names()?;
        let double = arr.subs[start_idx].as_any().downcast_ref::<IRNodeDouble>()?;
        if double.inst != UPLIST {
            return None;
        }
        let indent = self.opt.indent.repeat(self.opt.tab);
        let params = names.join(", ");
        Some(format!("{}param {{ {} }}", indent, params))
    }

    pub fn should_flatten_syscall_cat(&self) -> bool {
        self.opt.flatten_syscall_cat
    }
}

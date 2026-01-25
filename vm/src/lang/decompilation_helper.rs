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
        let mut start_idx = if self.opt.trim_head_alloc {
            arr.subs
                .first()
                .map_or(0, |first| if first.bytecode() == ALLOC as u8 { 1 } else { 0 })
        } else {
            0
        };
        if self.opt.trim_param_unpack {
            if let Some(line) = self.build_param_line(arr, start_idx) {
                start_idx += 1;
                return (start_idx, Some(line));
            }
        }
        (start_idx, None)
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
}

use std::cell::RefCell;
use std::collections::HashSet as PrintHashSet;
use std::rc::Rc;

use crate::rt::SourceMap;

#[derive(Clone)]
pub struct PrintOption<'a> {
    pub indent: &'a str,
    pub tab: usize,
    pub map: Option<&'a SourceMap>,
    pub trim_root_block: bool,
    pub trim_head_alloc: bool,
    pub trim_param_unpack: bool,
    pub hide_func_nil_argv: bool,
    pub call_short_syntax: bool,
    pub flatten_call_packlist: bool,
    pub flatten_array_packlist: bool,
    pub flatten_syscall_cat: bool,
    allocated: Rc<RefCell<PrintHashSet<u8>>>,
}

impl<'a> PrintOption<'a> {
    pub fn new(indent: &'a str, tab: usize) -> Self {
        Self {
            indent,
            tab,
            map: None,
            trim_root_block: false,
            trim_head_alloc: false,
            trim_param_unpack: false,
            hide_func_nil_argv: false,
            call_short_syntax: false,
            flatten_call_packlist: true,
            flatten_array_packlist: true,
            flatten_syscall_cat: true,
            allocated: Rc::new(RefCell::new(PrintHashSet::new())),
        }
    }

    pub fn with_trim_root_block(mut self, trim: bool) -> Self {
        self.trim_root_block = trim;
        self
    }

    pub fn with_trim_head_alloc(mut self, trim: bool) -> Self {
        self.trim_head_alloc = trim;
        self
    }

    pub fn with_source_map(mut self, map: &'a SourceMap) -> Self {
        self.map = Some(map);
        self
    }

    pub fn with_trim_param_unpack(mut self, trim: bool) -> Self {
        self.trim_param_unpack = trim;
        self
    }

    pub fn with_hide_func_nil_argv(mut self, hide: bool) -> Self {
        self.hide_func_nil_argv = hide;
        self
    }

    pub fn with_call_short_syntax(mut self, enable: bool) -> Self {
        self.call_short_syntax = enable;
        self
    }

    pub fn with_flatten_call_packlist(mut self, flatten: bool) -> Self {
        self.flatten_call_packlist = flatten;
        self
    }

    pub fn with_flatten_array_packlist(mut self, flatten: bool) -> Self {
        self.flatten_array_packlist = flatten;
        self
    }

    pub fn with_flatten_syscall_cat(mut self, flatten: bool) -> Self {
        self.flatten_syscall_cat = flatten;
        self
    }

    pub fn with_tab(&self, tab: usize) -> Self {
        let mut next = self.clone();
        next.tab = tab;
        next
    }

    pub fn child(&self) -> Self {
        self.with_tab(self.tab + 1)
    }

    pub fn mark_slot_put(&self, slot: u8) -> bool {
        self.allocated.borrow_mut().insert(slot)
    }

    pub fn clear_slot_put(&self, slot: u8) {
        self.allocated.borrow_mut().remove(&slot);
    }

    pub fn clear_all_slot_puts(&self) {
        self.allocated.borrow_mut().clear();
    }
}

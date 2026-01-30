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
    pub flatten_call_list: bool,
    pub flatten_array_list: bool,
    pub flatten_syscall_cat: bool,
    pub recover_literals: bool,
    allocated: Rc<RefCell<PrintHashSet<u8>>>,
    printed_consts: Rc<RefCell<PrintHashSet<String>>>,
    pending_consts: Rc<RefCell<Vec<String>>>,
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
            flatten_call_list: false,
            flatten_array_list: false,
            flatten_syscall_cat: false,
            recover_literals: false,
            allocated: Rc::new(RefCell::new(PrintHashSet::new())),
            printed_consts: Rc::new(RefCell::new(PrintHashSet::new())),
            pending_consts: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn with_source_map(mut self, map: &'a SourceMap) -> Self {
        self.map = Some(map);
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

    pub fn mark_const_printed(&self, name: String) -> bool {
        self.printed_consts.borrow_mut().insert(name)
    }

    pub fn is_const_printed(&self, name: &str) -> bool {
        self.printed_consts.borrow_mut().contains(name)
    }

    pub fn add_pending_const(&self, name: String) {
        let mut pending = self.pending_consts.borrow_mut();
        if !pending.contains(&name) {
            pending.push(name);
        }
    }

    pub fn take_pending_consts(&self) -> Vec<String> {
        let mut pending = self.pending_consts.borrow_mut();
        std::mem::take(&mut *pending)
    }
}

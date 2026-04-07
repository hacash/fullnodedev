pub fn register_protocol_extensions(setup: &mut protocol::setup::ProtocolSetup) {
    crate::action::register(setup);
    setup.action_hook(crate::hook::try_action_hook);
    setup.set_vm_assigner(|height| Box::new(crate::global_runtime_pool().checkout(height)));
}

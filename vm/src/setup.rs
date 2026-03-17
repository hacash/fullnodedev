use protocol::setup::SetupBuilder;

pub fn extend_standard_vm_stack(builder: SetupBuilder) -> SetupBuilder {
    builder
        .action_register(crate::action::register)
        .action_hooker(crate::hook::try_action_hook)
        .vm_assigner(|height| Box::new(crate::global_machine_manager().assign(height)))
}

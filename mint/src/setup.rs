use protocol::setup::SetupBuilder;

pub fn extend_standard_mint_stack(builder: SetupBuilder) -> SetupBuilder {
    builder.action_register(crate::action::register)
}

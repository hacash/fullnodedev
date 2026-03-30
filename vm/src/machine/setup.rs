pub fn setup_vm_runtime_gascap(ctx: &mut dyn Context, height: u64) -> (GasExtra, SpaceCap) {
    let Some(conf) = ctx.vm_runtime_config() else {
        return (GasExtra::new(height), SpaceCap::new(height));
    };
    let Ok(conf) = conf.downcast::<(GasExtra, SpaceCap)>() else {
        return (GasExtra::new(height), SpaceCap::new(height));
    };
    *conf
}

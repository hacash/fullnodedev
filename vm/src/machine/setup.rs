pub fn peek_vm_runtime_limits(ctx: &mut dyn Context, height: u64) -> (GasExtra, SpaceCap) {
    let Some(conf) = ctx.vm_runtime_config() else {
        return (GasExtra::new(height), SpaceCap::new(height));
    };
    let conf = conf
        .downcast::<(GasExtra, SpaceCap)>()
        .expect("vm runtime config type mismatch");
    let (gst, mut cap) = *conf;
    cap.normalize_zero_storage_period(height);
    (gst, cap)
}


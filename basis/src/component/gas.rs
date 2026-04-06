#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
/// VM runtime gas buckets used for metering/limits/reporting inside the VM.
///
/// This is NOT the source of truth for protocol-side billing.
/// Final HAC burn/refund is derived from `protocol::context::GasCounter`
/// via host/context gas charges already applied during execution.
pub struct VmGasBuckets {
    pub compute: i64,
    pub resource: i64,
    pub storage: i64,
}

impl VmGasBuckets {
    #[inline(always)]
    pub fn total(&self) -> i64 {
        self.compute
            .saturating_add(self.resource)
            .saturating_add(self.storage)
    }

    #[inline(always)]
    pub fn checked_total(&self) -> Option<i64> {
        self.compute
            .checked_add(self.resource)?
            .checked_add(self.storage)
    }

    #[inline(always)]
    pub fn checked_add(&self, more: &VmGasBuckets) -> Option<VmGasBuckets> {
        Some(VmGasBuckets {
            compute: self.compute.checked_add(more.compute)?,
            resource: self.resource.checked_add(more.resource)?,
            storage: self.storage.checked_add(more.storage)?,
        })
    }

    #[inline(always)]
    pub fn checked_sub(self, base: VmGasBuckets) -> Option<VmGasBuckets> {
        Some(VmGasBuckets {
            compute: self.compute.checked_sub(base.compute)?,
            resource: self.resource.checked_sub(base.resource)?,
            storage: self.storage.checked_sub(base.storage)?,
        })
    }
}

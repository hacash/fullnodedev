#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GasUse {
    pub compute: i64,
    pub resource: i64,
    pub storage: i64,
}

impl GasUse {
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
    pub fn checked_add(&self, more: &GasUse) -> Option<GasUse> {
        Some(GasUse {
            compute: self.compute.checked_add(more.compute)?,
            resource: self.resource.checked_add(more.resource)?,
            storage: self.storage.checked_add(more.storage)?,
        })
    }

    #[inline(always)]
    pub fn checked_sub(self, base: GasUse) -> Option<GasUse> {
        Some(GasUse {
            compute: self.compute.checked_sub(base.compute)?,
            resource: self.resource.checked_sub(base.resource)?,
            storage: self.storage.checked_sub(base.storage)?,
        })
    }
}

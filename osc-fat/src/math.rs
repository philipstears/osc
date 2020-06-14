// TODO: use https://docs.rs/num-integer? it is probably slower though because
// it is more general
pub(crate) trait DivCeiling {
    type Value;

    fn div_ceiling(self, divisor: Self::Value) -> Self::Value;
}

impl DivCeiling for u32 {
    type Value = Self;

    #[inline]
    fn div_ceiling(self, divisor: Self::Value) -> Self::Value {
        (self + (divisor - 1)) / divisor
    }
}

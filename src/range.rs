use std::{ops::{Range, RangeInclusive, RangeTo, RangeFrom, Add}, fmt::Display};

pub trait Num: Sized + Add<Self, Output = Self> {
    fn min() -> Self;
    fn max() -> Self;
    fn one() -> Self;
}

macro_rules! impl_num_bounds {
    ($T:ty) => {
        impl Num for $T {
            fn min() -> Self { <$T>::MIN }
            fn max() -> Self { <$T>::MAX }
            fn one() -> Self { 1 }
        }
    }
}

impl_num_bounds!(u16);

/// Range type that can represent `Range`, `RangeInclusive`, `RangeTo`, and
/// `RangeFrom` values.
/// The half-open ranges use `Idx::MIN` or `Idx::MAX` for the missing end.
/// The internal range uses an exclusive end so `Idx::MAX` is an invalid end
/// value for creation from `RangeInclusive`.
pub struct AnyRange<Idx> {
    pub start: Idx,
    pub end: Idx,
}

impl<Idx: Copy + PartialOrd> AnyRange<Idx> {
    /// Clamps the given value into the range
    pub fn clamp(&self, x: Idx) -> Idx {
        if x < self.start {
            self.start
        } else if x >= self.end {
            self.end
        } else {
            x
        }
    }
}

impl<Idx: PartialOrd + Num> AnyRange<Idx> {
    pub fn contains(&self, sample: Idx) -> bool {
        sample >= self.start && sample < self.end
    }
}

impl<Idx: PartialOrd + Num> From<Range<Idx>> for AnyRange<Idx> {
    fn from(r: Range<Idx>) -> Self {
        Self {
            start: r.start,
            end: r.end,
        }
    }
}

impl<Idx: Copy + PartialOrd + Num> From<RangeInclusive<Idx>> for AnyRange<Idx> {
    fn from(r: RangeInclusive<Idx>) -> Self {
        if *r.end() == Idx::max() {
            panic!("end value outside of allowed range")
        }
        Self {
            start: *r.start(),
            end: *r.end() + Idx::one(),
        }
    }
}

impl<Idx: PartialOrd + Num> From<RangeTo<Idx>> for AnyRange<Idx> {
    fn from(r: RangeTo<Idx>) -> Self {
        Self {
            start: Idx::min(),
            end: r.end
        }
    }
}

impl<Idx: PartialOrd + Num> From<RangeFrom<Idx>> for AnyRange<Idx> {
    fn from(r: RangeFrom<Idx>) -> Self {
        Self {
            start: r.start,
            end: Idx::max(),
        }
    }
}

impl<Idx: Copy + Num + Display> Display for AnyRange<Idx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}..{}]", self.start, self.end + Idx::one())
    }
}

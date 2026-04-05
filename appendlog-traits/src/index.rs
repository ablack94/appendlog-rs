#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Index(u64);

impl Index {
    pub fn new(index: impl Into<u64>) -> Self {
        Self(index.into())
    }
}

impl From<u64> for Index {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<Index> for u64 {
    fn from(value: Index) -> Self {
        value.0
    }
}

impl TryFrom<Index> for usize {
    type Error = std::num::TryFromIntError;

    fn try_from(value: Index) -> Result<Self, Self::Error> {
        value.0.try_into()
    }
}

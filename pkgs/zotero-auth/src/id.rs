use std::fmt::{self, Display};

use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, sqlx::Type,
)]
#[serde(transparent)]
#[sqlx(transparent)]
pub(crate) struct UserId(u64);

impl UserId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    pub(crate) fn from_runtime(value: f64) -> Option<Self> {
        (value.is_finite() && value.fract() == 0.0 && value >= 1.0 && value <= u64::MAX as f64)
            .then_some(Self(value as u64))
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

impl Display for UserId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

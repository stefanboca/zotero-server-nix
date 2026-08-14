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

    pub(crate) fn from_runtime(value: &str) -> Option<Self> {
        value
            .parse::<u64>()
            .ok()
            .filter(|value| *value != 0)
            .map(Self)
    }
}

impl Display for UserId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

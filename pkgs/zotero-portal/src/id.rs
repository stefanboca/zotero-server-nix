use std::fmt::{self, Display};

use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, sqlx::Type,
)]
#[serde(transparent)]
#[sqlx(transparent)]
pub(crate) struct UserId(u32);

impl UserId {
    #[cfg(test)]
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }
}

impl TryFrom<u64> for UserId {
    type Error = std::num::TryFromIntError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Ok(Self(value.try_into()?))
    }
}

impl std::str::FromStr for UserId {
    type Err = std::num::ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.parse()?))
    }
}

impl Display for UserId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

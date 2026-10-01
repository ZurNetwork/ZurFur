use domain::datetime::DateTimeUtc;

use crate::commission::{CommissionResult, status::direction::Direction};
use crate::use_case;

pub type Command = super::set::Command;

pub type Output = super::set::Output;

impl Direction<'_> {
    #[use_case]
    pub async fn clear(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        self.set(cmd, now).await
    }
}

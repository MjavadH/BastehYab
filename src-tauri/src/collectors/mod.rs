//! Operator collectors retrieve official upstream data into operator-specific raw models.

pub mod irancell;

use crate::{
    domain::operator::Operator,
    refresh::orchestrator::{CollectedPackages, Collector, CollectorError},
};

#[derive(Debug, Clone, Default)]
pub struct OperatorCollectors {
    irancell: irancell::IrancellCollector,
}

impl OperatorCollectors {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Collector for OperatorCollectors {
    fn collect(&self, operator: Operator) -> Result<CollectedPackages, CollectorError> {
        match operator {
            Operator::Irancell => self.irancell.collect(operator),
            Operator::Mci | Operator::Rightel | Operator::Samantel => Err(CollectorError::Failed(
                format!("collector for {:?} is not implemented", operator),
            )),
        }
    }
}

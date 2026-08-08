//! Operator collectors retrieve official upstream data into operator-specific raw models.

pub mod irancell;
pub mod mci;
pub mod rightel;
pub mod samantel;

use crate::{
    domain::operator::Operator,
    refresh::orchestrator::{CollectedPackages, Collector, CollectorError},
};

#[derive(Debug, Clone, Default)]
pub struct OperatorCollectors {
    irancell: irancell::IrancellCollector,
    mci: mci::MCICollector,
    rightel: rightel::RightelCollector,
    samantel: samantel::SamantelCollector,
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
            Operator::Mci => self.mci.collect(operator),
            Operator::Rightel => self.rightel.collect(operator),
            Operator::Samantel => self.samantel.collect(operator),
        }
    }
}

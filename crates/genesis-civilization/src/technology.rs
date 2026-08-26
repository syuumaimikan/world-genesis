use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InnovationField {
    Agriculture,
    Metallurgy,
    NavalArchitecture,
    Machinery,
    Electricity,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TechTree {
    pub progress: std::collections::HashMap<InnovationField, f32>,
}

impl TechTree {
    pub fn add_research(&mut self, field: InnovationField, points: f32) -> bool {
        let entry = self.progress.entry(field).or_insert(0.0);
        *entry += points;
        *entry >= 100.0
    }
}

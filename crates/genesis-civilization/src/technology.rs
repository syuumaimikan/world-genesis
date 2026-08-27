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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tech_tree_has_no_progress() {
        assert!(TechTree::default().progress.is_empty());
    }

    #[test]
    fn research_accumulates_per_field_until_it_completes() {
        let mut tree = TechTree::default();
        assert!(!tree.add_research(InnovationField::Agriculture, 40.0));
        assert!(!tree.add_research(InnovationField::Agriculture, 40.0));
        assert!(tree.add_research(InnovationField::Agriculture, 40.0));
        assert_eq!(tree.progress[&InnovationField::Agriculture], 120.0);
    }

    #[test]
    fn fields_progress_independently() {
        let mut tree = TechTree::default();
        tree.add_research(InnovationField::Metallurgy, 100.0);
        assert!(!tree.add_research(InnovationField::Electricity, 10.0));
        assert_eq!(tree.progress[&InnovationField::Metallurgy], 100.0);
        assert_eq!(tree.progress[&InnovationField::Electricity], 10.0);
    }

    #[test]
    fn a_single_breakthrough_can_complete_a_field_immediately() {
        let mut tree = TechTree::default();
        assert!(tree.add_research(InnovationField::Machinery, 250.0));
    }
}

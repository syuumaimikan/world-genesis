use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModItemDefinition {
    pub id: String,
    pub display_name: String,
    pub base_value: f32,
    pub weight_kg: f32,
    pub is_perishable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModBuildingDefinition {
    pub id: String,
    pub display_name: String,
    pub required_wood: f32,
    pub required_stone: f32,
    pub required_tools: f32,
    pub monetary_cost: f64,
    pub productivity_bonus: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModPackage {
    pub mod_name: String,
    pub version: String,
    pub items: Vec<ModItemDefinition>,
    pub buildings: Vec<ModBuildingDefinition>,
}

#[derive(Debug, Clone, Default)]
pub struct ModRegistry {
    pub items: HashMap<String, ModItemDefinition>,
    pub buildings: HashMap<String, ModBuildingDefinition>,
}

impl ModRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_mod_from_json(&mut self, path: impl AsRef<Path>) -> Result<(), String> {
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).map_err(|e| e.to_string())?;

        let package: ModPackage = serde_json::from_str(&contents).map_err(|e| e.to_string())?;

        for item in package.items {
            self.items.insert(item.id.clone(), item);
        }
        for bld in package.buildings {
            self.buildings.insert(bld.id.clone(), bld);
        }

        Ok(())
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_json(name: &str, contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("genesis_modding_{name}.json"));
        let mut file = File::create(&path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        path
    }

    const VALID_MOD: &str = r#"{
        "mod_name": "Ironworks",
        "version": "1.2.0",
        "items": [
            {"id": "steel_ingot", "display_name": "Steel Ingot", "base_value": 12.5,
             "weight_kg": 4.0, "is_perishable": false}
        ],
        "buildings": [
            {"id": "blast_furnace", "display_name": "Blast Furnace", "required_wood": 20.0,
             "required_stone": 40.0, "required_tools": 8.0, "monetary_cost": 500.0,
             "productivity_bonus": 0.25}
        ]
    }"#;

    #[test]
    fn registry_starts_empty() {
        let registry = ModRegistry::new();
        assert!(registry.items.is_empty());
        assert!(registry.buildings.is_empty());
    }

    #[test]
    fn loading_a_mod_registers_items_and_buildings_by_id() {
        let path = write_temp_json("valid", VALID_MOD);
        let mut registry = ModRegistry::new();
        registry.load_mod_from_json(&path).unwrap();

        let item = registry.items.get("steel_ingot").expect("item registered");
        assert_eq!(item.display_name, "Steel Ingot");
        assert_eq!(item.base_value, 12.5);
        assert!(!item.is_perishable);

        let building = registry
            .buildings
            .get("blast_furnace")
            .expect("building registered");
        assert_eq!(building.monetary_cost, 500.0);
        assert_eq!(building.productivity_bonus, 0.25);

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn later_mods_override_entries_with_the_same_id() {
        let first = write_temp_json("override_a", VALID_MOD);
        let second = write_temp_json(
            "override_b",
            r#"{"mod_name":"Rebalance","version":"1","items":[
                {"id":"steel_ingot","display_name":"Refined Steel","base_value":30.0,
                 "weight_kg":4.0,"is_perishable":false}],"buildings":[]}"#,
        );

        let mut registry = ModRegistry::new();
        registry.load_mod_from_json(&first).unwrap();
        registry.load_mod_from_json(&second).unwrap();

        assert_eq!(registry.items.len(), 1);
        assert_eq!(registry.items["steel_ingot"].display_name, "Refined Steel");
        assert_eq!(registry.items["steel_ingot"].base_value, 30.0);
        // Buildings from the first package are retained.
        assert!(registry.buildings.contains_key("blast_furnace"));

        std::fs::remove_file(first).ok();
        std::fs::remove_file(second).ok();
    }

    #[test]
    fn missing_file_is_reported_as_an_error() {
        let mut registry = ModRegistry::new();
        let err = registry
            .load_mod_from_json(std::env::temp_dir().join("genesis_no_such_mod.json"))
            .unwrap_err();
        assert!(!err.is_empty());
        assert!(registry.items.is_empty());
    }

    #[test]
    fn malformed_json_is_reported_as_an_error() {
        let path = write_temp_json("broken", "{ not json ]");
        let mut registry = ModRegistry::new();
        assert!(registry.load_mod_from_json(&path).is_err());
        assert!(registry.items.is_empty());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn package_roundtrips_through_json() {
        let package = ModPackage {
            mod_name: "Test".to_string(),
            version: "0.1".to_string(),
            items: vec![ModItemDefinition {
                id: "apple".to_string(),
                display_name: "Apple".to_string(),
                base_value: 1.0,
                weight_kg: 0.2,
                is_perishable: true,
            }],
            buildings: Vec::new(),
        };
        let json = serde_json::to_string(&package).unwrap();
        let restored: ModPackage = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.items[0].id, "apple");
        assert!(restored.items[0].is_perishable);
    }
}

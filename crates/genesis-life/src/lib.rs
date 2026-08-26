pub mod animals;
pub mod ecology;
pub mod genetics;
pub mod plants;

pub use animals::{AnimalIndividual, AnimalSpecies, FaunaAction};
pub use ecology::{EcosystemGrid, TrophicLevelMetrics};
pub use genetics::{GeneticCode, TraitAllele};
pub use plants::{FloraBiome, FloraCell};

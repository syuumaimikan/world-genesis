pub mod erosion;
pub mod minerals;
pub mod tectonics;
pub mod terrain;

pub use erosion::{ErosionParameters, HydraulicErosionSimulator};
pub use minerals::{MineralComposition, MineralDeposit, MineralType};
pub use tectonics::{TectonicBoundaryType, TectonicPlate, TectonicSimulator};
pub use terrain::{HeightField, RockLayer, SurfaceMaterial};

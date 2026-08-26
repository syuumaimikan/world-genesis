use crate::config::WorldGenesisConfig;
use genesis_civilization::dynasty::PersonId;
use genesis_civilization::{NationState, Settlement};
use genesis_climate::{AtmosphericGrid, ClimateParameters, PlanetaryState, WaterCycleSystem};
use genesis_core::causality::CausalityGraph;
use genesis_core::events::{EventBus, WorldEvent};
use genesis_core::time::{SimClock, SimTick, TICKS_PER_DAY, TICKS_PER_YEAR};
use genesis_economy::{CommodityType, RegionalMarket};
use genesis_geology::erosion::{ErosionParameters, HydraulicErosionSimulator};
use genesis_geology::tectonics::TectonicSimulator;
use genesis_geology::terrain::HeightField;
use genesis_life::ecology::EcosystemGrid;
use glam::Vec2;

pub struct WorldSimulation {
    pub config: WorldGenesisConfig,
    pub clock: SimClock,
    pub causality: CausalityGraph,
    pub event_bus: EventBus,

    pub heightfield: HeightField,
    pub tectonics: TectonicSimulator,
    pub atmosphere: AtmosphericGrid,
    pub water_cycle: WaterCycleSystem,
    pub ecology: EcosystemGrid,
    pub settlements: Vec<Settlement>,
    pub nations: Vec<NationState>,
    pub markets: Vec<RegionalMarket>,

    pub hydraulic_erosion: HydraulicErosionSimulator,
    pub planet_state: PlanetaryState,
}

impl WorldSimulation {
    pub fn new(config: WorldGenesisConfig) -> Self {
        let w = config.map_width;
        let h = config.map_height;

        let heightfield = HeightField::new(w, h, 0.0);
        let tectonics = TectonicSimulator::new(w, h, config.plate_count, config.seed);
        let atmosphere = AtmosphericGrid::new(w, h);
        let ecology = EcosystemGrid::new(w, h);

        let planet_state = PlanetaryState {
            axial_tilt_deg: config.axial_tilt_deg,
            solar_luminosity: 1.0,
            orbital_progress: 0.0,
            sea_level: config.sea_level,
        };

        Self {
            config,
            clock: SimClock::new(),
            causality: CausalityGraph::new(),
            event_bus: EventBus::new(),
            heightfield,
            tectonics,
            atmosphere,
            water_cycle: WaterCycleSystem::default(),
            ecology,
            settlements: Vec::new(),
            nations: Vec::new(),
            markets: Vec::new(),
            hydraulic_erosion: HydraulicErosionSimulator::new(ErosionParameters::default()),
            planet_state,
        }
    }

    pub fn bootstrap_genesis(&mut self) {
        // 1. フラクタル大陸・海洋の生成
        self.heightfield.generate_continents(self.config.seed);

        // 2. 気候シミュレーション初期化
        let climate_params = ClimateParameters::default();
        self.atmosphere.update_climate(
            &self.heightfield.elevation,
            &self.planet_state,
            &climate_params,
        );

        // 3. 河川ネットワーク生成
        let precip: Vec<f32> = self.atmosphere.cells.iter().map(|c| c.precipitation_rate).collect();
        self.water_cycle.generate_drainage_network(
            self.config.map_width,
            self.config.map_height,
            &self.heightfield.elevation,
            &precip,
            self.config.sea_level,
        );

        // 4. 国家・首都村の創設
        let nation_id = 1;
        self.nations.push(NationState {
            id: nation_id,
            name: "Elvoria Kingdom".to_string(),
            government: genesis_civilization::GovernmentForm::FeudalMonarchy,
            succession: genesis_civilization::SuccessionLaw::Primogeniture,
            sovereign_ruler_id: PersonId(101),
            treasury_balance: 20_000.0,
            legitimacy_pct: 95.0,
            tax_rate: 0.10,
            is_at_war: false,
            capital_settlement_id: 1,
        });

        // 平野（標高 20m〜80m）に初期の村を配置
        let mut village_x = self.config.map_width / 2;
        let mut village_y = self.config.map_height / 2;
        for y in (self.config.map_height / 4)..(self.config.map_height * 3 / 4) {
            for x in (self.config.map_width / 4)..(self.config.map_width * 3 / 4) {
                let e = self.heightfield.get_elevation(x, y);
                if e > 15.0 && e < 80.0 {
                    village_x = x;
                    village_y = y;
                    break;
                }
            }
        }

        let mut capital = Settlement::new(
            1,
            "Riverdale Village",
            Vec2::new(village_x as f32, village_y as f32),
            nation_id,
        );
        capital.population = 85;
        self.settlements.push(capital);
        self.markets.push(RegionalMarket::new());

        let genesis_event = self.causality.record_event(
            SimTick(0),
            "Genesis",
            "World Genesis realm initialized with flora, fauna, and early settlements",
            Vec::new(),
            1.0,
            "{}",
        );
        self.event_bus.publish(
            SimTick(0),
            Some(genesis_event),
            WorldEvent::NationFallen {
                nation_id: 0,
                successor_nation_id: Some(nation_id),
            },
        );
    }

    pub fn tick_step(&mut self, elapsed_ticks: u64) {
        let current_tick = self.clock.step(elapsed_ticks);

        if current_tick.0 % TICKS_PER_DAY == 0 {
            self.step_daily(current_tick);
        }
        if current_tick.0 % TICKS_PER_YEAR == 0 {
            self.step_yearly(current_tick);
        }
    }

    fn step_daily(&mut self, _tick: SimTick) {
        for (i, settlement) in self.settlements.iter_mut().enumerate() {
            settlement.step_demographics(80.0);
            if i < self.markets.len() {
                self.markets[i].post_production(CommodityType::Grain, 60.0);
                self.markets[i].post_consumption(CommodityType::Grain, settlement.population as f32 * 0.2);
                self.markets[i].clear_market();
            }
        }
    }

    fn step_yearly(&mut self, _tick: SimTick) {
        self.planet_state.orbital_progress = (self.planet_state.orbital_progress + 0.05) % 1.0;
        let climate_params = ClimateParameters::default();
        self.atmosphere.update_climate(
            &self.heightfield.elevation,
            &self.planet_state,
            &climate_params,
        );
        let temps: Vec<f32> = self.atmosphere.cells.iter().map(|c| c.temperature_c).collect();
        let precip: Vec<f32> = self.atmosphere.cells.iter().map(|c| c.precipitation_rate).collect();
        self.ecology.step_lotka_volterra(&temps, &precip, 1.0);
    }
}

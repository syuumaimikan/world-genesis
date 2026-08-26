use crate::world::WorldSimulation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Elevation,
    Temperature,
    Vegetation,
    Political,
}

pub struct WorldInspector;

impl WorldInspector {
    pub fn render_ansi_viewport(world: &WorldSimulation, mode: ViewMode, view_w: usize, view_h: usize) {
        let step_x = world.config.map_width / view_w;
        let step_y = world.config.map_height / view_h;

        println!("\x1B[2J\x1B[H"); // ANSI Terminal Clear & Home
        println!("==================== WORLD GENESIS: INSPECTOR ====================");

        for vy in 0..view_h {
            let mut line = String::with_capacity(view_w * 8);
            for vx in 0..view_w {
                let gx = vx * step_x;
                let gy = vy * step_y;
                let idx = gy * world.config.map_width + gx;

                let char_symbol = match mode {
                    ViewMode::Elevation => {
                        let e = world.heightfield.elevation[idx];
                        if e < world.config.sea_level {
                            "\x1B[38;5;33m~\x1B[0m" // Sea Blue
                        } else if e < 300.0 {
                            "\x1B[38;5;34m.\x1B[0m" // Lowland Green
                        } else if e < 800.0 {
                            "\x1B[38;5;178mn\x1B[0m" // Hill Yellow
                        } else {
                            "\x1B[38;5;255m^\x1B[0m" // Mountain Snow White
                        }
                    }
                    ViewMode::Temperature => {
                        let t = world.atmosphere.cells[idx].temperature_c;
                        if t < 0.0 {
                            "\x1B[38;5;39m*\x1B[0m" // Freezing Ice Cyan
                        } else if t < 20.0 {
                            "\x1B[38;5;48m+\x1B[0m" // Temperate Green
                        } else {
                            "\x1B[38;5;196m#\x1B[0m" // Arid Heat Red
                        }
                    }
                    ViewMode::Vegetation => {
                        let bio = world.ecology.flora[idx].biomass_density;
                        if bio > 15.0 {
                            "\x1B[38;5;28m%\x1B[0m" // Dense Rainforest
                        } else if bio > 4.0 {
                            "\x1B[38;5;64m\"\x1B[0m" // Forest / Savanna
                        } else {
                            "\x1B[38;5;240m.\x1B[0m" // Barren
                        }
                    }
                    ViewMode::Political => {
                        let is_settlement = world.settlements.iter().any(|s| {
                            (s.position.x as usize / step_x == vx) && (s.position.y as usize / step_y == vy)
                        });
                        if is_settlement {
                            "\x1B[38;5;220m@\x1B[0m" // Settlement Crown Gold
                        } else {
                            "\x1B[38;5;236m.\x1B[0m"
                        }
                    }
                };
                line.push_str(char_symbol);
            }
            println!("{}", line);
        }
        println!("------------------------------------------------------------------");
    }
}

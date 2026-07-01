use crate::simulation::config::SimulationConfig;
use crate::simulation::{CellType, EnemyState, MeteoriteAnim, MeteoriteFlight, RobotState};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Serialize, Deserialize)]
pub struct GameState {
    pub config: SimulationConfig,
    pub width: usize,
    pub height: usize,
    pub map: Vec<Vec<CellType>>,
    pub robots: Vec<RobotState>,
    pub enemies: Vec<EnemyState>,
    pub fear_factor: f32,
    pub base_hp: i32,
    pub collected_crystals: u32,
    pub collected_meat: u32,
    pub collected_metal: u32,
    pub cheat_mode: bool,
    pub meteorite_anims: Vec<MeteoriteAnim>,
    pub meteorite_flights: Vec<MeteoriteFlight>,
    pub wall_built: bool,
    pub known_resources: Vec<(usize, usize)>,
    pub claimed_resources: HashSet<(usize, usize)>,
}

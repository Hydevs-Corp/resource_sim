use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct SimulationConfig {
    pub base_hp: i32,
    pub robot_hp: i32,
    pub enemy_hp: i32,
    pub enemy_spawn_speed_ms: u64,
    pub collector_capacity: u32,
    pub army_cost_metal: u32,
    pub army_cost_meat: u32,
    pub width: usize,
    pub height: usize,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            base_hp: 1000,
            robot_hp: 100,
            enemy_hp: 10,
            enemy_spawn_speed_ms: 3000,
            collector_capacity: 50,
            army_cost_metal: 100,
            army_cost_meat: 10,
            width: 80,
            height: 40,
        }
    }
}

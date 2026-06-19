use noise::{NoiseFn, Perlin};
use rand::RngExt;
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::{HashSet, VecDeque};
use std::sync::LazyLock;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

mod spawns;

#[derive(Clone, Copy, PartialEq)]
pub struct EnemyState {
    pub id: usize,
    pub x: usize,
    pub y: usize,
    pub hp: i32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CellType {
    Empty,
    Obstacle,
    Wall(u32),
    Door(u32),
    Energy(u32),
    Crystal(u32),
    Metal(u32),
    Meat(u32),
    Base,
}
impl CellType {
    fn is_passable(self) -> bool {
        match self {
            CellType::Obstacle => false,
            CellType::Wall(_) => false,
            CellType::Door(hp) => hp == 0,
            _ => true,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum RobotType {
    Scout,
    Collector,
    Army,
}

#[derive(Clone, Copy, PartialEq)]
pub struct RobotState {
    pub id: usize,
    pub r_type: RobotType,
    pub x: usize,
    pub y: usize,
    pub hp: i32,
}

pub const METEORITE_ANIM_FRAMES: u8 = 8;

#[derive(Clone, Copy)]
pub struct MeteoriteAnim {
    pub x: usize,
    pub y: usize,
    pub frame: u8,
    ticks_since_advance: u8,
    pub center_x: usize,
    pub center_y: usize,
}

const METEORITE_TICKS_PER_FRAME: u8 = 3;
const METEORITE_IMPACT_RADIUS: usize = 2;

pub const BASE_WALL_BUILD_THRESHOLD: u32 = 1000;
pub const BASE_WALL_RADIUS: usize = 4;
pub const BASE_WALL_HP: u32 = 9999;
pub const BASE_DOOR_HP: u32 = 60;

const METEORITE_RESOURCE_SPAWN_CHANCE_PERCENT: u8 = 35; // 35% by default
const METEORITE_RESOURCE_BASE_MIN: u32 = 20;
const METEORITE_RESOURCE_BASE_MAX: u32 = 100;

#[derive(Clone, Copy)]
pub struct MeteoriteFlight {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub tx: usize,
    pub ty: usize,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct FontConfig {
    pub robots: HashMap<String, FontItem>,
    pub cells: HashMap<String, FontItem>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct FontItem {
    pub character: String,
    pub color: String,
}

pub static DEFAULT_FONT: LazyLock<FontConfig> =
    LazyLock::new(|| serde_json::from_str(include_str!("../fonts/default.json")).unwrap());
pub static NERD_FONT: LazyLock<FontConfig> =
    LazyLock::new(|| serde_json::from_str(include_str!("../fonts/nerdfont.json")).unwrap());

pub enum Message {
    Moved(usize, usize, usize),
    ResourceFound(usize, usize),
    ResourceCollected(usize, usize, u32),
    Unloaded(u32, u32, u32, u32),
    EnemySpawned(usize, usize, usize),
    EnemyMoved(usize, usize, usize),
    AttackRobot(usize, i32),
    AttackEnemy(usize, i32, bool),
    AttackBase(u32),
    MeteoriteIncoming(usize, usize, usize, usize),
    AttackDoor(usize, usize, i32),
}

pub struct Simulation {
    pub width: usize,
    pub height: usize,
    pub map: Arc<RwLock<Vec<Vec<CellType>>>>,
    pub robots: Arc<RwLock<Vec<RobotState>>>,
    pub enemies: Arc<RwLock<Vec<EnemyState>>>,
    pub fear_factor: f32,
    pub shared_fear: Arc<RwLock<f32>>,
    pub base_hp: i32,
    pub collected_crystals: u32,
    pub collected_meat: u32,
    pub collected_metal: u32,
    pub sender: Sender<Message>,
    pub cheat_mode: bool,
    pub meteorite_anims: Arc<RwLock<Vec<MeteoriteAnim>>>,
    pub meteorite_flights: Arc<RwLock<Vec<MeteoriteFlight>>>,
    pub wall_built: bool,
    pub selected_font: &'static FontConfig,
    receiver: Receiver<Message>,
    known_resources: Arc<RwLock<Vec<(usize, usize)>>>,
    _claimed_resources: Arc<RwLock<HashSet<(usize, usize)>>>,
}

fn step_towards(
    map: &Vec<Vec<CellType>>,
    from: (usize, usize),
    to: (usize, usize),
    width: usize,
    height: usize,
) -> Option<(usize, usize)> {
    if from == to {
        return None;
    }

    let mut queue = VecDeque::new();
    let mut visited = vec![vec![false; width]; height];
    let mut parent: Vec<Vec<Option<(usize, usize)>>> = vec![vec![None; width]; height];

    queue.push_back(from);
    visited[from.1][from.0] = true;

    while let Some((cx, cy)) = queue.pop_front() {
        if (cx, cy) == to {
            let mut cur = to;
            loop {
                match parent[cur.1][cur.0] {
                    Some(p) if p == from => return Some(cur),
                    Some(p) => cur = p,
                    None => return None,
                }
            }
        }

        for (dx, dy) in [(0i32, 1i32), (0, -1), (1, 0), (-1, 0)] {
            let nx = cx as i32 + dx;
            let ny = cy as i32 + dy;
            if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                let nx = nx as usize;
                let ny = ny as usize;
                if !visited[ny][nx] && (map[ny][nx].is_passable() || (nx, ny) == to) {
                    visited[ny][nx] = true;
                    parent[ny][nx] = Some((cx, cy));
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    None
}

impl Simulation {
    pub fn new(width: usize, height: usize) -> Self {
        let mut raw_map = vec![vec![CellType::Empty; width]; height];
        let mut rng = rand::rng();
        let perlin = Perlin::new(rng.random());
        for y in 0..height {
            for x in 0..width {
                let nx = x as f64 / 10.0;
                let ny = y as f64 / 10.0;
                let noise_val = perlin.get([nx, ny]);

                if noise_val > 0.3 {
                    raw_map[y][x] = CellType::Obstacle;
                } else if rng.random_bool(0.02) {
                    raw_map[y][x] = CellType::Energy(rng.random_range(50..=200));
                } else if rng.random_bool(0.02) {
                    raw_map[y][x] = CellType::Crystal(rng.random_range(50..=200));
                }
            }
        }

        // 2
        let original_map = raw_map.clone();

        for y in 0..height {
            for x in 0..width {
                if matches!(original_map[y][x], CellType::Obstacle) {
                    let mut is_border = false;

                    let neighbors = [(0, -1), (0, 1), (-1, 0), (1, 0)];

                    for (dx, dy) in neighbors {
                        let nx = x as isize + dx;
                        let ny = y as isize + dy;

                        if nx >= 0 && nx < width as isize && ny >= 0 && ny < height as isize {
                            if !matches!(original_map[ny as usize][nx as usize], CellType::Obstacle)
                            {
                                is_border = true;
                            }
                        } else {
                            is_border = true;
                        }
                    }

                    if is_border && rng.random_bool(0.10) {
                        raw_map[y][x] = CellType::Metal(rng.random_range(50..=200));
                    }
                }
            }
        }

        // 3
        let base_x = width / 2;
        let base_y = height / 2;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let bx = (base_x as i32 + dx) as usize;
                let by = (base_y as i32 + dy) as usize;
                if bx < width && by < height {
                    raw_map[by][bx] = CellType::Empty;
                }
            }
        }
        raw_map[base_y][base_x] = CellType::Base;

        let map = Arc::new(RwLock::new(raw_map));
        let known_resources: Arc<RwLock<Vec<(usize, usize)>>> = Arc::new(RwLock::new(Vec::new()));
        let claimed_resources: Arc<RwLock<HashSet<(usize, usize)>>> =
            Arc::new(RwLock::new(HashSet::new()));
        let (sender, receiver) = mpsc::channel();
        let robots = Arc::new(RwLock::new(Vec::new()));
        let enemies = Arc::new(RwLock::new(Vec::new()));
        let meteorite_anims: Arc<RwLock<Vec<MeteoriteAnim>>> = Arc::new(RwLock::new(Vec::new()));
        let meteorite_flights: Arc<RwLock<Vec<MeteoriteFlight>>> =
            Arc::new(RwLock::new(Vec::new()));

        let shared_fear = Arc::new(RwLock::new(0.5));

        // 4
        for i in 0..5 {
            let r_type = if i < 2 {
                RobotType::Scout
            } else {
                RobotType::Collector
            };
            let hp = if r_type == RobotType::Scout { 50 } else { 100 };
            robots.write().unwrap().push(RobotState {
                id: i,
                r_type,
                x: base_x,
                y: base_y,
                hp,
            });

            let sender_clone = sender.clone();
            if r_type == RobotType::Scout {
                spawns::spawn_scout(
                    i,
                    base_x,
                    base_y,
                    sender_clone,
                    Arc::clone(&map),
                    width,
                    height,
                );
            } else {
                spawns::spawn_collector(
                    i,
                    base_x,
                    base_y,
                    sender_clone,
                    Arc::clone(&map),
                    Arc::clone(&known_resources),
                    Arc::clone(&claimed_resources),
                    Arc::clone(&shared_fear),
                    width,
                    height,
                );
            }
        }

        let mut next_id = robots.read().unwrap().len();
        for _ in 0..2 {
            let id = next_id;
            next_id += 1;
            robots.write().unwrap().push(RobotState {
                id,
                r_type: RobotType::Army,
                x: base_x,
                y: base_y,
                hp: 10000,
            });
            spawns::spawn_army(
                id,
                base_x,
                base_y,
                sender.clone(),
                Arc::clone(&map),
                Arc::clone(&enemies),
                Arc::clone(&robots),
                width,
                height,
            );
        }

        // 5
        let sender_spawner = sender.clone();
        let map_spawner = Arc::clone(&map);
        let robots_spawner = Arc::clone(&robots);
        let w = width;
        let h = height;
        thread::spawn(move || {
            let mut rng = rand::rng();
            let mut enemy_id = 0;
            loop {
                thread::sleep(Duration::from_secs(3));

                let valid_spawns = {
                    let map_r = map_spawner.read().unwrap();
                    let mut spots = Vec::new();

                    for x in 0..w {
                        if map_r[0][x].is_passable() {
                            spots.push((x, 0));
                        }
                        if map_r[h - 1][x].is_passable() {
                            spots.push((x, h - 1));
                        }
                    }

                    for y in 1..h - 1 {
                        if map_r[y][0].is_passable() {
                            spots.push((0, y));
                        }
                        if map_r[y][w - 1].is_passable() {
                            spots.push((w - 1, y));
                        }
                    }

                    spots
                };

                if valid_spawns.is_empty() {
                    continue;
                }

                let (ex, ey) = valid_spawns[rng.random_range(0..valid_spawns.len())];

                let _ = sender_spawner.send(Message::EnemySpawned(enemy_id, ex, ey));
                spawns::spawn_enemy(
                    enemy_id,
                    ex,
                    ey,
                    sender_spawner.clone(),
                    Arc::clone(&map_spawner),
                    Arc::clone(&robots_spawner),
                    w,
                    h,
                );
                enemy_id += 1;
            }
        });

        let sender_meteorite = sender.clone();
        let width_meteorite = width;
        let height_meteorite = height;
        thread::spawn(move || {
            let mut rng = rand::rng();
            loop {
                thread::sleep(Duration::from_secs(rng.random_range(10..30)));
                let tx = rng.random_range(0..width_meteorite);
                let ty = rng.random_range(0..height_meteorite);
                let sx = rng.random_range(0..width_meteorite);
                let sy = 0usize;
                let _ = sender_meteorite.send(Message::MeteoriteIncoming(sx, sy, tx, ty));
            }
        });

        Simulation {
            width,
            height,
            map,
            robots,
            enemies,
            fear_factor: 0.5,
            shared_fear,
            base_hp: 1000,
            collected_crystals: 0,
            collected_meat: 0,
            collected_metal: 0,
            sender,
            cheat_mode: false,
            meteorite_anims,
            meteorite_flights,
            wall_built: false,
            selected_font: &DEFAULT_FONT,
            receiver,
            known_resources,
            _claimed_resources: claimed_resources,
        }
    }

    pub fn create_random_crystals(&mut self, count: usize) {
        let mut rng = rand::rng();
        let mut map_w = self.map.write().unwrap();
        for _ in 0..count {
            let x = rng.random_range(0..self.width);
            let y = rng.random_range(0..self.height);
            if map_w[y][x] == CellType::Empty {
                map_w[y][x] = CellType::Crystal(rng.random_range(50..=200));
            }
        }
    }

    pub fn create_random_energy(&mut self, count: usize) {
        let mut rng = rand::rng();
        let mut map_w = self.map.write().unwrap();
        for _ in 0..count {
            let x = rng.random_range(0..self.width);
            let y = rng.random_range(0..self.height);
            if map_w[y][x] == CellType::Empty {
                map_w[y][x] = CellType::Energy(rng.random_range(50..=200));
            }
        }
    }

    pub fn update(&mut self) {
        if !self.wall_built && self.fear_factor >= 40.0 {
            let total_resources =
                self.collected_crystals + self.collected_metal + self.collected_meat;
            if total_resources >= BASE_WALL_BUILD_THRESHOLD {
                let bx = self.width / 2;
                let by = self.height / 2;
                let mut map_w = self.map.write().unwrap();
                for dy in -(BASE_WALL_RADIUS as isize)..=(BASE_WALL_RADIUS as isize) {
                    for dx in -(BASE_WALL_RADIUS as isize)..=(BASE_WALL_RADIUS as isize) {
                        let nx_i = bx as isize + dx;
                        let ny_i = by as isize + dy;
                        if nx_i < 0 || ny_i < 0 {
                            continue;
                        }
                        let nx = nx_i as usize;
                        let ny = ny_i as usize;
                        if nx >= self.width || ny >= self.height {
                            continue;
                        }

                        let dist = ((dx * dx + dy * dy) as f64).sqrt();
                        if (dist - BASE_WALL_RADIUS as f64).abs() <= 0.6 {
                            if map_w[ny][nx] != CellType::Empty {
                                continue;
                            }
                            if (dx == 0 && dy == -(BASE_WALL_RADIUS as isize))
                                || (dx == (BASE_WALL_RADIUS as isize) && dy == 0)
                                || (dx == 0 && dy == (BASE_WALL_RADIUS as isize))
                                || (dx == -(BASE_WALL_RADIUS as isize) && dy == 0)
                            {
                                map_w[ny][nx] = CellType::Door(BASE_DOOR_HP);
                            } else {
                                map_w[ny][nx] = CellType::Wall(BASE_WALL_HP);
                            }
                        }
                    }
                }
                self.wall_built = true;
            }
        }
        {
            let mut flights = self.meteorite_flights.write().unwrap();
            let mut arrived: Vec<(usize, usize)> = Vec::new();
            *self.shared_fear.write().unwrap() = self.fear_factor;
            for f in flights.iter_mut() {
                f.x += f.vx;
                f.y += f.vy;
                let dx = f.x - f.tx as f32;
                let dy = f.y - f.ty as f32;
                if (dx * dx + dy * dy) <= 0.5 {
                    arrived.push((f.tx, f.ty));
                }
            }
            flights.retain(|f| {
                let dx = f.x - f.tx as f32;
                let dy = f.y - f.ty as f32;
                (dx * dx + dy * dy) > 0.5
            });
            drop(flights);
            if !arrived.is_empty() {
                let mut to_place: Vec<(usize, usize, usize, usize)> = Vec::new();
                {
                    let mut anims = self.meteorite_anims.write().unwrap();
                    let map_r = self.map.read().unwrap();
                    for (cx, cy) in arrived.iter() {
                        for dy in
                            -(METEORITE_IMPACT_RADIUS as isize)..=(METEORITE_IMPACT_RADIUS as isize)
                        {
                            for dx in -(METEORITE_IMPACT_RADIUS as isize)
                                ..=(METEORITE_IMPACT_RADIUS as isize)
                            {
                                let nx_i = *cx as isize + dx;
                                let ny_i = *cy as isize + dy;
                                if nx_i < 0 || ny_i < 0 {
                                    continue;
                                }
                                let nx = nx_i as usize;
                                let ny = ny_i as usize;
                                if nx >= self.width || ny >= self.height {
                                    continue;
                                }
                                let ddx = dx as isize as f32;
                                let ddy = dy as isize as f32;
                                if (ddx * ddx + ddy * ddy)
                                    > (METEORITE_IMPACT_RADIUS as f32).powf(2.0)
                                {
                                    continue;
                                }
                                if anims.iter().any(|a| a.x == nx && a.y == ny) {
                                    continue;
                                }
                                let dist = (dx.abs() as usize).max(dy.abs() as usize);
                                let ticks_offset = dist as u8;
                                anims.push(MeteoriteAnim {
                                    x: nx,
                                    y: ny,
                                    frame: 0,
                                    ticks_since_advance: ticks_offset,
                                    center_x: *cx,
                                    center_y: *cy,
                                });
                                if map_r[ny][nx] == CellType::Empty {
                                    to_place.push((nx, ny, *cx, *cy));
                                }
                            }
                        }
                    }
                }
                if !to_place.is_empty() {
                    let mut map_w = self.map.write().unwrap();
                    let mut rng = rand::rng();
                    for (nx, ny, cx, cy) in to_place {
                        if map_w[ny][nx] == CellType::Empty {
                            if rng.random_range(0..100) as u8
                                >= METEORITE_RESOURCE_SPAWN_CHANCE_PERCENT
                            {
                                continue;
                            }
                            let dx = (nx as isize - cx as isize).abs() as f32;
                            let dy = (ny as isize - cy as isize).abs() as f32;
                            let eu = (dx * dx + dy * dy).sqrt();
                            let multiplier = if eu < 0.75 {
                                3
                            } else if eu < 1.75 {
                                2
                            } else {
                                1
                            };
                            let resource_type = rng.random_range(0..3);
                            let base_amount: u32 = rng.random_range(
                                METEORITE_RESOURCE_BASE_MIN..=METEORITE_RESOURCE_BASE_MAX,
                            );
                            let amount = base_amount.saturating_mul(multiplier as u32);
                            map_w[ny][nx] = match resource_type {
                                0 => CellType::Crystal(amount),
                                1 => CellType::Energy(amount),
                                2 => CellType::Metal(amount),
                                _ => CellType::Crystal(amount),
                            };
                        }
                    }
                }
            }

            let mut anims = self.meteorite_anims.write().unwrap();
            let mut finished: Vec<(usize, usize, usize, usize)> = Vec::new();
            for anim in anims.iter_mut() {
                anim.ticks_since_advance += 1;
                if anim.ticks_since_advance >= METEORITE_TICKS_PER_FRAME {
                    anim.ticks_since_advance = 0;
                    if anim.frame + 1 >= METEORITE_ANIM_FRAMES {
                        finished.push((anim.x, anim.y, anim.center_x, anim.center_y));
                    } else {
                        anim.frame += 1;
                    }
                }
            }
            anims.retain(|a| a.frame + 1 < METEORITE_ANIM_FRAMES);

            if !finished.is_empty() {
                let mut map_w = self.map.write().unwrap();
                let mut rng = rand::rng();
                for (x, y, cx, cy) in finished {
                    if map_w[y][x] == CellType::Empty {
                        if rng.random_range(0..100) as u8 >= METEORITE_RESOURCE_SPAWN_CHANCE_PERCENT
                        {
                            continue;
                        }
                        let dx = (x as isize - cx as isize).abs() as f32;
                        let dy = (y as isize - cy as isize).abs() as f32;
                        let eu = (dx * dx + dy * dy).sqrt();
                        let multiplier = if eu < 0.75 {
                            3
                        } else if eu < 1.75 {
                            2
                        } else {
                            1
                        };
                        let resource_type = rng.random_range(0..3);
                        let base_amount: u32 = rng.random_range(
                            METEORITE_RESOURCE_BASE_MIN..=METEORITE_RESOURCE_BASE_MAX,
                        );
                        let amount = base_amount.saturating_mul(multiplier as u32);
                        map_w[y][x] = match resource_type {
                            0 => CellType::Crystal(amount),
                            1 => CellType::Energy(amount),
                            2 => CellType::Metal(amount),
                            _ => CellType::Crystal(amount),
                        };
                    }
                }
            }
        }

        let target_army_units = (self.fear_factor / 10.0).floor() as usize;

        let current_army_units = self
            .robots
            .read()
            .unwrap()
            .iter()
            .filter(|r| r.r_type == RobotType::Army)
            .count();

        if target_army_units > current_army_units {
            let mut spawn_count = target_army_units - current_army_units;

            while spawn_count > 0 && self.collected_metal >= 100 && self.collected_meat >= 10 {
                self.collected_metal -= 100;
                self.collected_meat -= 10;

                self.base_hp = self.base_hp.saturating_add(500);

                let next_id = {
                    let robs = self.robots.read().unwrap();
                    robs.iter().map(|r| r.id).max().unwrap_or(0) + 1
                };

                let base_x = self.width / 2;
                let base_y = self.height / 2;

                self.robots.write().unwrap().push(RobotState {
                    id: next_id,
                    r_type: RobotType::Army,
                    x: base_x,
                    y: base_y,
                    hp: 150,
                });

                spawns::spawn_army(
                    next_id,
                    base_x,
                    base_y,
                    self.sender.clone(),
                    Arc::clone(&self.map),
                    Arc::clone(&self.enemies),
                    Arc::clone(&self.robots),
                    self.width,
                    self.height,
                );

                spawn_count -= 1;
            }
        }

        let known_nodes_count = self.known_resources.read().unwrap().len();

        if known_nodes_count < 20 {
            let current_scouts = self
                .robots
                .read()
                .unwrap()
                .iter()
                .filter(|r| r.r_type == RobotType::Scout)
                .count();
            if self.collected_crystals >= 50 {
                self.collected_crystals -= 50;

                let next_id = {
                    let robs = self.robots.read().unwrap();
                    robs.iter().map(|r| r.id).max().unwrap_or(0) + 1
                };

                let base_x = self.width / 2;
                let base_y = self.height / 2;

                self.robots.write().unwrap().push(RobotState {
                    id: next_id,
                    r_type: RobotType::Scout,
                    x: base_x,
                    y: base_y,
                    hp: 50,
                });

                spawns::spawn_scout(
                    next_id,
                    base_x,
                    base_y,
                    self.sender.clone(),
                    Arc::clone(&self.map),
                    self.width,
                    self.height,
                );
            }

            if self.collected_crystals >= 15 && current_scouts > 0 {
                self.collected_crystals -= 15;

                let next_id = {
                    let robs = self.robots.read().unwrap();
                    robs.iter().map(|r| r.id).max().unwrap_or(0) + 1
                };

                let base_x = self.width / 2;
                let base_y = self.height / 2;

                self.robots.write().unwrap().push(RobotState {
                    id: next_id,
                    r_type: RobotType::Collector,
                    x: base_x,
                    y: base_y,
                    hp: 100,
                });

                spawns::spawn_collector(
                    next_id,
                    base_x,
                    base_y,
                    self.sender.clone(),
                    Arc::clone(&self.map),
                    Arc::clone(&self.known_resources),
                    Arc::clone(&self._claimed_resources),
                    Arc::clone(&self.shared_fear),
                    self.width,
                    self.height,
                );
            }
        }

        while let Ok(msg) = self.receiver.try_recv() {
            match msg {
                Message::Moved(id, x, y) => {
                    if let Some(robot) =
                        self.robots.write().unwrap().iter_mut().find(|r| r.id == id)
                    {
                        robot.x = x;
                        robot.y = y;
                    }
                }
                Message::ResourceFound(x, y) => {
                    let mut resources = self.known_resources.write().unwrap();
                    if !resources.contains(&(x, y)) {
                        resources.push((x, y));
                    }
                }
                Message::ResourceCollected(x, y, amount) => {
                    let mut map_w = self.map.write().unwrap();
                    match map_w[y][x] {
                        CellType::Energy(n) => {
                            if n > amount {
                                map_w[y][x] = CellType::Energy(n - amount);
                            } else {
                                map_w[y][x] = CellType::Empty;
                                self.known_resources
                                    .write()
                                    .unwrap()
                                    .retain(|&(rx, ry)| !(rx == x && ry == y));
                                self._claimed_resources.write().unwrap().remove(&(x, y));
                            }
                        }
                        CellType::Crystal(n) => {
                            if n > amount {
                                map_w[y][x] = CellType::Crystal(n - amount);
                            } else {
                                map_w[y][x] = CellType::Empty;
                                self.known_resources
                                    .write()
                                    .unwrap()
                                    .retain(|&(rx, ry)| !(rx == x && ry == y));
                                self._claimed_resources.write().unwrap().remove(&(x, y));
                            }
                        }
                        CellType::Metal(n) => {
                            if n > amount {
                                map_w[y][x] = CellType::Metal(n - amount);
                            } else {
                                map_w[y][x] = CellType::Empty;
                                self.known_resources
                                    .write()
                                    .unwrap()
                                    .retain(|&(rx, ry)| !(rx == x && ry == y));
                                self._claimed_resources.write().unwrap().remove(&(x, y));
                            }
                        }
                        CellType::Meat(n) => {
                            if n > amount {
                                map_w[y][x] = CellType::Meat(n - amount);
                            } else {
                                map_w[y][x] = CellType::Empty;
                                self.known_resources
                                    .write()
                                    .unwrap()
                                    .retain(|&(rx, ry)| !(rx == x && ry == y));
                                self._claimed_resources.write().unwrap().remove(&(x, y));
                            }
                        }
                        _ => {}
                    }
                }
                Message::Unloaded(energy, crystals, metal, meat) => {
                    if energy > 0 {
                        self.base_hp = self.base_hp.saturating_add(energy as i32);
                        self.fear_factor = (self.fear_factor - 1.0).max(0.0);
                    }
                    self.collected_crystals = self.collected_crystals.saturating_add(crystals);
                    self.collected_metal = self.collected_metal.saturating_add(metal);
                    self.collected_meat = self.collected_meat.saturating_add(meat);
                }
                Message::EnemySpawned(id, x, y) => {
                    self.enemies
                        .write()
                        .unwrap()
                        .push(EnemyState { id, x, y, hp: 30 });
                }
                Message::EnemyMoved(id, x, y) => {
                    let mut en = self.enemies.write().unwrap();
                    if let Some(enemy) = en.iter_mut().find(|e| e.id == id) {
                        enemy.x = x;
                        enemy.y = y;
                    }
                }
                Message::AttackEnemy(id, damage, killed_by_army) => {
                    let mut en = self.enemies.write().unwrap();
                    if let Some(idx) = en.iter().position(|e| e.id == id) {
                        en[idx].hp -= damage;
                        if en[idx].hp <= 0 {
                            let ex = en[idx].x;
                            let ey = en[idx].y;
                            en.remove(idx);
                            let mut map_w = self.map.write().unwrap();
                            if map_w[ey][ex] == CellType::Empty {
                                map_w[ey][ex] = CellType::Meat(30);
                                if killed_by_army {
                                    let mut known = self.known_resources.write().unwrap();
                                    if !known.contains(&(ex, ey)) {
                                        known.push((ex, ey));
                                    }
                                }
                            }
                            self.fear_factor = (self.fear_factor - 1.0).max(0.0);
                        }
                    }
                }
                Message::AttackRobot(id, damage) => {
                    let mut robs = self.robots.write().unwrap();
                    if let Some(robot) = robs.iter_mut().find(|r| r.id == id) {
                        robot.hp -= damage;
                        if robot.hp <= 0 {
                            self.fear_factor = self.fear_factor + 5.0;
                        }
                    }
                    robs.retain(|r| r.hp > 0);
                }
                Message::AttackDoor(dx, dy, dmg) => {
                    let mut map_w = self.map.write().unwrap();
                    if let CellType::Door(hp) = map_w[dy][dx] {
                        let nhp = hp.saturating_sub(dmg as u32);
                        map_w[dy][dx] = CellType::Door(nhp);
                    }
                }
                Message::MeteoriteIncoming(sx, sy, tx, ty) => {
                    let mut flights = self.meteorite_flights.write().unwrap();
                    let dx = tx as f32 - sx as f32;
                    let dy = ty as f32 - sy as f32;
                    let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                    let mut rng = rand::rng();
                    let speed = rng.random_range(5..15) as f32 / 10.0;
                    let vx = dx / dist * speed;
                    let vy = dy / dist * speed;
                    flights.push(MeteoriteFlight {
                        x: sx as f32,
                        y: sy as f32,
                        vx,
                        vy,
                        tx,
                        ty,
                    });
                }

                Message::AttackBase(damage) => {
                    self.base_hp = self.base_hp.saturating_sub(damage as i32);
                    self.fear_factor = self.fear_factor + 10.0;
                }
            }
        }
    }
}

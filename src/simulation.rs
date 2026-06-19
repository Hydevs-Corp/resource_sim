use noise::{NoiseFn, Perlin};
use rand::RngExt;
use std::collections::{HashSet, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

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
    Energy(u32),
    Crystal(u32),
    Metal(u32),
    Meat(u32),
    Base,
}
impl CellType {
    fn is_passable(self) -> bool {
        !matches!(self, CellType::Obstacle)
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

 

pub enum Message {
    Moved(usize, usize, usize),
    ResourceFound(usize, usize),
    ResourceCollected(usize, usize),
    Unloaded(u32, u32, u32, u32),
    EnemySpawned(usize, usize, usize),
    EnemyMoved(usize, usize, usize),
    AttackRobot(usize, i32),
    AttackEnemy(usize, i32),
    AttackBase(u32),
    MeteoriteImpact(usize, usize),
    MeteoriteIncoming(usize, usize, usize, usize),
}

pub struct Simulation {
    pub width: usize,
    pub height: usize,
    pub map: Arc<RwLock<Vec<Vec<CellType>>>>,
    pub robots: Arc<RwLock<Vec<RobotState>>>,
    pub enemies: Arc<RwLock<Vec<EnemyState>>>,
    pub fear_factor: f32,
    pub base_hp: i32,
    pub collected_crystals: u32,
    pub collected_meat: u32,
    pub collected_metal: u32,
    pub sender: Sender<Message>,
    pub cheat_mode: bool,
    pub meteorite_anims: Arc<RwLock<Vec<MeteoriteAnim>>>,
    pub meteorite_flights: Arc<RwLock<Vec<MeteoriteFlight>>>,
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
        let meteorite_flights: Arc<RwLock<Vec<MeteoriteFlight>>> = Arc::new(RwLock::new(Vec::new()));

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
                Self::spawn_scout(
                    i,
                    base_x,
                    base_y,
                    sender_clone,
                    Arc::clone(&map),
                    width,
                    height,
                );
            } else {
                Self::spawn_collector(
                    i,
                    base_x,
                    base_y,
                    sender_clone,
                    Arc::clone(&map),
                    Arc::clone(&known_resources),
                    Arc::clone(&claimed_resources),
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
                hp: 150,
            });
            Self::spawn_army(
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
                let edge = rng.random_range(0..4);
                let (ex, ey) = match edge {
                    0 => (rng.random_range(0..w), 0),
                    1 => (rng.random_range(0..w), h - 1),
                    2 => (0, rng.random_range(0..h)),
                    _ => (w - 1, rng.random_range(0..h)),
                };
                let _ = sender_spawner.send(Message::EnemySpawned(enemy_id, ex, ey));
                Self::spawn_enemy(
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
            base_hp: 1000,
            collected_crystals: 0,
            collected_meat: 0,
            collected_metal: 0,
            sender,
            receiver,
            known_resources,
            _claimed_resources: claimed_resources,
            cheat_mode: false,
            fear_factor: 0.5,
            meteorite_anims,
            meteorite_flights,
        }
    }

    fn spawn_scout(
        id: usize,
        start_x: usize,
        start_y: usize,
        sender: Sender<Message>,
        map: Arc<RwLock<Vec<Vec<CellType>>>>,
        width: usize,
        height: usize,
    ) {
        thread::spawn(move || {
            let mut rng = rand::rng();
            let mut x = start_x;
            let mut y = start_y;

            loop {
                thread::sleep(Duration::from_millis(rng.random_range(150..350)));

                {
                    let map_r = map.read().unwrap();
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            let nx = x as i32 + dx;
                            let ny = y as i32 + dy;
                            if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                                let cell = map_r[ny as usize][nx as usize];
                                if matches!(
                                    cell,
                                    CellType::Energy(_)
                                        | CellType::Crystal(_)
                                        | CellType::Metal(_)
                                        | CellType::Meat(_)
                                ) {
                                    let _ = sender
                                        .send(Message::ResourceFound(nx as usize, ny as usize));
                                }
                            }
                        }
                    }
                }

                let candidates: Vec<(usize, usize)> = {
                    let map_r = map.read().unwrap();
                    let mut c = Vec::new();
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            let nx = (x as i32 + dx).clamp(0, (width - 1) as i32) as usize;
                            let ny = (y as i32 + dy).clamp(0, (height - 1) as i32) as usize;
                            if map_r[ny][nx].is_passable() {
                                c.push((nx, ny));
                            }
                        }
                    }
                    c
                };

                if !candidates.is_empty() {
                    let (nx, ny) = candidates[rng.random_range(0..candidates.len())];
                    x = nx;
                    y = ny;
                }

                if sender.send(Message::Moved(id, x, y)).is_err() {
                    break;
                }
            }
        });
    }

    fn spawn_army(
        id: usize,
        start_x: usize,
        start_y: usize,
        sender: Sender<Message>,
        map: Arc<RwLock<Vec<Vec<CellType>>>>,
        enemies: Arc<RwLock<Vec<EnemyState>>>,
        robots: Arc<RwLock<Vec<RobotState>>>,
        width: usize,
        height: usize,
    ) {
        thread::spawn(move || {
            let mut rng = rand::rng();
            let mut x = start_x;
            let mut y = start_y;
            loop {
                thread::sleep(Duration::from_millis(200));

                let own_target = {
                    let en = enemies.read().unwrap();
                    en.iter()
                        .filter(|e| {
                            let dx = (e.x as isize - x as isize).abs() as usize;
                            let dy = (e.y as isize - y as isize).abs() as usize;
                            (dx * dx + dy * dy) as f64 <= 100.0
                        })
                        .min_by_key(|e| {
                            ((e.x as isize - x as isize).abs() + (e.y as isize - y as isize).abs())
                                as usize
                        })
                        .map(|e| (e.id, e.x, e.y))
                };

                if let Some((eid, ex, ey)) = own_target {
                    if x == ex && y == ey {
                        let _ = sender.send(Message::AttackEnemy(eid, 10));
                    } else {
                        let map_r = map.read().unwrap();
                        if let Some((nx, ny)) =
                            step_towards(&map_r, (x, y), (ex, ey), width, height)
                        {
                            x = nx;
                            y = ny;
                            let _ = sender.send(Message::Moved(id, x, y));
                        }
                    }
                    continue;
                }

                let other_target = {
                    let en = enemies.read().unwrap();
                    let robs = robots.read().unwrap();
                    en.iter()
                        .filter(|e| {
                            robs.iter().any(|r| {
                                let dx = (e.x as isize - r.x as isize).abs() as usize;
                                let dy = (e.y as isize - r.y as isize).abs() as usize;
                                (dx * dx + dy * dy) as f64 <= 100.0
                            })
                        })
                        .min_by_key(|e| {
                            ((e.x as isize - x as isize).abs() + (e.y as isize - y as isize).abs())
                                as usize
                        })
                        .map(|e| (e.id, e.x, e.y))
                };

                if let Some((eid, ex, ey)) = other_target {
                    if x == ex && y == ey {
                        let _ = sender.send(Message::AttackEnemy(eid, 10));
                    } else {
                        let map_r = map.read().unwrap();
                        if let Some((nx, ny)) =
                            step_towards(&map_r, (x, y), (ex, ey), width, height)
                        {
                            x = nx;
                            y = ny;
                            let _ = sender.send(Message::Moved(id, x, y));
                        }
                    }
                    continue;
                }

                let base = (width / 2, height / 2);
                if (x, y) != base {
                    let map_r = map.read().unwrap();
                    if let Some((nx, ny)) = step_towards(&map_r, (x, y), base, width, height) {
                        x = nx;
                        y = ny;
                        let _ = sender.send(Message::Moved(id, x, y));
                    }
                } else {
                    thread::sleep(Duration::from_millis(rng.random_range(100..300)));
                }
            }
        });
    }

    fn spawn_collector(
        id: usize,
        start_x: usize,
        start_y: usize,
        sender: Sender<Message>,
        map: Arc<RwLock<Vec<Vec<CellType>>>>,
        known_resources: Arc<RwLock<Vec<(usize, usize)>>>,
        claimed: Arc<RwLock<HashSet<(usize, usize)>>>,
        width: usize,
        height: usize,
    ) {
        thread::spawn(move || {
            let mut rng = rand::rng();
            let mut x = start_x;
            let mut y = start_y;
            let base = (start_x, start_y);
            let mut carrying_energy: u32 = 0;
            let mut carrying_crystals: u32 = 0;
            let mut carrying_metal: u32 = 0;
            let mut carrying_meat: u32 = 0;
            let mut target: Option<(usize, usize)> = None;
            let mut returning = false;

            loop {
                thread::sleep(Duration::from_millis(150));

                if returning {
                    if (x, y) == base {
                        let _ = sender.send(Message::Unloaded(
                            carrying_energy,
                            carrying_crystals,
                            carrying_metal,
                            carrying_meat,
                        ));
                        carrying_energy = 0;
                        carrying_crystals = 0;
                        carrying_metal = 0;
                        carrying_meat = 0;
                        returning = false;
                    } else {
                        let map_r = map.read().unwrap();
                        if let Some((nx, ny)) = step_towards(&map_r, (x, y), base, width, height) {
                            drop(map_r);
                            x = nx;
                            y = ny;
                        }
                    }
                } else {
                    if target.is_none() {
                        let found = {
                            let resources = known_resources.read().unwrap();
                            let map_r = map.read().unwrap();
                            let claimed_r = claimed.read().unwrap();
                            resources
                                .iter()
                                .find(|&&(rx, ry)| {
                                    matches!(
                                        map_r[ry][rx],
                                        CellType::Energy(_)
                                            | CellType::Crystal(_)
                                            | CellType::Metal(_)
                                            | CellType::Meat(_)
                                    ) && !claimed_r.contains(&(rx, ry))
                                })
                                .copied()
                        };
                        if let Some(t) = found {
                            claimed.write().unwrap().insert(t);
                            target = Some(t);
                        }
                    }

                    if let Some((tx, ty)) = target {
                        let cell = { map.read().unwrap()[ty][tx] };

                        match cell {
                            CellType::Energy(n) => {
                                if (x, y) == (tx, ty) {
                                    carrying_energy += n;
                                    claimed.write().unwrap().remove(&(tx, ty));
                                    let _ = sender.send(Message::ResourceCollected(tx, ty));
                                    target = None;
                                    returning = true;
                                } else {
                                    let map_r = map.read().unwrap();
                                    match step_towards(&map_r, (x, y), (tx, ty), width, height) {
                                        Some((nx, ny)) => {
                                            drop(map_r);
                                            x = nx;
                                            y = ny;
                                        }
                                        None => {
                                            drop(map_r);
                                            claimed.write().unwrap().remove(&(tx, ty));
                                            target = None;
                                        }
                                    }
                                }
                            }
                            CellType::Crystal(n) => {
                                if (x, y) == (tx, ty) {
                                    carrying_crystals += n;
                                    claimed.write().unwrap().remove(&(tx, ty));
                                    let _ = sender.send(Message::ResourceCollected(tx, ty));
                                    target = None;
                                    returning = true;
                                } else {
                                    let map_r = map.read().unwrap();
                                    match step_towards(&map_r, (x, y), (tx, ty), width, height) {
                                        Some((nx, ny)) => {
                                            drop(map_r);
                                            x = nx;
                                            y = ny;
                                        }
                                        None => {
                                            drop(map_r);
                                            claimed.write().unwrap().remove(&(tx, ty));
                                            target = None;
                                        }
                                    }
                                }
                            }
                            CellType::Metal(n) => {
                                if (x, y) == (tx, ty) {
                                    carrying_metal += n;
                                    claimed.write().unwrap().remove(&(tx, ty));
                                    let _ = sender.send(Message::ResourceCollected(tx, ty));
                                    target = None;
                                    returning = true;
                                } else {
                                    let map_r = map.read().unwrap();
                                    match step_towards(&map_r, (x, y), (tx, ty), width, height) {
                                        Some((nx, ny)) => {
                                            drop(map_r);
                                            x = nx;
                                            y = ny;
                                        }
                                        None => {
                                            drop(map_r);
                                            claimed.write().unwrap().remove(&(tx, ty));
                                            target = None;
                                        }
                                    }
                                }
                            }
                            CellType::Meat(n) => {
                                if (x, y) == (tx, ty) {
                                    carrying_meat += n;
                                    claimed.write().unwrap().remove(&(tx, ty));
                                    let _ = sender.send(Message::ResourceCollected(tx, ty));
                                    target = None;
                                    returning = true;
                                } else {
                                    let map_r = map.read().unwrap();
                                    match step_towards(&map_r, (x, y), (tx, ty), width, height) {
                                        Some((nx, ny)) => {
                                            drop(map_r);
                                            x = nx;
                                            y = ny;
                                        }
                                        None => {
                                            drop(map_r);
                                            claimed.write().unwrap().remove(&(tx, ty));
                                            target = None;
                                        }
                                    }
                                }
                            }
                            _ => {
                                claimed.write().unwrap().remove(&(tx, ty));
                                target = None;
                            }
                        }
                    } else {
                        if (x, y) == base {
                            let candidates: Vec<(usize, usize)> = {
                                let map_r = map.read().unwrap();
                                let mut c = Vec::new();
                                for dy in -1i32..=1 {
                                    for dx in -1i32..=1 {
                                        if dx == 0 && dy == 0 {
                                            continue;
                                        }
                                        let nx =
                                            (x as i32 + dx).clamp(0, (width - 1) as i32) as usize;
                                        let ny =
                                            (y as i32 + dy).clamp(0, (height - 1) as i32) as usize;
                                        if map_r[ny][nx].is_passable() {
                                            c.push((nx, ny));
                                        }
                                    }
                                }
                                c
                            };
                            if !candidates.is_empty() {
                                let (nx, ny) = candidates[rng.random_range(0..candidates.len())];
                                x = nx;
                                y = ny;
                            }
                        } else {
                            let map_r = map.read().unwrap();
                            if let Some((nx, ny)) =
                                step_towards(&map_r, (x, y), base, width, height)
                            {
                                drop(map_r);
                                if (nx, ny) != base {
                                    x = nx;
                                    y = ny;
                                }
                            }
                        }
                    }
                }

                if sender.send(Message::Moved(id, x, y)).is_err() {
                    break;
                }
            }
        });
    }

    fn spawn_enemy(
        id: usize,
        start_x: usize,
        start_y: usize,
        sender: Sender<Message>,
        map: Arc<RwLock<Vec<Vec<CellType>>>>,
        robots: Arc<RwLock<Vec<RobotState>>>,
        width: usize,
        height: usize,
    ) {
        thread::spawn(move || {
            let mut x = start_x;
            let mut y = start_y;
            let base = (width / 2, height / 2);
            loop {
                thread::sleep(Duration::from_millis(300));
                let mut target = None;
                let mut min_dist = 11.0;
                {
                    let robs = robots.read().unwrap();
                    for r in robs.iter() {
                        let dist = (((r.x as isize - x as isize).pow(2)
                            + (r.y as isize - y as isize).pow(2))
                            as f64)
                            .sqrt();
                        if dist <= 10.0 && dist < min_dist {
                            min_dist = dist;
                            target = Some((r.id, r.x, r.y));
                        }
                    }
                }
                if let Some((r_id, rx, ry)) = target {
                    if x == rx && y == ry {
                        let _ = sender.send(Message::AttackRobot(r_id, 10));
                    } else {
                        let map_r = map.read().unwrap();
                        if let Some((nx, ny)) =
                            step_towards(&map_r, (x, y), (rx, ry), width, height)
                        {
                            x = nx;
                            y = ny;
                            let _ = sender.send(Message::EnemyMoved(id, x, y));
                        }
                    }
                } else {
                    if (x, y) == base {
                        let _ = sender.send(Message::AttackBase(10));
                    } else {
                        let map_r = map.read().unwrap();
                        if let Some((nx, ny)) = step_towards(&map_r, (x, y), base, width, height) {
                            x = nx;
                            y = ny;
                            let _ = sender.send(Message::EnemyMoved(id, x, y));
                        }
                    }
                }
            }
        });
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
        // Create the specified number of random energy
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
        {
            let mut flights = self.meteorite_flights.write().unwrap();
            let mut arrived: Vec<(usize, usize)> = Vec::new();
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
                        for dy in -(METEORITE_IMPACT_RADIUS as isize)..=(METEORITE_IMPACT_RADIUS as isize) {
                            for dx in -(METEORITE_IMPACT_RADIUS as isize)..=(METEORITE_IMPACT_RADIUS as isize) {
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
                                if (ddx * ddx + ddy * ddy) > (METEORITE_IMPACT_RADIUS as f32).powf(2.0) {
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
                            // spawn only with a certain probability to reduce total resources
                            if rng.random_range(0..100) as u8 >= METEORITE_RESOURCE_SPAWN_CHANCE_PERCENT {
                                continue;
                            }
                            let dx = (nx as isize - cx as isize).abs() as f32;
                            let dy = (ny as isize - cy as isize).abs() as f32;
                            let eu = (dx * dx + dy * dy).sqrt();
                            let multiplier = if eu < 0.75 { 3 } else if eu < 1.75 { 2 } else { 1 };
                            let resource_type = rng.random_range(0..4);
                            let base_amount: u32 = rng.random_range(METEORITE_RESOURCE_BASE_MIN..=METEORITE_RESOURCE_BASE_MAX);
                            let amount = base_amount.saturating_mul(multiplier as u32);
                            map_w[ny][nx] = match resource_type {
                                0 => CellType::Crystal(amount),
                                1 => CellType::Energy(amount),
                                2 => CellType::Metal(amount),
                                _ => CellType::Meat(amount),
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
                        // spawn with limited probability
                        if rng.random_range(0..100) as u8 >= METEORITE_RESOURCE_SPAWN_CHANCE_PERCENT {
                            continue;
                        }
                        let dx = (x as isize - cx as isize).abs() as f32;
                        let dy = (y as isize - cy as isize).abs() as f32;
                        let eu = (dx * dx + dy * dy).sqrt();
                        let multiplier = if eu < 0.75 { 3 } else if eu < 1.75 { 2 } else { 1 };
                        let resource_type = rng.random_range(0..4);
                        let base_amount: u32 = rng.random_range(METEORITE_RESOURCE_BASE_MIN..=METEORITE_RESOURCE_BASE_MAX);
                        let amount = base_amount.saturating_mul(multiplier as u32);
                        map_w[y][x] = match resource_type {
                            0 => CellType::Crystal(amount),
                            1 => CellType::Energy(amount),
                            2 => CellType::Metal(amount),
                            _ => CellType::Meat(amount),
                        };
                    }
                }
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
                Message::ResourceCollected(x, y) => {
                    self.map.write().unwrap()[y][x] = CellType::Empty;
                    self.known_resources
                        .write()
                        .unwrap()
                        .retain(|&(rx, ry)| !(rx == x && ry == y));
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
                Message::AttackEnemy(id, damage) => {
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
                Message::MeteoriteImpact(x, y) => {
                    let mut anims = self.meteorite_anims.write().unwrap();
                    for dy in -(METEORITE_IMPACT_RADIUS as isize)..=(METEORITE_IMPACT_RADIUS as isize) {
                        for dx in -(METEORITE_IMPACT_RADIUS as isize)..=(METEORITE_IMPACT_RADIUS as isize) {
                            let nx_i = x as isize + dx;
                            let ny_i = y as isize + dy;
                            if nx_i < 0 || ny_i < 0 {
                                continue;
                            }
                            let nx = nx_i as usize;
                            let ny = ny_i as usize;
                            if nx >= self.width || ny >= self.height {
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
                                center_x: x,
                                center_y: y,
                            });
                        }
                    }
                }
                Message::AttackBase(damage) => {
                    self.base_hp = self.base_hp.saturating_sub(damage as i32);
                    self.fear_factor = self.fear_factor + 10.0;
                }
            }
        }
    }
}
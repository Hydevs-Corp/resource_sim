use super::*;
use std::thread;
use std::time::Duration;

pub fn spawn_scout(
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

        let dirs = [(0, -1), (1, 0), (0, 1), (-1, 0)];
        let mut dir_idx: i32 = rng.random_range(0..4);

        let mut rot_dir: i32 = if rng.random_bool(0.5) { 1 } else { -1 };

        let mut is_expanding = true;
        let mut step_limit = 1;
        let mut current_steps = 0;
        let mut segments_done = 0;

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
                                CellType::Energy(_) | CellType::Crystal(_) | CellType::Metal(_) | CellType::Meat(_)
                            ) {
                                let _ = sender.send(Message::ResourceFound(nx as usize, ny as usize));
                            }
                        }
                    }
                }
            }

            let (dx, dy) = dirs[dir_idx as usize];
            let ideal_nx = x as i32 + dx;
            let ideal_ny = y as i32 + dy;

            let hit_edge = ideal_nx < 0
                || ideal_nx >= width as i32
                || ideal_ny < 0
                || ideal_ny >= height as i32;

            if hit_edge {
                is_expanding = !is_expanding;

                rot_dir = if rng.random_bool(0.5) { 1 } else { -1 };

                dir_idx = (dir_idx + rot_dir + 4) % 4;
                current_steps = 0;
                segments_done = 0;
            } else {
                let mut moved_x = x;
                let mut moved_y = y;

                {
                    let map_r = map.read().unwrap();
                    if map_r[ideal_ny as usize][ideal_nx as usize].is_passable() {
                        moved_x = ideal_nx as usize;
                        moved_y = ideal_ny as usize;
                    } else {
                        let mut best_dist = i32::MAX;
                        let mut best_pos = None;

                        for test_dy in -1i32..=1 {
                            for test_dx in -1i32..=1 {
                                if test_dx == 0 && test_dy == 0 {
                                    continue;
                                }
                                let nx = x as i32 + test_dx;
                                let ny = y as i32 + test_dy;

                                if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                                    if map_r[ny as usize][nx as usize].is_passable() {
                                        let dist = (nx - ideal_nx).abs() + (ny - ideal_ny).abs();
                                        if dist < best_dist {
                                            best_dist = dist;
                                            best_pos = Some((nx as usize, ny as usize));
                                        }
                                    }
                                }
                            }
                        }

                        if let Some((bx, by)) = best_pos {
                            moved_x = bx;
                            moved_y = by;
                        }
                    }
                }

                x = moved_x;
                y = moved_y;
                current_steps += 1;

                if current_steps >= step_limit {
                    current_steps = 0;
                    segments_done += 1;

                    dir_idx = (dir_idx + rot_dir + 4) % 4;

                    if segments_done >= 2 {
                        segments_done = 0;
                        if is_expanding {
                            step_limit += 1;
                        } else {
                            step_limit -= 1;

                            if step_limit <= 0 {
                                is_expanding = true;
                                step_limit = 1;

                                rot_dir = if rng.random_bool(0.5) { 1 } else { -1 };
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

pub fn spawn_army(
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
                    let _ = sender.send(Message::AttackEnemy(eid, 10, true));
                } else {
                    let map_r = map.read().unwrap();
                    if let Some((nx, ny)) =
                        super::step_towards(&map_r, (x, y), (ex, ey), width, height)
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
                    let _ = sender.send(Message::AttackEnemy(eid, 10, true));
                } else {
                    let map_r = map.read().unwrap();
                    if let Some((nx, ny)) =
                        super::step_towards(&map_r, (x, y), (ex, ey), width, height)
                    {
                        x = nx;
                        y = ny;
                        let _ = sender.send(Message::Moved(id, x, y));
                    }
                }
                continue;
            }

            let base_x = width / 2;
            let base_y = height / 2;

            let map_r = map.read().unwrap();

            let mut valid_posts = Vec::new();

            let offsets = [
                (2, 0),
                (-2, 0),
                (0, 2),
                (0, -2),
                (2, 2),
                (-2, -2),
                (2, -2),
                (-2, 2),
            ];

            for (dx, dy) in offsets.iter() {
                let gx = base_x as i32 + *dx;
                let gy = base_y as i32 + *dy;

                if gx >= 0 && gx < width as i32 && gy >= 0 && gy < height as i32 {
                    let gx = gx as usize;
                    let gy = gy as usize;

                    if map_r[gy][gx].is_passable() {
                        valid_posts.push((gx, gy));
                    }
                }
            }

            let guard_post = if !valid_posts.is_empty() {
                valid_posts[id % valid_posts.len()]
            } else {
                (base_x, base_y)
            };

            if (x, y) != guard_post {
                if let Some((nx, ny)) = super::step_towards(&map_r, (x, y), guard_post, width, height)
                {
                    drop(map_r);
                    x = nx;
                    y = ny;
                    let _ = sender.send(Message::Moved(id, x, y));
                }
            } else {
                drop(map_r);
                thread::sleep(Duration::from_millis(rng.random_range(100..300)));
            }
        }
    });
}

pub fn spawn_collector(
    id: usize,
    start_x: usize,
    start_y: usize,
    sender: Sender<Message>,
    map: Arc<RwLock<Vec<Vec<CellType>>>>,
    known_resources: Arc<RwLock<Vec<(usize, usize)>>>,
    claimed: Arc<RwLock<HashSet<(usize, usize)>>>,
    shared_fear: Arc<RwLock<f32>>,
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
                    if let Some((nx, ny)) = super::step_towards(&map_r, (x, y), base, width, height) {
                        drop(map_r);
                        x = nx;
                        y = ny;
                    }
                }
            } else {
                if target.is_none() {
                    let fear = *shared_fear.read().unwrap();

                    let (army_p, col_p, scout_p) = if fear <= 20.0 {
                        (3, 2, 1)
                    } else if fear <= 50.0 {
                        (2, 1, 3)
                    } else if fear <= 70.0 {
                        (1, 3, 2)
                    } else {
                        (1, 2, 3)
                    };

                    let get_prio = |cell: CellType| -> i32 {
                        match cell {
                            CellType::Crystal(_) => scout_p.min(col_p),
                            CellType::Metal(_) | CellType::Meat(_) => army_p,
                            CellType::Energy(_) => 1,
                            _ => 99,
                        }
                    };

                    let found = {
                        let resources = known_resources.read().unwrap();
                        let map_r = map.read().unwrap();
                        let claimed_r = claimed.read().unwrap();
                        resources
                            .iter()
                            .filter(|&&(rx, ry)| {
                                matches!(
                                    map_r[ry][rx],
                                    CellType::Energy(_) | CellType::Crystal(_) | CellType::Metal(_) | CellType::Meat(_)
                                ) && !claimed_r.contains(&(rx, ry))
                            })
                            .min_by_key(|&&(rx, ry)| {
                                let cell = map_r[ry][rx];
                                let prio = get_prio(cell);
                                let dist = (rx as i32 - x as i32).abs() + (ry as i32 - y as i32).abs();
                                (prio, dist)
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
                                let take = (50u32).min(n);
                                carrying_energy += take;
                                if sender.send(Message::ResourceCollected(tx, ty, take)).is_err() {
                                    eprintln!("collector {}: receiver closed while sending ResourceCollected", id);
                                    break;
                                }
                                claimed.write().unwrap().remove(&(tx, ty));
                                target = None;
                                returning = true;
                            } else {
                                let map_r = map.read().unwrap();
                                match super::step_towards(&map_r, (x, y), (tx, ty), width, height) {
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
                                let take = (50u32).min(n);
                                carrying_crystals += take;
                                if sender.send(Message::ResourceCollected(tx, ty, take)).is_err() {
                                    eprintln!("collector {}: receiver closed while sending ResourceCollected", id);
                                    break;
                                }
                                target = None;
                                returning = true;
                            } else {
                                let map_r = map.read().unwrap();
                                match super::step_towards(&map_r, (x, y), (tx, ty), width, height) {
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
                                let take = (50u32).min(n);
                                carrying_metal += take;
                                if sender.send(Message::ResourceCollected(tx, ty, take)).is_err() {
                                    eprintln!("collector {}: receiver closed while sending ResourceCollected", id);
                                    break;
                                }
                                target = None;
                                returning = true;
                            } else {
                                let map_r = map.read().unwrap();
                                match super::step_towards(&map_r, (x, y), (tx, ty), width, height) {
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
                                let take = (50u32).min(n);
                                carrying_meat += take;
                                if sender.send(Message::ResourceCollected(tx, ty, take)).is_err() {
                                    eprintln!("collector {}: receiver closed while sending ResourceCollected", id);
                                    break;
                                }
                                target = None;
                                returning = true;
                            } else {
                                let map_r = map.read().unwrap();
                                match super::step_towards(&map_r, (x, y), (tx, ty), width, height) {
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
                        if let Some((nx, ny)) = super::step_towards(&map_r, (x, y), base, width, height) {
                            drop(map_r);
                            if (nx, ny) != base {
                                x = nx;
                                y = ny;
                            }
                        }
                    }
                }

                    if sender.send(Message::Moved(id, x, y)).is_err() {
                        eprintln!("collector {}: receiver closed while sending Moved, exiting thread", id);
                        break;
                    }
            }
        }
    });
}

pub fn spawn_enemy(
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
                    if let Some((nx, ny)) = super::step_towards(&map_r, (x, y), (rx, ry), width, height) {
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
                    let bx = base.0 as isize;
                    let by = base.1 as isize;
                    let doors = [
                        (bx, by - super::BASE_WALL_RADIUS as isize),
                        (bx + super::BASE_WALL_RADIUS as isize, by),
                        (bx, by + super::BASE_WALL_RADIUS as isize),
                        (bx - super::BASE_WALL_RADIUS as isize, by),
                    ];
                    let mut nearest: Option<(usize, usize)> = None;
                    let mut ndist = usize::MAX;
                    for (dx, dy) in doors.iter() {
                        if *dx < 0 || *dy < 0 || *dx >= width as isize || *dy >= height as isize {
                            continue;
                        }
                        let (dxu, dyu) = (*dx as usize, *dy as usize);
                        if matches!(map_r[dyu][dxu], CellType::Door(_) | CellType::Wall(_)) {
                            let d = ((dxu as isize - x as isize).abs() + (dyu as isize - y as isize).abs()) as usize;
                            if d < ndist {
                                ndist = d;
                                nearest = Some((dxu, dyu));
                            }
                        }
                    }
                    if let Some((tx, ty)) = nearest {
                        if (x, y) == (tx, ty) {
                            let _ = sender.send(Message::AttackDoor(tx, ty, 10));
                        } else {
                            if let Some((nx, ny)) = super::step_towards(&map_r, (x, y), (tx, ty), width, height) {
                                x = nx;
                                y = ny;
                                let _ = sender.send(Message::EnemyMoved(id, x, y));
                            }
                        }
                    } else {
                        if let Some((nx, ny)) = super::step_towards(&map_r, (x, y), base, width, height) {
                            x = nx;
                            y = ny;
                            let _ = sender.send(Message::EnemyMoved(id, x, y));
                        }
                    }
                }
            }
        }
    });
}

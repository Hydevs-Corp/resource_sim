mod simulation;
mod ui;
mod server;
use std::collections::VecDeque;

use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
};
use simulation::{Simulation, config::SimulationConfig, state::GameState};
use std::{
    error::Error,
    fs, io,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    server: bool,
    #[arg(long)]
    port: Option<u16>,
    #[arg(long)]
    new: bool,
    #[arg(long)]
    resume: Option<String>,
    #[arg(long)]
    base_hp: Option<i32>,
    #[arg(long)]
    robot_hp: Option<i32>,
    #[arg(long)]
    enemy_hp: Option<i32>,
    #[arg(long)]
    enemy_spawn_speed_ms: Option<u64>,
    #[arg(long)]
    collector_capacity: Option<u32>,
    #[arg(long)]
    army_cost_metal: Option<u32>,
    #[arg(long)]
    army_cost_meat: Option<u32>,
    #[arg(long)]
    width: Option<usize>,
    #[arg(long)]
    height: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut config = SimulationConfig::default();
    if let Some(v) = args.base_hp {
        config.base_hp = v;
    }
    if let Some(v) = args.robot_hp {
        config.robot_hp = v;
    }
    if let Some(v) = args.enemy_hp {
        config.enemy_hp = v;
    }
    if let Some(v) = args.enemy_spawn_speed_ms {
        config.enemy_spawn_speed_ms = v;
    }
    if let Some(v) = args.collector_capacity {
        config.collector_capacity = v;
    }
    if let Some(v) = args.army_cost_metal {
        config.army_cost_metal = v;
    }
    if let Some(v) = args.army_cost_meat {
        config.army_cost_meat = v;
    }
    if let Some(v) = args.width {
        config.width = v;
    }
    if let Some(v) = args.height {
        config.height = v;
    }

    if args.server {
        let port = args.port.unwrap_or(3000);
        let sim = if args.new {
            Simulation::new(config.width, config.height, config)
        } else if let Some(path) = &args.resume {
            if let Ok(data) = fs::read_to_string(path) {
                if let Ok(state) = serde_json::from_str::<GameState>(&data) {
                    Simulation::from_state(state)
                } else {
                    Simulation::new(config.width, config.height, config)
                }
            } else {
                Simulation::new(config.width, config.height, config)
            }
        } else {
            Simulation::new(config.width, config.height, config)
        };
        
        let sim_arc = std::sync::Arc::new(std::sync::Mutex::new(sim));
        let sim_clone = sim_arc.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(50));
            loop {
                interval.tick().await;
                let mut sim_lock = sim_clone.lock().unwrap();
                if sim_lock.base_hp > 0 {
                    sim_lock.update();
                }
            }
        });

        server::run_server(sim_arc, port).await;
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut sim: Option<Simulation> = None;

    if args.new {
        sim = Some(Simulation::new(config.width, config.height, config));
    } else if let Some(path) = args.resume {
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(state) = serde_json::from_str::<GameState>(&data) {
                sim = Some(Simulation::from_state(state));
            }
        }
    }

    if sim.is_none() {
        sim = show_home_screen(&mut terminal, config)?;
    }

    let mut res = Ok(());
    if let Some(mut simulation) = sim {
        res = run_app(&mut terminal, &mut simulation);
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn show_home_screen(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut config: SimulationConfig,
) -> Result<Option<Simulation>, Box<dyn Error>> {
    let items = vec![
        "1. Create New Simulation",
        "2. Resume Simulation",
        "3. Quit",
    ];
    let mut state = ListState::default();
    state.select(Some(0));

    let mut mode = 0;
    let mut save_files: Vec<String> = Vec::new();
    let mut save_state = ListState::default();
    
    let mut config_state = ListState::default();
    config_state.select(Some(0));

    loop {
        terminal.draw(|f| {
            let size = f.area();
            let block = Block::default()
                .title(if mode == 0 { "Resource Sim - Home" } else if mode == 1 { "Select Save Game" } else { "Configure New Game (Left/Right to change)" })
                .borders(Borders::ALL);

            if mode == 0 {
                let list = List::new(items.clone())
                    .block(block)
                    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                    .highlight_symbol(">> ");
                f.render_stateful_widget(list, size, &mut state);
            } else if mode == 1 {
                let mut display_items = Vec::new();
                for file in &save_files {
                    display_items.push(ListItem::new(file.as_str()));
                }
                if display_items.is_empty() {
                    display_items.push(ListItem::new("No saves found. Press ESC to go back."));
                }
                let list = List::new(display_items)
                    .block(block)
                    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                    .highlight_symbol(">> ");
                f.render_stateful_widget(list, size, &mut save_state);
            } else if mode == 2 {
                let config_items = vec![
                    ListItem::new(format!("Base HP: {}", config.base_hp)),
                    ListItem::new(format!("Robot HP: {}", config.robot_hp)),
                    ListItem::new(format!("Enemy HP: {}", config.enemy_hp)),
                    ListItem::new(format!("Enemy Spawn Speed (ms): {}", config.enemy_spawn_speed_ms)),
                    ListItem::new(format!("Collector Capacity: {}", config.collector_capacity)),
                    ListItem::new(format!("Army Cost Metal: {}", config.army_cost_metal)),
                    ListItem::new(format!("Army Cost Meat: {}", config.army_cost_meat)),
                    ListItem::new(format!("Map Width: {}", config.width)),
                    ListItem::new(format!("Map Height: {}", config.height)),
                    ListItem::new("[ Start Simulation ]"),
                ];
                let list = List::new(config_items)
                    .block(block)
                    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                    .highlight_symbol(">> ");
                f.render_stateful_widget(list, size, &mut config_state);
            }
        })?;

        if let Event::Key(key) = event::read()? {
            if mode == 0 {
                match key.code {
                    KeyCode::Down => {
                        let i = match state.selected() {
                            Some(i) => if i >= items.len() - 1 { 0 } else { i + 1 },
                            None => 0,
                        };
                        state.select(Some(i));
                    }
                    KeyCode::Up => {
                        let i = match state.selected() {
                            Some(i) => if i == 0 { items.len() - 1 } else { i - 1 },
                            None => 0,
                        };
                        state.select(Some(i));
                    }
                    KeyCode::Enter => match state.selected() {
                        Some(0) => {
                            mode = 2;
                            config_state.select(Some(0));
                        }
                        Some(1) => {
                            mode = 1;
                            save_files.clear();
                            if let Ok(entries) = fs::read_dir(".") {
                                for entry in entries.flatten() {
                                    if let Ok(file_type) = entry.file_type() {
                                        if file_type.is_file() {
                                            let name = entry.file_name().to_string_lossy().to_string();
                                            if name.ends_with(".json") {
                                                save_files.push(name);
                                            }
                                        }
                                    }
                                }
                            }
                            if !save_files.is_empty() {
                                save_state.select(Some(0));
                            }
                        }
                        Some(2) => return Ok(None),
                        _ => {}
                    },
                    _ => {}
                }
            } else if mode == 1 {
                match key.code {
                    KeyCode::Esc => mode = 0,
                    KeyCode::Down => {
                        if !save_files.is_empty() {
                            let i = match save_state.selected() {
                                Some(i) => if i >= save_files.len() - 1 { 0 } else { i + 1 },
                                None => 0,
                            };
                            save_state.select(Some(i));
                        }
                    }
                    KeyCode::Up => {
                        if !save_files.is_empty() {
                            let i = match save_state.selected() {
                                Some(i) => if i == 0 { save_files.len() - 1 } else { i - 1 },
                                None => 0,
                            };
                            save_state.select(Some(i));
                        }
                    }
                    KeyCode::Enter => {
                        if !save_files.is_empty() {
                            if let Some(i) = save_state.selected() {
                                let path = &save_files[i];
                                if let Ok(data) = fs::read_to_string(path) {
                                    if let Ok(state) = serde_json::from_str::<GameState>(&data) {
                                        return Ok(Some(Simulation::from_state(state)));
                                    }
                                }
                            }
                        }
                        mode = 0;
                    }
                    _ => {}
                }
            } else if mode == 2 {
                match key.code {
                    KeyCode::Esc => mode = 0,
                    KeyCode::Down => {
                        let i = match config_state.selected() {
                            Some(i) => if i >= 9 { 0 } else { i + 1 },
                            None => 0,
                        };
                        config_state.select(Some(i));
                    }
                    KeyCode::Up => {
                        let i = match config_state.selected() {
                            Some(i) => if i == 0 { 9 } else { i - 1 },
                            None => 0,
                        };
                        config_state.select(Some(i));
                    }
                    KeyCode::Left => match config_state.selected() {
                        Some(0) => config.base_hp = config.base_hp.saturating_sub(100).max(100),
                        Some(1) => config.robot_hp = config.robot_hp.saturating_sub(10).max(10),
                        Some(2) => config.enemy_hp = config.enemy_hp.saturating_sub(5).max(5),
                        Some(3) => config.enemy_spawn_speed_ms = config.enemy_spawn_speed_ms.saturating_sub(500).max(500),
                        Some(4) => config.collector_capacity = config.collector_capacity.saturating_sub(10).max(10),
                        Some(5) => config.army_cost_metal = config.army_cost_metal.saturating_sub(10).max(10),
                        Some(6) => config.army_cost_meat = config.army_cost_meat.saturating_sub(5).max(5),
                        Some(7) => config.width = config.width.saturating_sub(10).max(20),
                        Some(8) => config.height = config.height.saturating_sub(10).max(10),
                        _ => {}
                    },
                    KeyCode::Right => match config_state.selected() {
                        Some(0) => config.base_hp += 100,
                        Some(1) => config.robot_hp += 10,
                        Some(2) => config.enemy_hp += 5,
                        Some(3) => config.enemy_spawn_speed_ms += 500,
                        Some(4) => config.collector_capacity += 10,
                        Some(5) => config.army_cost_metal += 10,
                        Some(6) => config.army_cost_meat += 5,
                        Some(7) => config.width = (config.width + 10).min(200),
                        Some(8) => config.height = (config.height + 10).min(200),
                        _ => {}
                    },
                    KeyCode::Enter => {
                        if config_state.selected() == Some(9) {
                            return Ok(Some(Simulation::new(config.width, config.height, config)));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    sim: &mut Simulation,
) -> io::Result<()> {
    let mut scroll_x: usize = 0;
    let mut scroll_y: usize = 0;
    let mut export_status: Option<String> = None;

    let mut last_pressed_keys: VecDeque<crossterm::event::KeyEvent> = VecDeque::with_capacity(10);
    let mut paused = false;

    loop {
        let max_scroll = max_scroll_offsets(terminal, sim)?;
        scroll_x = scroll_x.min(max_scroll.0);
        scroll_y = scroll_y.min(max_scroll.1);

        if !paused {
            sim.update();
        }

        terminal.draw(|f| ui::draw(f, sim, scroll_x, scroll_y, export_status.as_deref()))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key_event) = event::read()? {
                if key_event.kind == crossterm::event::KeyEventKind::Press {
                    last_pressed_keys.push_front(key_event);

                    while last_pressed_keys.len() > 10 {
                        last_pressed_keys.pop_back();
                    }
                    if last_pressed_keys.len() >= 10 {
                        let konami_code = [
                            crossterm::event::KeyCode::Char('a'),
                            crossterm::event::KeyCode::Char('b'),
                            crossterm::event::KeyCode::Right,
                            crossterm::event::KeyCode::Left,
                            crossterm::event::KeyCode::Right,
                            crossterm::event::KeyCode::Left,
                            crossterm::event::KeyCode::Down,
                            crossterm::event::KeyCode::Down,
                            crossterm::event::KeyCode::Up,
                            crossterm::event::KeyCode::Up,
                        ];

                        let pressed_keys: Vec<crossterm::event::KeyCode> =
                            last_pressed_keys.iter().map(|k| k.code).collect();

                        if pressed_keys == konami_code {
                            sim.cheat_mode = true;
                            last_pressed_keys.clear();
                        }
                    }
                }

                match key_event.code {
                    crossterm::event::KeyCode::Char('c') => {
                        if sim.cheat_mode {
                            sim.create_random_crystals(50);
                        }
                    }
                    crossterm::event::KeyCode::Char('e') => {
                        if sim.cheat_mode {
                            sim.create_random_energy(50);
                        }
                    }
                    crossterm::event::KeyCode::Char('q') => return Ok(()),
                    crossterm::event::KeyCode::Char('h') => {
                        scroll_x = sim.width;
                        scroll_y = sim.height;
                    }
                    crossterm::event::KeyCode::Char('s') => {
                        let timestamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|duration| duration.as_secs())
                            .unwrap_or_default();
                        let file_name = format!("savegame_{}.json", timestamp);
                        let state = sim.to_state();
                        if let Ok(json) = serde_json::to_string(&state) {
                            if fs::write(&file_name, json).is_ok() {
                                export_status = Some(format!("Saved: {}", file_name));
                            } else {
                                export_status = Some("Save failed".to_string());
                            }
                        }
                    }
                    crossterm::event::KeyCode::F(3) => {
                        let timestamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|duration| duration.as_secs())
                            .unwrap_or_default();
                        let file_name = format!("resource_sim_map_{}.md", timestamp);

                        match fs::write(&file_name, sim.export_markdown()) {
                            Ok(_) => {
                                export_status = Some(format!("Export .md: {}", file_name));
                            }
                            Err(err) => {
                                export_status = Some(format!("Export failed: {}", err));
                            }
                        }
                    }

                    crossterm::event::KeyCode::Left => {
                        if key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::SHIFT)
                        {
                            scroll_x = scroll_x.saturating_sub(5)
                        } else {
                            scroll_x = scroll_x.saturating_sub(1)
                        }
                    }
                    crossterm::event::KeyCode::Right => {
                        if key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::SHIFT)
                        {
                            scroll_x = scroll_x.saturating_add(5)
                        } else {
                            scroll_x = scroll_x.saturating_add(1)
                        }
                    }
                    crossterm::event::KeyCode::Up => {
                        if key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::SHIFT)
                        {
                            scroll_y = scroll_y.saturating_sub(5)
                        } else {
                            scroll_y = scroll_y.saturating_sub(1)
                        }
                    }
                    crossterm::event::KeyCode::Down => {
                        if key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::SHIFT)
                        {
                            scroll_y = scroll_y.saturating_add(5)
                        } else {
                            scroll_y = scroll_y.saturating_add(1)
                        }
                    }
                    crossterm::event::KeyCode::F(1) => {
                        sim.selected_font = &simulation::DEFAULT_FONT;
                    }
                    crossterm::event::KeyCode::F(2) => {
                        sim.selected_font = &simulation::NERD_FONT;
                    }
                    _ => {}
                }
            }

            if !paused && sim.base_hp <= 0 {
                paused = true;
            }
        }
    }

    fn max_scroll_offsets(
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        sim: &Simulation,
    ) -> io::Result<(usize, usize)> {
        let area = terminal.size()?;
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(area.into());

        let map_width = chunks[0].width.saturating_sub(2) as usize;
        let map_height = chunks[0].height.saturating_sub(2) as usize;

        Ok((
            sim.width.saturating_sub(map_width),
            sim.height.saturating_sub(map_height),
        ))
    }
}

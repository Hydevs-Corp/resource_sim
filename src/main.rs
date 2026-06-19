mod simulation;
mod ui;
use std::collections::VecDeque;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
};
use simulation::Simulation;
use std::{error::Error, io, time::Duration};

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut sim = Simulation::new(80, 40);

    let res = run_app(&mut terminal, &mut sim);

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

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    sim: &mut Simulation,
) -> io::Result<()> {
    let mut scroll_x: usize = 0;
    let mut scroll_y: usize = 0;

    let mut last_pressed_keys: VecDeque<crossterm::event::KeyEvent> = VecDeque::with_capacity(10);
    let mut paused = false;

    loop {
        let max_scroll = max_scroll_offsets(terminal, sim)?;
        scroll_x = scroll_x.min(max_scroll.0);
        scroll_y = scroll_y.min(max_scroll.1);

        if !paused {
            sim.update();
        }

        terminal.draw(|f| ui::draw(f, sim, scroll_x, scroll_y))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key_event) = event::read()? {
                if key_event.kind == crossterm::event::KeyEventKind::Press {
                    last_pressed_keys.push_front(key_event);

                    while last_pressed_keys.len() > 10 {
                        last_pressed_keys.pop_back();
                    }
                    // [Char('a'), Char('b'), Right, Left, Right, Left, Down, Down, Up, Up]
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

                // --- Ton code de match habituel ---
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
                    // f1 for default font, f2 for nerd font
                    crossterm::event::KeyCode::F(1) => {
                        sim.selected_font = &simulation::DEFAULT_FONT;
                    }
                    crossterm::event::KeyCode::F(2) => {
                        sim.selected_font = &simulation::NERD_FONT;
                    }
                    _ => {}
                }
            }

            // Enter paused state when base HP reaches 0
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

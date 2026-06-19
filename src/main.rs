mod simulation;
mod ui;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use simulation::Simulation;
use std::{error::Error, io, time::Duration};

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut sim = Simulation::new(800, 400);

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

    loop {
        let max_scroll = max_scroll_offsets(terminal, sim)?;
        scroll_x = scroll_x.min(max_scroll.0);
        scroll_y = scroll_y.min(max_scroll.1);

        sim.update();

        terminal.draw(|f| ui::draw(f, sim, scroll_x, scroll_y))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key_event) = event::read()? {
                match key_event.code {
                    crossterm::event::KeyCode::Char('c') => sim.create_random_crystals(),
                    crossterm::event::KeyCode::Char('e') => sim.create_random_energy(),
                    crossterm::event::KeyCode::Char('q') => return Ok(()),
                    crossterm::event::KeyCode::Char('h') => { // h to go to the base
                        scroll_x = sim.width;
                        scroll_y = sim.height;
                        
                    },

                    // if maj is pressed, move faster
                    crossterm::event::KeyCode::Left => {
                        if key_event.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                            scroll_x = scroll_x.saturating_sub(5)
                        } else {
                            scroll_x = scroll_x.saturating_sub(1)
                        }
                    },
                    crossterm::event::KeyCode::Right => {
                        if key_event.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                            scroll_x = scroll_x.saturating_add(5)
                        } else {
                            scroll_x = scroll_x.saturating_add(1)
                        }
                    },
                    crossterm::event::KeyCode::Up => {
                        if key_event.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                            scroll_y = scroll_y.saturating_sub(5)
                        } else {
                            scroll_y = scroll_y.saturating_sub(1)
                        }
                    },
                    crossterm::event::KeyCode::Down => {
                        if key_event.modifiers.contains(crossterm::event::KeyModifiers::SHIFT) {
                            scroll_y = scroll_y.saturating_add(5)
                        } else {
                            scroll_y = scroll_y.saturating_add(1)
                        }
                    },
                    _ => {}
                }
            }
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

    Ok((sim.width.saturating_sub(map_width), sim.height.saturating_sub(map_height)))
}
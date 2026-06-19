mod simulation;
mod ui;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
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
    loop {
        sim.update();

        terminal.draw(|f| ui::draw(f, sim))?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key_event) = event::read()? {
                if key_event.code == crossterm::event::KeyCode::Char('c') {
                    sim.create_random_crystals();
                }
                if key_event.code == crossterm::event::KeyCode::Char('e') {
                    sim.create_random_energy();
                }
                if key_event.code == crossterm::event::KeyCode::Char('q') {
                    return Ok(());
                }
            }
        }
    }
}
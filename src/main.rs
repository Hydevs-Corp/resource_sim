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
    // Configuration du terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Initialisation de la simulation
    let mut sim = Simulation::new(80, 40);

    // Boucle principale
    let res = run_app(&mut terminal, &mut sim);

    // Restauration du terminal
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
        // Mise à jour de l'état asynchrone (réception des messages des robots)
        sim.update();

        // Rendu UI
        terminal.draw(|f| ui::draw(f, sim))?;

        // Gestion des événements clavier avec un timeout pour ne pas bloquer la simulation
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
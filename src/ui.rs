use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::simulation::{CellType, RobotType, Simulation};

pub fn draw(f: &mut Frame, sim: &Simulation) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(f.area());

    let map = sim.map.read().unwrap();

    let mut map_lines = Vec::new();
    for y in 0..sim.height {
        let mut row_spans = Vec::new();
        for x in 0..sim.width {
            let robot_here = sim.robots.iter().find(|r| r.x == x && r.y == y);

            let (symbol, color) = if let Some(robot) = robot_here {
                match robot.r_type {
                    RobotType::Scout => ("x", Color::Red),
                    RobotType::Collector => ("o", Color::Magenta),
                }
            } else {
                match map[y][x] {
                    CellType::Empty => (" ", Color::Reset),
                    CellType::Obstacle => ("O", Color::LightCyan),
                    CellType::Energy(_) => ("E", Color::Green),
                    CellType::Crystal(_) => ("C", Color::LightMagenta),
                    CellType::Base => ("#", Color::LightGreen),
                }
            };
            row_spans.push(Span::styled(symbol, Style::default().fg(color)));
        }
        map_lines.push(Line::from(row_spans));
    }

    let map_paragraph = Paragraph::new(map_lines)
        .block(Block::default().borders(Borders::ALL).title("Simulation de Collecte"));
    f.render_widget(map_paragraph, chunks[0]);

    let stats = format!(
        " Énergie: {} | Cristaux: {} | [toute touche] Quitter ",
        sim.collected_energy, sim.collected_crystals
    );
    let ui_paragraph = Paragraph::new(stats)
        .block(Block::default().borders(Borders::ALL).title("Tableau de bord"))
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(ui_paragraph, chunks[1]);
}

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use crate::simulation::{CellType, RobotType, Simulation};

pub fn draw(f: &mut Frame, sim: &Simulation, scroll_x: usize, scroll_y: usize) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(f.area());

    let map = sim.map.read().unwrap();
    let map_block = Block::default().borders(Borders::ALL).title("Simulation de Collecte");
    let map_inner = map_block.inner(chunks[0]);
    f.render_widget(map_block, chunks[0]);

    let map_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(map_inner);

    let map_content_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(map_split[0]);

    let content_area = map_content_split[0];
    let vertical_scrollbar_area = map_content_split[1];
    let horizontal_scrollbar_area = map_split[1];

    let visible_width = content_area.width as usize;
    let visible_height = map_split[0].height as usize;
    let max_scroll_x = sim.width.saturating_sub(visible_width);
    let max_scroll_y = sim.height.saturating_sub(visible_height);
    let scroll_x = scroll_x.min(max_scroll_x);
    let scroll_y = scroll_y.min(max_scroll_y);

    let mut map_lines = Vec::new();
    for y in scroll_y..(scroll_y + visible_height).min(sim.height) {
        let mut row_spans = Vec::new();
        for x in scroll_x..(scroll_x + visible_width).min(sim.width) {
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

    let map_paragraph = Paragraph::new(map_lines);
    f.render_widget(map_paragraph, content_area);

    let vertical_scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
    let mut vertical_state = ScrollbarState::new(sim.height).position(scroll_y);
    f.render_stateful_widget(vertical_scrollbar, vertical_scrollbar_area, &mut vertical_state);

    let horizontal_scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::HorizontalBottom);
    let mut horizontal_state = ScrollbarState::new(sim.width).position(scroll_x);
    f.render_stateful_widget(horizontal_scrollbar, horizontal_scrollbar_area, &mut horizontal_state);

    let stats = format!(
        " Énergie: {} | Cristaux: {} | Flèches: déplacer | [q] Quitter ",
        sim.collected_energy, sim.collected_crystals
    );
    let ui_paragraph = Paragraph::new(stats)
        .block(Block::default().borders(Borders::ALL).title("Tableau de bord"))
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(ui_paragraph, chunks[1]);
}

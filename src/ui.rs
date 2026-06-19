use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
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

    let robots_lock = sim.robots.read().unwrap();
    let mut map_lines = Vec::new();
    for y in scroll_y..(scroll_y + visible_height).min(sim.height) {
        let mut row_spans = Vec::new();
        for x in scroll_x..(scroll_x + visible_width).min(sim.width) {
            let robot_here = robots_lock.iter().find(|r| r.x == x && r.y == y);
            let enemies_lock = sim.enemies.read().unwrap();
            let enemy_here = enemies_lock.iter().find(|e| e.x == x && e.y == y);

            let (symbol, color) = if let Some(_) = enemy_here {
                ("V", Color::LightRed)
            } else if let Some(robot) = robot_here {
                match robot.r_type {
                    RobotType::Scout => ("x", Color::Red),
                    RobotType::Collector => ("o", Color::Magenta),
                    RobotType::Army => ("A", Color::LightYellow),
                }
            } else {
                match map[y][x] {
                    CellType::Empty => (" ", Color::Reset),
                    CellType::Obstacle => ("O", Color::LightCyan),
                    CellType::Energy(_) => ("E", Color::Green),
                    CellType::Crystal(_) => ("C", Color::LightMagenta),
                    CellType::Metal(_) => ("M", Color::LightBlue),
                    CellType::Meat(_) => ("m", Color::Rgb(150,75,0)),
                    CellType::Base => ("#", Color::Yellow),
                }
            };
            row_spans.push(Span::styled(symbol, Style::default().fg(color)));
        }
        map_lines.push(Line::from(row_spans));
    }

    let map_paragraph = Paragraph::new(map_lines);
    f.render_widget(map_paragraph, content_area);

    let vertical_scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight);
    let mut vertical_state = ScrollbarState::new(sim.height)
        .position(scroll_y)
        .viewport_content_length(visible_height);
    f.render_stateful_widget(vertical_scrollbar, vertical_scrollbar_area, &mut vertical_state);

    let horizontal_scrollbar = Scrollbar::default().orientation(ScrollbarOrientation::HorizontalBottom);
    let mut horizontal_state = ScrollbarState::new(sim.width)
        .position(scroll_x)
        .viewport_content_length(visible_width);
    f.render_stateful_widget(horizontal_scrollbar, horizontal_scrollbar_area, &mut horizontal_state);

    let base_x = sim.width / 2;
    let base_y = sim.height / 2;
    let marker_style = Style::default().fg(Color::LightGreen);

    let v_track_height = vertical_scrollbar_area.height.saturating_sub(2); 
    if v_track_height > 0 && sim.height > 0 {
        let relative_y = (base_y * v_track_height as usize) / sim.height;
        let marker_y = vertical_scrollbar_area.y + 1 + relative_y as u16;
        let marker_x = vertical_scrollbar_area.x;

        let marker = Paragraph::new("*").style(marker_style);
        f.render_widget(marker, Rect::new(marker_x, marker_y, 1, 1));
    }

    let h_track_width = horizontal_scrollbar_area.width.saturating_sub(2);
    if h_track_width > 0 && sim.width > 0 {
        let relative_x = (base_x * h_track_width as usize) / sim.width;
        let marker_x = horizontal_scrollbar_area.x + 1 + relative_x as u16;
        let marker_y = horizontal_scrollbar_area.y;

        let marker = Paragraph::new("*").style(marker_style);
        f.render_widget(marker, Rect::new(marker_x, marker_y, 1, 1));
    }

    if sim.base_hp <= 0 {
        let area = chunks[0];
        let overlay_width = area.width.min(40);
        let overlay_height = 3u16;
        let overlay_x = area.x + (area.width.saturating_sub(overlay_width)) / 2;
        let overlay_y = area.y + (area.height.saturating_sub(overlay_height)) / 2;

        let overlay = Paragraph::new("La simulation est terminée")
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::White).bg(Color::DarkGray));
        f.render_widget(overlay, Rect::new(overlay_x, overlay_y, overlay_width, overlay_height));
    }

    let stats = format!(
        " HP: {} | Cristaux: {} | Viande: {} | Métal: {} | Flèches: déplacer | [q] Quitter | Facteur de peur: {:.2}",
        sim.base_hp, sim.collected_crystals, sim.collected_meat, sim.collected_metal, sim.fear_factor
    );
    let ui_paragraph = Paragraph::new(stats)
        .block(Block::default().borders(Borders::ALL).title("Tableau de bord"))
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(ui_paragraph, chunks[1]);
}

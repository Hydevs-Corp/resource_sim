use ratatui::style::Color;

pub fn meteorite_frame_visual(frame: u8) -> (&'static str, Color) {
    match frame {
        0 => ("☄", Color::LightYellow),
        1 => ("✺", Color::White),
        2 => ("✸", Color::LightRed),
        3 => ("▓", Color::Rgb(255, 140, 0)),
        4 => ("▒", Color::Rgb(180, 90, 30)),
        5 => ("█", Color::Rgb(200, 50, 0)),
        6 => ("▓", Color::Rgb(120, 60, 20)),
        7 => ("░", Color::DarkGray),
        _ => (" ", Color::DarkGray),
    }
}

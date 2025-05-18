use crate::api::bf1api::server::ServerDetails;
use crate::botstatus::{BotStatus, StatusTypes};
use crate::BotStats;
use crossterm::cursor::{MoveTo, MoveToNextLine, RestorePosition, SavePosition};
use crossterm::style::{Attribute, Color, ContentStyle, Print, SetAttribute, SetForegroundColor};
use crossterm::terminal::{Clear, ClearType, ScrollDown, ScrollUp};
use crossterm::{execute, terminal, ExecutableCommand};
use std::cmp::max;
use std::error::Error;
use std::io::Write;
use tokio::sync::RwLock;

pub struct Console {
    logging_y: u16,
    player_name: String,
}

pub fn log<T: Error>(err: &T) {
    let mut out = std::io::stdout();
    let (_, height) = terminal::size().unwrap();
    let _ = execute!(
        out,
        MoveTo(0, height - 1),
        SetAttribute(Attribute::Bold),
        SetForegroundColor(Color::Red),
        Print("Error: "),
        SetAttribute(Attribute::NoBold),
        Print(err.to_string()),
        SetAttribute(Attribute::Reset)
    );

    out.flush().unwrap();
}

pub fn update_status(status: StatusTypes) {
    let mut out = std::io::stdout();
    let _ = execute!(
        out,
        MoveTo(0, 0),
        SetAttribute(Attribute::Fraktur),
        Print("Status: ".to_string()),
        SetForegroundColor(Color::from(status)),
        Print(status.to_string()),
        SetAttribute(Attribute::Reset),
        Print(" ".repeat(31 - status.to_string().len()))
    );

    out.flush().unwrap();
}

pub fn update_kick_count(kick_count: i32) {
    let mut out = std::io::stdout();

    let _ = execute!(
        out,
        MoveTo(0, 2),
        Print(format!("Kicked {} players", kick_count.to_string()))
    );

    out.flush().unwrap();
}

pub fn clear() {
    let mut out = std::io::stdout();

    let _ = execute!(out, Clear(ClearType::Purge));

    out.flush().unwrap();
}

impl Console {
    pub fn new(player_name: String) -> Self {
        Console {
            logging_y: 10,
            player_name,
        }
    }

    pub async fn update_static_area(
        &self,
        server_details: &ServerDetails,
        bot_status: &BotStatus,
        bot_stats: &BotStats,
        width: u16,
        height: u16,
    ) {
        let mut out = std::io::stdout();

        let mid_x = width / 2;

        let _ = execute!(
            out,
            MoveTo(0, 0),
            Clear(ClearType::All),
            Clear(ClearType::Purge)
        );

        let title_text = String::from("=== VG KICKBOT ===");
        let credits_text = String::from("by mystzone44 and octorix");

        let title_start_x = max(mid_x - title_text.len() as u16 / 2, 0);
        let credits_start_x = max(mid_x - credits_text.len() as u16 / 2, 0);

        // Title
        let _ = execute!(
            out,
            MoveTo(title_start_x, 0),
            SetAttribute(Attribute::SlowBlink),
            SetAttribute(Attribute::Bold),
            Print("=== VG KICKBOT ===".to_string()),
            MoveTo(credits_start_x, 1),
            SetAttribute(Attribute::Dim),
            SetAttribute(Attribute::Underlined),
            Print("by mystzone44 and octorix".to_string()),
            SetAttribute(Attribute::Reset)
        );

        let status_y = 0;

        // Bot Status
        execute!(
            out,
            MoveTo(0, status_y),
            SetAttribute(Attribute::Fraktur),
            Print("Status: ".to_string()),
            SetForegroundColor(Color::from(bot_status.status)),
            Print(bot_status.status.to_string()),
            SetForegroundColor(Color::White),
            MoveTo(0, status_y + 1),
            SetAttribute(Attribute::Dim),
            Print("Logged in as: "),
            SetAttribute(Attribute::DoubleUnderlined),
            Print(&self.player_name),
            SetAttribute(Attribute::Reset),
            MoveTo(0, status_y + 2),
            Print(format!(
                "Kicked {} players",
                bot_stats.players_kicked.to_string()
            ))
        )
        .unwrap();

        let player_count_text = format!(
            "Player Count [{}/{}]",
            server_details.player_count().to_string(),
            server_details.max_player_count.to_string()
        );
        let spectator_count_text = format!(
            "Spectator Count [{}/{}]",
            server_details.spectator_count.to_string(),
            server_details.max_spectator_count.to_string()
        );

        let map_name = server_details.map.clone();

        // Game Info
        execute!(
            out,
            MoveTo(width - player_count_text.len() as u16, status_y),
            Print(player_count_text),
            MoveTo(width - spectator_count_text.len() as u16, status_y + 1),
            Print(spectator_count_text),
            MoveTo(width - server_details.name.len() as u16, status_y + 2),
            Print(&server_details.name),
            MoveTo(width - map_name.len() as u16 - 5, status_y + 3),
            Print("Map: "),
            SetAttribute(Attribute::Bold),
            Print(map_name),
            SetAttribute(Attribute::Reset)
        )
        .unwrap();

        let seperator_pos_x = mid_x - 1;

        let start_team_names = seperator_pos_x - server_details.team1_name.len() as u16;

        // Teams
        let start_y = 5;
        let team_names = format!(
            "{} | {}",
            server_details.team1_name, server_details.team2_name
        );
        execute!(
            out,
            MoveTo(start_team_names, start_y),
            SetAttribute(Attribute::Bold),
            Print(team_names.as_str()),
            MoveTo(start_team_names, start_y + 1),
            Print("-".to_string().repeat(team_names.len())),
            SetAttribute(Attribute::Reset)
        );

        let mut y = start_y + 2;

        for players in server_details.team1.iter().zip(server_details.team2.iter()) {
            let start_x = seperator_pos_x - players.0 .0.len() as u16;
            execute!(
                out,
                MoveTo(start_x, y),
                Print(format!("{}", players.0 .0)),
                SetAttribute(Attribute::Bold),
                Print(" | "),
                SetAttribute(Attribute::Reset),
                Print(format!("{}", players.1 .0))
            );
            y += 1;
        }

        out.flush().unwrap();
    }
}

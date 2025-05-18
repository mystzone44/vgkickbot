extern crate core;

mod api;
mod botstatus;
mod config;
mod console;
mod cycle;
mod discord;
mod errors;
mod recognition;

use crate::api::bf1api::server::ServerDetails;
use crate::api::bf1api::BF1Api;
use crate::botstatus::{BotStatus, StatusTypes};
use crate::config::{load_kick_history_record, save_kick_record, Config, PlayerKickHistoryRecord};
use crate::console::{clear, log, update_status};
use crate::cycle::{execute, Executors, GameState, SpecCycle};
use crate::discord::{announce_bot_crashed, announce_monitoring, announce_shutdown};
use crate::errors::KickbotError;
use crate::errors::KickbotError::ScreenshotError;
use crate::recognition::kick_player::kick_player;
use crate::recognition::model::Classifier;
use chrono::{DateTime, Utc};
use crossterm::event::{poll, read, Event};
use enigo::Direction::{Press, Release};
use enigo::{Button, Enigo, Keyboard, Mouse, Settings};
use serenity::all::MemberAction::Kick;
use std::collections::HashSet;
use std::ffi::CString;
use std::io::ErrorKind;
use std::ops::Deref;
use std::path::Path;
use std::process::{exit, Command};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use std::{env, io, thread};
use sysinfo::System;
use tokio::sync::{Mutex, OnceCell, RwLock};
use tokio::time::sleep;
use win_screenshot::prelude::find_window;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Console::SetConsoleCtrlHandler;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowA, SetForegroundWindow};
use windows_core::PCSTR;
/*
Don't try refactor this piece of shit, it works on hopes, dreams and an incredibly poorly written web of functions
 */

#[derive(Debug)]
struct BotStats {
    start_time: DateTime<Utc>,
    players_kicked: i32,
}

static BOT_STATS: OnceLock<Arc<RwLock<BotStats>>> = OnceLock::new();

static CONFIG: OnceCell<Config> = OnceCell::const_new();

static KICK_RECORD: OnceLock<Arc<Mutex<PlayerKickHistoryRecord>>> = OnceLock::new();

static mut DO_EXIT_ANNOUNCEMENT: bool = true;

unsafe fn restart_bot() -> io::Result<()> {
    let current_exe = env::current_exe()?;
    Command::new(current_exe).args(&["0"]).status()?;
    DO_EXIT_ANNOUNCEMENT = false;

    clear();

    exit(0);
}

fn bf1_running() -> bool {
    find_window("Battlefield™ 1").is_ok()
}

fn kill_bf1() {
    let s = System::new_all();
    if let Some(process) = s.processes_by_name("bf1".as_ref()).next() {
        process.kill();
    };
}

fn launch_bf1_join_server(path_str: String, game_id: String) {
    //"C:\Program Files\EA Games\Battlefield 1\bf1.exe" -gameMode MP -role solder -asSpectator true -gameId ...
    let path = Path::new(path_str.as_str());
    if let Err(err) = Command::new(path)
        .args(&[
            "-gameMode",
            "MP",
            "-role",
            "soldier",
            "-asSpectator",
            "true",
            "-gameId",
            game_id.as_str(),
        ])
        .status()
    {
        log(&KickbotError::IOError(format!(
            "Failed to launch BF1 at path {}",
            path.as_os_str().to_str().unwrap()
        )));
    }
}

async fn try_focus_bf1() {
    let window_title = String::from("Battlefield™ 1");
    let mut hwnd = unsafe { FindWindowA(None, PCSTR::from_raw(window_title.as_ptr())).ok() };

    if let Some(hwnd) = hwnd {
        if hwnd.0 != std::ptr::null_mut() {
            unsafe {
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }

    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        let _ = enigo.button(Button::Left, Press);
        sleep(Duration::from_secs(1)).await;
        let _ = enigo.button(Button::Left, Release);
    }
}

async fn focus_bf1_once_running() {
    while !bf1_running() {
        sleep(Duration::from_secs(1)).await;
    }
    sleep(Duration::from_secs(10)).await;
    unsafe {
        if let Err(err) = restart_bot() {
            log(&KickbotError::IOError(format!(
                "Failed to restart bot, please do it manually, {err}"
            )));
        }
    }

    let window_title = String::from("Battlefield™ 1");
    let mut hwnd: Option<HWND> = None;
    while hwnd.is_none() {
        hwnd = unsafe { FindWindowA(None, PCSTR::from_raw(window_title.as_ptr())).ok() };
        sleep(Duration::from_secs(1)).await;
    }

    if let Some(hwnd) = hwnd {
        if hwnd.0 != std::ptr::null_mut() {
            unsafe {
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }

    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        let _ = enigo.button(Button::Left, Press);
        sleep(Duration::from_secs(1)).await;
        let _ = enigo.button(Button::Left, Release);
    }
}

unsafe extern "system" fn close_handler(_: u32) -> windows_core::BOOL {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            if (DO_EXIT_ANNOUNCEMENT) {
                announce_shutdown(
                    &CONFIG.get().unwrap().monitoring_webhook,
                    BOT_STATS.get().unwrap().read().await.deref(),
                )
                .await
                .expect("Something went wrong announcing shutdown");
            }

            let _ = save_kick_record(KICK_RECORD.get().unwrap().lock().await.deref()).inspect_err(
                |err| {
                    panic!(
                        "Something went wrong saving kick record, {}",
                        err.to_string()
                    )
                },
            );
        });

    windows_core::BOOL(1)
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    // Arg 1: Announce monitoring yes/no
    let mut should_announce_monitor = true;
    if args.len() >= 2 {
        should_announce_monitor = args[1].parse::<u8>().unwrap() == 1;
    }

    unsafe {
        SetConsoleCtrlHandler(Some(close_handler), true)?;
    }

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(main_thread(should_announce_monitor))
}

async fn main_thread(should_announce_monitor: bool) -> io::Result<()> {
    let config = match Config::read_config("config.json").await {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Error reading config, {err}");
            return Err(io::Error::new(ErrorKind::InvalidData, err));
        }
    };

    BOT_STATS
        .set(Arc::new(RwLock::new(BotStats {
            start_time: Utc::now(),
            players_kicked: 0,
        })))
        .unwrap();

    let bot_status = Arc::new(RwLock::new(BotStatus {
        status: StatusTypes::WaitingForBF1,
        timer_start: Instant::now(),
        map_start: String::new(),
        last_valid_name: None,
    }));

    if should_announce_monitor {
        announce_monitoring(
            &config.monitoring_webhook,
            BOT_STATS.get().unwrap().read().await.start_time,
        )
        .await
        .inspect_err(log)?;
    }

    let bf1_api = BF1Api::new().await?;

    static BF1_API: OnceCell<BF1Api> = OnceCell::const_new();

    let display_names = bf1_api
        .get_display_names_by_persona_ids(vec![bf1_api.persona_id().as_str()])
        .await?;

    let user_name = display_names[0].clone();
    let server = Arc::new(Mutex::new(bf1_api.get_server_by_name("![VG]").await?));

    let console = Arc::new(Mutex::new(console::Console::new(user_name)));

    let (width, height) = crossterm::terminal::size()?;
    console
        .lock()
        .await
        .update_static_area(
            server.lock().await.deref(),
            bot_status.read().await.deref(),
            BOT_STATS.get().unwrap().read().await.deref(),
            width,
            height,
        )
        .await;

    CONFIG.set(config).unwrap();
    BF1_API.set(bf1_api).unwrap();
    KICK_RECORD
        .set(Arc::new(Mutex::new(load_kick_history_record()?)))
        .unwrap();

    let spec_cycle = Arc::new(Mutex::new(SpecCycle::new()));
    let game_state = Arc::new(RwLock::new(GameState::default()));
    let executors = Arc::new(Mutex::new(Executors::new(10)));

    let server_updated: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

    let server_clone = server.clone();
    let bot_status_clone = bot_status.clone();
    let server_updated_clone = server_updated.clone();

    let server_clone_2 = server.clone();
    let bot_status_clone_2 = bot_status.clone();
    let console_clone = console.clone();

    tokio::spawn(async move {
        loop {
            if let Ok(Event::Resize(width, height)) = read() {
                console_clone
                    .lock()
                    .await
                    .update_static_area(
                        server_clone_2.lock().await.deref(),
                        bot_status_clone_2.read().await.deref(),
                        BOT_STATS.get().unwrap().read().await.deref(),
                        width,
                        height,
                    )
                    .await;
            }
        }
    });

    let game_state_clone = game_state.clone();

    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(10)).await;

            let mut server_details = server_clone.lock().await;
            if let Err(err) = server_details.update_players(BF1_API.get().unwrap()).await {
                log(&err);
            }
            let gameid = Some(server_details.game_id.clone());
            if let Err(err) = server_details
                .update_server_details(BF1_API.get().unwrap(), gameid)
                .await
            {
                log(&err);
            }

            if server_details.player_count() < CONFIG.get().unwrap().min_players_for_kick as usize {
                bot_status_clone.write().await.status = StatusTypes::Disabled;
            } else {
                if bot_status_clone.read().await.status == StatusTypes::Disabled {
                    bot_status_clone.write().await.status = StatusTypes::WaitingForBF1;
                }
            }

            let (width, height) = crossterm::terminal::size().unwrap();
            console
                .lock()
                .await
                .update_static_area(
                    server_details.deref(),
                    bot_status_clone.read().await.deref(),
                    BOT_STATS.get().unwrap().read().await.deref(),
                    width,
                    height,
                )
                .await;

            let mut server_updated_writer = server_updated_clone.lock().await;
            *server_updated_writer = true;

            let game_state_read = game_state_clone.read().await;
            for (player, weapon) in game_state_read.pending_kick_players.iter() {
                kick_player(
                    BF1_API.get().unwrap(),
                    CONFIG.get().unwrap(),
                    KICK_RECORD.get().unwrap().clone(),
                    player,
                    weapon.name.clone(),
                    weapon.category.clone(),
                    game_state_clone.clone(),
                    server_details.deref(),
                    BOT_STATS.get().unwrap().clone(),
                    true,
                )
                .await
            }
        }
    });

    let server_cached: ServerDetails = server.clone().lock().await.clone();

    let classifier = Arc::new(Classifier::new());

    if !bf1_running() {
        launch_bf1_join_server(
            CONFIG.get().unwrap().bf1_path.clone(),
            server_cached.game_id,
        );

        focus_bf1_once_running().await;
    } else {
        try_focus_bf1().await;
    }

    // If we crash/don't have BF1, invalidate the last player name
    // So if we don't have a valid last player name then we know not to send a crash message if we don't read one
    loop {
        let do_cycle = async || {
            if let Ok(window) = active_win_pos_rs::get_active_window() {
                if window.title != "Battlefield™ 1" {
                    return false;
                }

                if matches!(
                    bot_status.read().await.status,
                    StatusTypes::Online
                        | StatusTypes::WaitingForNewMap
                        | StatusTypes::WaitingForBF1
                ) {
                    let mut is_updated = server_updated.lock().await;
                    if *is_updated {
                        let server_cached = server.lock().await.clone();
                        *is_updated = false;

                        let mut game_state = game_state.write().await;
                        let names: HashSet<String> = server_cached
                            .team1
                            .keys()
                            .chain(server_cached.team2.keys())
                            .cloned()
                            .collect();

                        game_state
                            .already_kicked_list_players
                            .retain(|name| names.contains(name))
                    }

                    if let Err(err) = execute(
                        BF1_API.get().unwrap(),
                        CONFIG.get().unwrap(),
                        KICK_RECORD.get().unwrap().clone(),
                        game_state.clone(),
                        executors.clone(),
                        spec_cycle.clone(),
                        server.clone(),
                        bot_status.clone(),
                        BOT_STATS.get().unwrap().clone(),
                        classifier.clone(),
                    )
                    .await
                    {
                        log(&err);
                    }
                }
                return true;
            }
            false
        };

        if !do_cycle().await {
            let bot_status_read = bot_status.read().await;

            if bot_status_read.status == StatusTypes::Crashed {
                if let Err(err) =
                    announce_bot_crashed(&CONFIG.get().unwrap().monitoring_webhook).await
                {
                    log(&err);
                }
                kill_bf1();
                while bf1_running() {
                    sleep(Duration::from_secs(1)).await;
                }
                launch_bf1_join_server(
                    CONFIG.get().unwrap().bf1_path.clone(),
                    server.lock().await.clone().game_id,
                );
                focus_bf1_once_running().await;

                drop(bot_status_read);
                bot_status.write().await.status = StatusTypes::WaitingForBF1;
                update_status(StatusTypes::WaitingForBF1);
            } else if bot_status_read.status != StatusTypes::WaitingForBF1
                && bot_status_read.status != StatusTypes::Disabled
            {
                drop(bot_status_read);
                bot_status.write().await.status = StatusTypes::WaitingForBF1;
                update_status(StatusTypes::WaitingForBF1);
            }
        }
    }
}

use crate::api::bf1api::server::ServerDetails;
use crate::api::bf1api::BF1Api;
use crate::botstatus::{BotStatus, StatusTypes};
use crate::config::{Config, PlayerKickHistoryRecord};
use crate::console::{log, update_status};
use crate::errors::KickbotError;
use crate::recognition::detection::{detect, detect_player_name};
use crate::recognition::kick_player::kick_player;
use crate::recognition::model::{Classifier, WeaponClasses};
use crate::recognition::ocr::OCR;
use crate::recognition::screenshot::Screenshot;
use crate::BotStats;
use chrono::Local;
use enigo::Direction::{Press, Release};
use enigo::{Enigo, Key, Keyboard, Settings};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::ops::Deref;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;

#[derive(Clone)]
pub struct RecordWeapon {
    pub(crate) name: String,
    pub(crate) category: WeaponClasses,
}

#[derive(Clone)]
pub struct GameState {
    pub no_player_count: u8,
    pub last_player: String,
    pub same_player_count: u8,
    pub rotate_key: char,
    pub already_kicked_list_players: HashSet<String>,
    pub pending_kick_players: HashMap<String, RecordWeapon>,
}

impl GameState {
    pub fn default() -> Self {
        GameState {
            no_player_count: 0,
            last_player: String::new(),
            same_player_count: 0,
            rotate_key: 'e',
            already_kicked_list_players: Default::default(),
            pending_kick_players: Default::default(),
        }
    }
}

pub struct Executors {
    pub ocr: Vec<Option<OCR>>,
    pub idx: usize,
}

impl Executors {
    pub fn new(size: usize) -> Self {
        let mut ocr = vec![];
        for _ in 0..size {
            ocr.push(Some(OCR::new()));
        }
        Executors { ocr, idx: 0 }
    }
}

pub struct SpecCycle {
    enigo: Enigo,
}
impl SpecCycle {
    pub fn new() -> SpecCycle {
        SpecCycle {
            enigo: Enigo::new(&Settings::default()).unwrap(),
        }
    }
}

pub async fn do_detection(
    api: &'static BF1Api,
    config: &'static Config,
    ocr: OCR,
    bot_status: &RwLock<BotStatus>,
    game_state: &RwLock<GameState>,
    server: Arc<Mutex<ServerDetails>>,
    classifier: Arc<Classifier>,
    bot_stats: Arc<RwLock<BotStats>>,
) -> Result<
    (
        OCR,
        Option<String>,
        Option<String>,
        Option<WeaponClasses>,
        Option<Screenshot>,
    ),
    KickbotError,
> {
    let screenshot = Screenshot::take_screenshot()?;
    let (ocr, player_name) = detect_player_name(&screenshot, config, ocr)?;
    let Some(player_name) = player_name else {
        return Ok((ocr, None, None, None, None));
    };

    // Max player name is 3 so probably didn't read anything
    if player_name.len() < 3 {
        let mut game_state_write = game_state.write().await;
        game_state_write.no_player_count += 1;
        if game_state_write.no_player_count == 2 {
            let mut bot_status = bot_status.write().await;
            bot_status.status = StatusTypes::WaitingForNewMap;
            update_status(bot_status.status);
            game_state_write.no_player_count = 0;
        }
        return Ok((ocr, None, None, None, None));
        // No need to continue
    } else {
        let bot_status_read = bot_status.read().await;
        if bot_status_read.status != StatusTypes::Online {
            drop(bot_status_read);
            let mut bot_status = bot_status.write().await;
            bot_status.status = StatusTypes::Online;
            update_status(bot_status.status);
            game_state.write().await.no_player_count = 0;
        }
    }

    let mut game_state_write = game_state.write().await;
    if config.are_similar(
        player_name.as_str(),
        game_state_write.last_player.as_str(),
        config.player_similar_name_probability,
    ) {
        game_state_write.same_player_count += 1;
        if game_state_write.same_player_count == 2 {
            // Go other way
            if game_state_write.rotate_key == 'e' {
                game_state_write.rotate_key = 'q';
            } else {
                game_state_write.rotate_key = 'e';
            }
        } else if game_state_write.same_player_count == 10 {
            bot_status.write().await.status = StatusTypes::Crashed;
            update_status(StatusTypes::Crashed);
        }
    } else {
        if game_state_write.same_player_count > 0 {
            game_state_write.same_player_count = 0;
        }
    }

    game_state_write.last_player = player_name.clone();

    let (ocr, banned_weapon_name, category) = detect(&screenshot, config, ocr, classifier.deref())?;

    if config.save_screenshots {
        Ok((
            ocr,
            Some(player_name),
            banned_weapon_name,
            category,
            Some(screenshot),
        ))
    } else {
        Ok((ocr, Some(player_name), banned_weapon_name, category, None))
    }
}

pub async fn execute(
    api: &'static BF1Api,
    config: &'static Config,
    kick_record: Arc<Mutex<PlayerKickHistoryRecord>>,
    game_state: Arc<RwLock<GameState>>,
    executors: Arc<Mutex<Executors>>,
    spec_cycle: Arc<Mutex<SpecCycle>>,
    server: Arc<Mutex<ServerDetails>>,
    bot_status: Arc<RwLock<BotStatus>>,
    bot_stats: Arc<RwLock<BotStats>>,
    classifier: Arc<Classifier>,
) -> Result<(), KickbotError> {
    sleep(config.rotate_delay).await;

    let bot_status_clone = bot_status.clone();
    let server_clone = server.clone();
    let bot_stats_clone = bot_stats.clone();

    tokio::spawn(async move {
        let (current_idx, maybe_ocr) = {
            let mut executors = executors.lock().await;
            let current_idx = executors.idx;
            let maybe_ocr = executors.ocr[current_idx].take();
            executors.idx = (executors.idx + 1) % executors.ocr.len();

            (current_idx, maybe_ocr)
        };

        if let Some(ocr) = maybe_ocr {
            if let Ok((
                new_ocr,
                maybe_player_name,
                maybe_banned_weapon,
                maybe_category,
                maybe_screenshot,
            )) = do_detection(
                api,
                config,
                ocr,
                bot_status_clone.deref(),
                &game_state,
                server_clone,
                classifier.clone(),
                bot_stats_clone,
            )
            .await
            {
                if let Some(banned_weapon) = maybe_banned_weapon {
                    if let Some(player_name) = maybe_player_name {
                        if let Some(category) = maybe_category {
                            kick_player(
                                api,
                                config,
                                kick_record,
                                &player_name,
                                banned_weapon.clone(),
                                category,
                                game_state,
                                server.lock().await.deref(),
                                bot_stats,
                                false,
                            )
                            .await;
                        }

                        if let Some(screenshot) = maybe_screenshot {
                            let _ = screenshot
                                .save(
                                    format!(
                                        "{}-{}-{}",
                                        player_name,
                                        banned_weapon,
                                        Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
                                    )
                                    .as_str(),
                                )
                                .map_err(|err| log(&err));
                        }
                    }
                }

                let mut executors = executors.lock().await;
                executors.ocr[current_idx] = Some(new_ocr);
            }
        } else {
            let mut executors = executors.lock().await;
            executors.ocr[current_idx] = Some(OCR::new());
        }
    });

    let mut key = Key::E;
    if bot_status.read().await.status == StatusTypes::WaitingForNewMap {
        key = Key::F5;
    }

    let mut spec_cycle = spec_cycle.lock().await;

    spec_cycle
        .enigo
        .key(key, Press)
        .map_err(|err| KickbotError::IOError(err.to_string()))?;
    tokio::time::sleep(Duration::from_millis(50)).await;
    spec_cycle
        .enigo
        .key(key, Release)
        .map_err(|err| KickbotError::IOError(err.to_string()))?;

    Ok(())
}

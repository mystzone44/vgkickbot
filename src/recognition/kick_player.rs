use crate::api::bf1api::server::ServerDetails;
use crate::api::bf1api::BF1Api;
use crate::config::{Config, PlayerKickHistoryRecord};
use crate::cycle::{GameState, RecordWeapon};
use crate::discord::DiscordWebhook;
use crate::recognition::model::WeaponClasses;
use crate::BotStats;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

pub async fn kick_player(
    api: &'static BF1Api,
    config: &'static Config,
    kick_record: Arc<Mutex<PlayerKickHistoryRecord>>,
    player_name: &String,
    banned_weapon: String,
    category: WeaponClasses,
    game_state: Arc<RwLock<GameState>>,
    server: &ServerDetails,
    bot_stats: Arc<RwLock<BotStats>>,
    is_pending: bool,
) {
    if game_state
        .read()
        .await
        .already_kicked_list_players
        .contains(player_name)
    {
        return;
    }

    let search_team = |team: &HashMap<String, String>| {
        if let Some(id) = team.get(player_name) {
            Some((player_name.clone(), id.clone()))
        } else {
            team.iter().find_map(|(name, id)| {
                if config.are_similar(name, player_name, config.player_similar_name_probability) {
                    Some((name.clone(), id.clone()))
                } else {
                    None
                }
            })
        }
    };

    if let Some((player_actual_name, id)) =
        search_team(&server.team1).or_else(|| search_team(&server.team2))
    {
        let game_id = server.game_id.clone();
        let id_string = String::from(id);
        let reason = format!("No {banned_weapon}, Read Rules");

        game_state
            .write()
            .await
            .already_kicked_list_players
            .insert(player_actual_name.clone());

        tokio::task::spawn(async {
            api.kick_player(
                game_id,
                id_string,
                player_actual_name,
                reason,
                category,
                kick_record,
                bot_stats,
                config,
            )
            .await
        });

        if is_pending {
            game_state
                .write()
                .await
                .pending_kick_players
                .remove(&player_name.clone());
        }
    } else {
        let mut game_state = game_state.write().await;
        if is_pending {
            game_state.pending_kick_players.insert(
                player_name.clone(),
                RecordWeapon {
                    name: banned_weapon,
                    category,
                },
            );
        } else {
            game_state.pending_kick_players.remove(&player_name.clone());
        }
    }
}

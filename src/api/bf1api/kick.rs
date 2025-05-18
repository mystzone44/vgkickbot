use crate::api::bf1api::{rpc_request, BF1Api};
use crate::api::endpoints;
use crate::api::errors::{BF1ApiError, BF1ApiSubError};
use crate::config::{add_to_player_kick_record, Config, PlayerKickHistoryRecord};
use crate::console::update_kick_count;
use crate::discord::{announce_kick_fail, announce_kick_success, DiscordWebhook};
use crate::errors::KickbotError;
use crate::recognition::model::WeaponClasses;
use crate::BotStats;
use serde_json::Value;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::{Arc, OnceLock};
use tokio::sync::{Mutex, RwLock};

impl BF1Api {
    pub async fn kick_player(
        &self,
        game_id: String,
        persona_id: String,
        player_name: String,
        reason: String,
        weapon_class: WeaponClasses,
        kick_record: Arc<Mutex<PlayerKickHistoryRecord>>,
        bot_stats: Arc<RwLock<BotStats>>,
        config: &Config,
    ) -> Result<(), KickbotError> {
        let params: HashMap<&str, Value> = HashMap::from([
            ("game", Value::String("tunguska".to_string())),
            ("gameId", Value::String(game_id)),
            ("personaId", Value::String(persona_id.clone())),
            ("reason", Value::String(reason.to_string())),
        ]);
        let body = rpc_request("RSP.kickPlayer".to_string(), params);
        match self
            .client
            .post(endpoints::RPC_HOST)
            .headers(self.rpc_header.clone())
            .json(&body)
            .send()
            .await
            .map_err(|err| BF1ApiError::from(err))?
            .error_for_status()
        {
            Ok(_) => {
                let mut bot_stats_writer = bot_stats.write().await;
                bot_stats_writer.players_kicked += 1;
                update_kick_count(bot_stats_writer.players_kicked);
                add_to_player_kick_record(
                    kick_record.lock().await.deref_mut(),
                    config,
                    player_name.clone(),
                    weapon_class,
                    &config.kick_webhook,
                    persona_id.as_str(),
                )
                .await;
                announce_kick_success(
                    &config.kick_webhook,
                    player_name.as_str(),
                    persona_id.as_str(),
                    reason.as_str(),
                )
                .await
            }
            Err(err) => {
                announce_kick_fail(
                    &config.kick_webhook,
                    player_name.as_str(),
                    persona_id.as_str(),
                    reason.as_str(),
                    err.to_string().as_str(),
                )
                .await
            }
        }
    }
}

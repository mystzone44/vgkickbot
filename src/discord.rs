use crate::api::errors::BF1ApiError;
use crate::config::{dates_to_csv_string, weapon_kick_records_to_csv_strings};
use crate::errors::KickbotError;
use crate::errors::KickbotError::DiscordError;
use crate::BotStats;
use chrono::{DateTime, TimeDelta, Utc};
use serenity::all::{Color, CreateEmbed, CreateEmbedAuthor, ExecuteWebhook, Http, Webhook};
use std::collections::HashMap;

#[derive(Debug)]
pub struct DiscordWebhook {
    pub http: Http,
    webhook: Webhook,
    author: CreateEmbedAuthor,
}

impl DiscordWebhook {
    pub async fn new(url: &str, username: &str) -> Result<DiscordWebhook, KickbotError> {
        let http = Http::new("token");

        let webhook = Webhook::from_url(&http, url).await.map_err(|err| {
            DiscordError(format!(
                "Error connecting to monitoring webhook: {}",
                err.to_string()
            ))
        })?;

        let author = CreateEmbedAuthor::new(username);

        Ok(DiscordWebhook {
            http,
            webhook,
            author,
        })
    }
}

pub async fn announce_monitoring(
    monitoring_webhook: &DiscordWebhook,
    start_time: DateTime<Utc>,
) -> Result<(), KickbotError> {
    let start_time_string = start_time.format("%H:%M:%S").to_string();
    let embed = CreateEmbed::new()
        .title("Now Monitoring")
        .description(format!("Began monitoring at {}", start_time_string))
        .color(Color::DARK_GREEN);

    let builder = ExecuteWebhook::new().embed(embed).username("Spec Bot");
    monitoring_webhook
        .webhook
        .execute(&monitoring_webhook.http, false, builder)
        .await
        .map_err(|err| {
            DiscordError(format!(
                "Error sending begin monitoring message: {}",
                err.to_string()
            ))
        })?;
    Ok(())
}

fn get_time_difference_string(elapsed_time: TimeDelta) -> String {
    let mut elapsed_time_str;
    if elapsed_time.num_hours() > 0 {
        elapsed_time_str = format!("{} hour", elapsed_time.num_hours().to_string());
        if elapsed_time.num_hours() > 1 {
            elapsed_time_str.push('s');
        }

        if elapsed_time.num_minutes() > 0 {
            elapsed_time_str = format!(
                "{} and {} minutes",
                elapsed_time_str,
                elapsed_time.num_minutes() % 60
            );
        }
        if elapsed_time.num_minutes() == 1 {
            elapsed_time_str.pop().unwrap();
        }
    } else {
        elapsed_time_str = format!("{} minutes", elapsed_time.num_minutes());
    }

    if elapsed_time.num_minutes() == 1 {
        elapsed_time_str.pop().unwrap();
    }

    elapsed_time_str
}

pub async fn announce_shutdown(
    monitoring_webhook: &DiscordWebhook,
    bot_stats: &BotStats,
) -> Result<(), KickbotError> {
    let elapsed_time = Utc::now() - bot_stats.start_time;

    let mut players_kicked_string = format!("{} players", bot_stats.players_kicked.to_string());
    if bot_stats.players_kicked == 1 {
        players_kicked_string.pop().unwrap();
    }

    let embed = CreateEmbed::new()
        .title("Stopped Monitoring")
        .description(format!(
            "Uptime: {}\n\n Kicked {}",
            get_time_difference_string(elapsed_time),
            players_kicked_string
        ))
        .color(Color::DARK_RED);

    let builder = ExecuteWebhook::new().embed(embed).username("Spec Bot");
    monitoring_webhook
        .webhook
        .execute(&monitoring_webhook.http, false, builder)
        .await
        .map_err(|err| {
            DiscordError(format!(
                "Error sending begin monitoring message: {}",
                err.to_string()
            ))
        })?;

    Ok(())
}

async fn announce_kick(
    kick_webhook: &DiscordWebhook,
    embed: CreateEmbed,
) -> Result<(), KickbotError> {
    let builder = ExecuteWebhook::new().embed(embed).username("Spec Bot");
    kick_webhook
        .webhook
        .execute(&kick_webhook.http, false, builder)
        .await
        .map_err(|err| DiscordError(format!("Error sending kick message: {}", err.to_string())))?;

    Ok(())
}

pub async fn announce_kick_success(
    kick_webhook: &DiscordWebhook,
    player_name: &str,
    player_pid: &str,
    reason: &str,
) -> Result<(), KickbotError> {
    let embed = CreateEmbed::new()
        .title("Kick Success")
        .description(format!(
            "Name: {}\nReason: {}\n PID: {}",
            player_name, reason, player_pid
        ))
        .color(Color::DARK_GREEN);
    announce_kick(kick_webhook, embed).await
}

pub async fn announce_kick_fail(
    kick_webhook: &DiscordWebhook,
    player_name: &str,
    player_pid: &str,
    reason: &str,
    error: &str,
) -> Result<(), KickbotError> {
    let embed = CreateEmbed::new()
        .title("Kick Failed")
        .description(format!(
            "Name: {}\nReason: {}\n PID: {}\n Error: {}",
            player_name, reason, player_pid, error
        ))
        .color(Color::DARK_RED);
    announce_kick(kick_webhook, embed).await
}

pub async fn announce_player_multiple_kicks(
    kick_webhook: &DiscordWebhook,
    player_name: &str,
    player_pid: &str,
    number_of_kicks: u64,
    record: &HashMap<String, Vec<DateTime<Utc>>>,
) -> Result<(), KickbotError> {
    let id = "<admin id>";
    let embed_msg_content = format!("<@&{}>\n", id);

    let embed = CreateEmbed::new()
        .color(Color::DARK_RED)
        .title("Multiple Kicks")
        .description(format!(
            "Player`{}`\nPID:`{}`\n has {} lifetime kicks",
            player_name, player_pid, number_of_kicks
        ));

    let builder = ExecuteWebhook::new()
        .embed(embed)
        .content(embed_msg_content)
        .username("Spec Bot");
    kick_webhook
        .webhook
        .execute(&kick_webhook.http, false, builder)
        .await
        .map_err(|err| {
            DiscordError(format!(
                "Error sending multiple kicks embed message: {}",
                err.to_string()
            ))
        })?;

    let mut weapon_lines = String::new();

    for (name, dates) in record {
        weapon_lines = format!(
            "{weapon_lines}{name}: {}\n",
            dates_to_csv_string(dates).join(", ")
        );
    }

    let content = format!("```\n{weapon_lines}```");

    let builder = ExecuteWebhook::new().content(content).username("Spec Bot");
    kick_webhook
        .webhook
        .execute(&kick_webhook.http, false, builder)
        .await
        .map_err(|err| {
            DiscordError(format!(
                "Error sending multiple kicks content message: {}",
                err.to_string()
            ))
        })?;

    Ok(())
}

pub async fn announce_bot_crashed(monitoring_webhook: &DiscordWebhook) -> Result<(), KickbotError> {
    let embed = CreateEmbed::new()
        .color(Color::DARK_RED)
        .description("BF1 Crashed, restarting...");
    let builder = ExecuteWebhook::new().username("Spec Bot").embed(embed);
    monitoring_webhook
        .webhook
        .execute(&monitoring_webhook.http, false, builder)
        .await
        .map_err(|err| {
            DiscordError(format!(
                "Error sending multiple kicks content message: {}",
                err.to_string()
            ))
        })?;

    Ok(())
}

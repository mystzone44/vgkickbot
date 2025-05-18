use crate::console::log;
use crate::discord::{announce_player_multiple_kicks, DiscordWebhook};
use crate::errors::KickbotError;
use crate::errors::KickbotError::{IOError, JsonError};
use crate::recognition::enhance::RGB;
use crate::recognition::model::WeaponClasses;
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use gestalt_ratio::gestalt_ratio;
use opencv::core::Rect;
use serde_json::Value;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

pub fn dates_to_csv_string(dates: &Vec<DateTime<Utc>>) -> Vec<String> {
    dates
        .iter()
        .map(|date| date.format("%Y-%m-%d %H:%M").to_string())
        .collect()
}

fn get_total_kicks(kick_record: &HashMap<String, Vec<DateTime<Utc>>>) -> u64 {
    kick_record
        .iter()
        .fold(0, |acc, (_, dates)| acc + dates.len() as u64)
}

pub fn weapon_kick_records_to_csv_strings(
    kick_records: &HashMap<String, Vec<DateTime<Utc>>>,
) -> Vec<String> {
    kick_records
        .iter()
        .flat_map(|(name, dates)| {
            let mut entries = vec![name.clone()];
            entries.extend(dates_to_csv_string(dates));
            entries
        })
        .collect()
}

#[derive(Debug)]
pub struct Vehicle {
    pub pretty_name: String,
    pub primary_names: Vec<String>,
    pub secondary_names: Vec<String>,
}

#[derive(Debug)]
pub struct Gadget {
    pretty_name: String,
    names: Vec<String>,
}

#[derive(Debug)]
pub struct Weapon {
    pub pretty_name: String,
    pub names: Vec<String>,
}

pub type PlayerKickHistoryRecord = HashMap<String, HashMap<String, Vec<DateTime<Utc>>>>;

static CSV_FILE_NAME: &str = "kick_history.csv";

fn get_csv_path() -> Result<&'static str, KickbotError> {
    if let Ok(false) = std::fs::exists(CSV_FILE_NAME) {
        log(&IOError(format!(
            "File {CSV_FILE_NAME} does not exist, creating"
        )));
        File::create_new(CSV_FILE_NAME)?;
    }
    Ok(CSV_FILE_NAME)
}

pub fn load_kick_history_record() -> Result<PlayerKickHistoryRecord, KickbotError> {
    let mut csv_reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_path(get_csv_path()?)?;

    let mut player_kick_history_records = PlayerKickHistoryRecord::new();

    for result in csv_reader.records() {
        if let Ok(record) = result {
            let mut iter = record.iter();
            let player_name = match iter.next() {
                None => {
                    continue;
                }
                Some(player_name) => player_name,
            }
            .to_string();

            let mut weapon_records: HashMap<String, Vec<DateTime<Utc>>> = HashMap::new();

            let mut current_weapon = String::new();
            for entry in iter {
                if let Ok(date) = NaiveDateTime::parse_from_str(entry, "%Y-%m-%d %H:%M") {
                    if let Some(dates) = weapon_records.get_mut(&current_weapon) {
                        dates.push(DateTime::<Utc>::from_naive_utc_and_offset(date, Utc));
                    } else {
                        log(&KickbotError::IOError(
                            "Expected weapon name not date".to_string(),
                        ));
                        continue;
                    }
                } else {
                    current_weapon = entry.to_string();
                    weapon_records.insert(current_weapon.clone(), vec![]);
                }
            }
            player_kick_history_records.insert(player_name, weapon_records);
        }
    }

    Ok(player_kick_history_records)
}

pub async fn add_to_player_kick_record(
    kick_record: &mut PlayerKickHistoryRecord,
    config: &Config,
    player_name: String,
    weapon_type: WeaponClasses,
    kick_webhook: &DiscordWebhook,
    player_pid: &str,
) {
    let weapon_string = match weapon_type {
        WeaponClasses::AllowedPrimaryGuns => {
            return;
        }
        WeaponClasses::HeavyBomber | WeaponClasses::HMG | WeaponClasses::LMG => {
            match config.banned_vehicles.get(&weapon_type) {
                None => {
                    return;
                }
                Some(vehicle) => vehicle.pretty_name.clone(),
            }
        }
        WeaponClasses::SMG08 => "smg08".to_string(),
    };

    let date = Utc::now();

    match kick_record.entry(player_name.clone()) {
        Entry::Occupied(mut value) => {
            let mut records = value.get_mut();
            match records.entry(weapon_string.clone()) {
                Entry::Occupied(mut dates) => dates.get_mut().push(date),
                Entry::Vacant(_) => {
                    records.insert(weapon_string, vec![date]);
                }
            }
            let total_offences = get_total_kicks(value.get());
            if total_offences % config.kicks_to_ping == 0 {
                if let Err(err) = announce_player_multiple_kicks(
                    kick_webhook,
                    player_name.as_str(),
                    player_pid,
                    total_offences,
                    &value.get(),
                )
                .await
                {
                    log(&err);
                }
            };
        }
        Entry::Vacant(_) => {
            kick_record.insert(player_name, HashMap::from([(weapon_string, vec![date])]));
        }
    };
}

pub fn save_kick_record(kick_record: &PlayerKickHistoryRecord) -> Result<(), KickbotError> {
    let mut csv_writer = csv::Writer::from_path(get_csv_path()?)?;

    for (player_name, kick_record) in kick_record {
        let mut record = vec![player_name.clone()];
        let mut record_strings = weapon_kick_records_to_csv_strings(kick_record);

        record.append(&mut record_strings);

        csv_writer.write_record(&record)?;
    }
    Ok(())
}

#[derive(Debug)]
pub struct Config {
    pub bf1_path: String,
    pub kicks_to_ping: u64,
    pub min_players_for_kick: u64,
    pub kick_webhook: DiscordWebhook,
    pub monitoring_webhook: DiscordWebhook,
    pub player_similar_name_probability: f64,
    pub weapon_similar_name_probability: f64,
    pub save_screenshots: bool,
    pub rotate_delay: Duration,
    pub player_name_box: Rect,
    pub weapon_icon_probability: f32,
    pub weapon_icon_box: Rect,
    pub weapon_name_slot1_box: Rect,
    pub weapon_name_slot2_box: Rect,
    pub gadget_slot1_box: Rect,
    pub gadget_slot2_box: Rect,
    pub ally_colour: RGB,
    pub enemy_colour: RGB,
    pub banned_vehicles: HashMap<WeaponClasses, Vehicle>,
    pub banned_gadgets: Vec<Gadget>,
    pub banned_weapon: Weapon,
}

trait Subfield<T> {
    fn err_parent(self, parent: &str) -> Result<T, KickbotError>;
}

impl<T> Subfield<T> for Result<T, KickbotError> {
    fn err_parent(self, parent: &str) -> Result<T, KickbotError> {
        self.map_err(|err| {
            KickbotError::JsonError(format!("In field {}, {}", parent, err.to_string()))
        })
    }
}

fn parse_primitive<T>(
    object: &Value,
    cast_func: fn(&Value) -> Option<T>,
) -> Result<T, KickbotError> {
    cast_func(object).ok_or(KickbotError::JsonError(
        "Couldn't parse primitive".to_string(),
    ))
}

fn parse<T: Clone>(object: &Value, cast_func: fn(&Value) -> Option<&T>) -> Result<T, KickbotError> {
    Ok(cast_func(object)
        .ok_or(KickbotError::JsonError("Couldn't parse".to_string()))?
        .clone())
}

fn deserialize<T: Clone>(
    object: &Value,
    field: &str,
    cast_func: fn(&Value) -> Option<&T>,
) -> Result<T, KickbotError> {
    let field_object = object.get(field).ok_or(KickbotError::JsonError(format!(
        "Couldn't find field {field}"
    )))?;
    parse(field_object, cast_func).err_parent(field)
}

fn deserialize_primitive<T>(
    object: &Value,
    field: &str,
    cast_func: fn(&Value) -> Option<T>,
) -> Result<T, KickbotError> {
    let field_object = object.get(field).ok_or(KickbotError::JsonError(format!(
        "Couldn't find field {field}"
    )))?;
    parse_primitive(field_object, cast_func).err_parent(field)
}

fn cant_find(field: &str) -> KickbotError {
    KickbotError::JsonError(format!("Couldn't find field {field}"))
}

fn to_rgb(object: &Value, field: &str) -> Result<RGB, KickbotError> {
    let array = deserialize(object, field, Value::as_array)?;
    if array.len() != 3 {
        return Err(KickbotError::JsonError(format!(
            "Colour field {field} must have 3 values (RGB)"
        )));
    }
    let to_i16 = |object: &Value| -> Result<i16, KickbotError> {
        let value = parse_primitive(object, Value::as_i64).err_parent(field)?;
        Ok(value as i16)
    };

    Ok(RGB {
        r: to_i16(&array[0])?,
        g: to_i16(&array[1])?,
        b: to_i16(&array[2])?,
    })
}

fn to_rect(object: &Value, field: &str) -> Result<Rect, KickbotError> {
    let rect_object = object.get(field).ok_or(cant_find(field))?;
    let to_i32 = |subfield: &str| -> Result<i32, KickbotError> {
        let value =
            deserialize_primitive(rect_object, subfield, Value::as_i64).err_parent(field)?;
        Ok(value as i32)
    };
    Ok(Rect {
        x: to_i32("x")?,
        y: to_i32("y")?,
        width: to_i32("width")?,
        height: to_i32("height")?,
    })
}

impl Config {
    pub async fn read_config(filename: &str) -> Result<Config, KickbotError> {
        let reader = File::open(filename).map_err(|err| {
            KickbotError::JsonError(format!(
                "Failed to open file {}: {}",
                filename,
                err.to_string()
            ))
        })?;

        let json: Value = serde_json::from_reader(reader).map_err(|err| {
            KickbotError::JsonError(format!(
                "File {} is not valid JSON: {}",
                filename,
                err.to_string()
            ))
        })?;

        let banned_weapon_object = json
            .get("banned_weapon")
            .ok_or(cant_find("banned_weapon"))?;
        let weapon_names = deserialize(banned_weapon_object, "weapon_names", Value::as_array)
            .err_parent("banned_weapon")?
            .iter()
            .map(|value| value.as_str().unwrap().to_string())
            .collect();
        let banned_weapon = Weapon {
            pretty_name: "SMG08/18".to_string(),
            names: weapon_names,
        };

        //let banned_gadgets = ...

        let banned_vehicles = json
            .get("banned_vehicles")
            .ok_or(cant_find("banned_vehicles"))?;

        let get_names = |object: &Value| -> Result<(Vec<String>, Vec<String>), KickbotError> {
            let primary_names = deserialize(object, "primary_names", Value::as_array)?
                .iter()
                .map(|value| value.as_str().unwrap().to_string())
                .collect();
            let secondary_names = deserialize(object, "secondary_names", Value::as_array)?
                .iter()
                .map(|value| value.as_str().unwrap().to_string())
                .collect();
            Ok((primary_names, secondary_names))
        };

        let heavy_bomber_object = banned_vehicles
            .get("heavybomber")
            .ok_or(cant_find("heavybomber"))?;

        let (primary_names_hb, secondary_names_hb) = get_names(heavy_bomber_object)?;

        let heavy_bomber = Vehicle {
            pretty_name: "heavy bomber".to_string(),
            primary_names: primary_names_hb,
            secondary_names: secondary_names_hb,
        };

        let hmg_object = banned_vehicles.get("hmg").ok_or(cant_find("hmg"))?;
        let (primary_names_hmg, secondary_names_hmg) = get_names(hmg_object)?;

        let mortar_truck = Vehicle {
            pretty_name: "mortar truck".to_string(),
            primary_names: primary_names_hmg,
            secondary_names: secondary_names_hmg,
        };

        // can't be bothered to adapt deserialize, just hardcode
        let kick_webhook_url = json
            .get("kick_webhook")
            .ok_or(JsonError("Couldn't find kick_webhook".to_string()))?
            .as_str()
            .ok_or(JsonError("Couldn't parse kick_webhook as str".to_string()))?;
        let monitoring_webhook_url = json
            .get("monitoring_webhook")
            .ok_or(JsonError("Couldn't find monitoring_webhook".to_string()))?
            .as_str()
            .ok_or(JsonError(
                "Couldn't parse monitoring_webhook as str".to_string(),
            ))?;

        let bf1_path = json
            .get("bf1_path")
            .ok_or(JsonError("Couldn't find bf1_path".to_string()))?
            .as_str()
            .ok_or(JsonError("Couldn't parse bf1_path as str".to_string()))?;

        Ok(Config {
            bf1_path: String::from(bf1_path),
            kicks_to_ping: deserialize_primitive(&json, "kicks_to_ping", Value::as_u64)?,
            min_players_for_kick: deserialize_primitive(
                &json,
                "min_players_for_kick",
                Value::as_u64,
            )?,
            kick_webhook: DiscordWebhook::new(kick_webhook_url, "SpecBot").await?,
            monitoring_webhook: DiscordWebhook::new(monitoring_webhook_url, "SpecBot").await?,
            player_similar_name_probability: deserialize_primitive(
                &json,
                "player_similar_name_probability",
                Value::as_f64,
            )?,
            weapon_similar_name_probability: deserialize_primitive(
                &json,
                "weapon_similar_name_probability",
                Value::as_f64,
            )?,
            save_screenshots: deserialize_primitive(&json, "save_screenshots", Value::as_bool)?,
            rotate_delay: Duration::from_secs_f64(deserialize_primitive(
                &json,
                "rotate_delay",
                Value::as_f64,
            )?),
            player_name_box: to_rect(&json, "player_name_box")?,
            weapon_icon_probability: deserialize_primitive(
                &json,
                "weapon_icon_probability",
                Value::as_f64,
            )? as f32,
            weapon_icon_box: to_rect(&json, "weapon_icon_box")?,
            weapon_name_slot1_box: to_rect(&json, "weapon_slot_1_name_box")?,
            weapon_name_slot2_box: to_rect(&json, "weapon_slot_2_name_box")?,
            gadget_slot1_box: to_rect(&json, "gadget_slot_1_box")?,
            gadget_slot2_box: to_rect(&json, "gadget_slot_2_box")?,
            ally_colour: to_rgb(&json, "ally_colour")?,
            enemy_colour: to_rgb(&json, "enemy_colour")?,
            banned_vehicles: HashMap::from([
                (WeaponClasses::HeavyBomber, heavy_bomber),
                (WeaponClasses::LMG, mortar_truck),
            ]),
            banned_gadgets: vec![],
            banned_weapon,
        })
    }

    pub fn are_similar(&self, string1: &str, string2: &str, probability: f64) -> bool {
        gestalt_ratio(string1, string2) >= probability
    }
}

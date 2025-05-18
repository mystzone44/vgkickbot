use crate::api::bf1api::{rpc_request, BF1Api, Lookup};
use crate::api::endpoints;
use crate::api::errors::{BF1ApiError, BF1ApiSubError};
use reqwest::header::{HeaderMap, COOKIE};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;

#[derive(Deserialize, Debug, Default, Clone)]
pub struct ServerDetails {
    pub game_id: String,
    pub name: String,

    pub max_player_count: u64,
    pub queue_count: u64,
    pub spectator_count: u64,
    pub max_spectator_count: u64,
    pub map: String,

    pub team1: HashMap<String, String>,
    pub team1_name: String,
    pub team2: HashMap<String, String>,
    pub team2_name: String,
    pub updated: bool,
}

impl ServerDetails {
    pub fn player_count(&self) -> usize {
        self.team1.len() + self.team2.len()
    }

    pub fn is_updated(&mut self) -> bool {
        if self.updated {
            self.updated = false;
            true
        } else {
            false
        }
    }

    pub async fn update_players(&mut self, api: &BF1Api) -> Result<(), BF1ApiError> {
        let params = [("gameID", self.game_id.clone())];

        let url = reqwest::Url::parse_with_params(endpoints::GAMETOOLS, params).unwrap();
        let response = api.client.get(url).send().await?.error_for_status()?;

        let response_json: Map<String, Value> =
            serde_json::from_str(response.text().await?.as_str())?;

        let teams = response_json.lookup("teams")?.as_array().unwrap();
        let team1Name = teams[0].lookup("name")?.as_str().unwrap();
        let team1Object = teams[0]
            .as_object()
            .unwrap()
            .lookup("players")?
            .as_array()
            .unwrap();
        let team2Name = teams[1].lookup("name")?.as_str().unwrap();
        let team2Object = teams[1]
            .as_object()
            .unwrap()
            .lookup("players")?
            .as_array()
            .unwrap();

        let GetTeam = |team: &mut HashMap<String, String>, teamObject: &Vec<Value>| {
            for player in teamObject.iter() {
                // Need to append platoon tag because it shows up in the spectator player name view
                let player_object = player.as_object().unwrap();
                let mut player_name = match player_object.get("name") {
                    None => continue,
                    Some(player_name) => player_name.to_string().replace("\"", ""),
                };
                let player_id = match player_object.get("player_id") {
                    None => continue,
                    Some(player_id) => player_id,
                }
                .to_string();
                if let Some(platoon) = player.get("platoon") {
                    let platoon_string = platoon.to_string().replace("\"", "");
                    if !platoon_string.is_empty() {
                        player_name = format!("[{}]{}", platoon_string, player_name);
                    }
                }
                team.insert(player_name, player_id);
            }
        };

        self.team1.clear();
        self.team2.clear();
        GetTeam(&mut self.team1, &team1Object);
        GetTeam(&mut self.team2, &team2Object);
        self.team1_name = team1Name.to_string();
        self.team2_name = team2Name.to_string();

        Ok(())
    }

    pub async fn update_server_details(
        &mut self,
        api: &BF1Api,
        mut game_id: Option<String>,
    ) -> Result<(), BF1ApiError> {
        let gameId = game_id.get_or_insert(self.game_id.clone());
        let result = api.get_server_by_game_id(gameId).await?;
        let slots = result.lookup("slots")?;
        self.decode_server_details_json(&result, &slots)
    }

    pub fn decode_server_details_json(
        &mut self,
        gameservers: &Value,
        slots: &Value,
    ) -> Result<(), BF1ApiError> {
        self.game_id = gameservers.lookup("gameId")?.as_str().unwrap().to_string();
        self.queue_count = slots.lookup("Queue")?.lookup("current")?.as_u64().unwrap();
        self.spectator_count = slots
            .lookup("Spectator")?
            .lookup("current")?
            .as_u64()
            .unwrap();
        self.map = gameservers
            .lookup("mapNamePretty")?
            .as_str()
            .unwrap()
            .to_string();
        Ok(())
    }
}

impl BF1Api {
    pub async fn get_server_by_name(
        &self,
        server_name: &str,
    ) -> Result<ServerDetails, BF1ApiError> {
        let params: HashMap<&str, Value> = HashMap::from([
            (
                "filterJson",
                Value::String(format!("{{\"version\":6,\"name\":\"{}\"}}", server_name)),
            ),
            ("game", Value::String("tunguska".to_string())),
            ("limit", Value::String("30".to_string())),
            ("protocolVersion", Value::String("3779779".to_string())),
        ]);
        let body = rpc_request("GameServer.searchServers".to_string(), params);
        let response = self
            .client
            .post(endpoints::RPC_HOST)
            .headers(self.rpc_header.clone())
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        let response_json: Map<String, Value> =
            serde_json::from_str(response.text().await?.as_str())?;

        let result = response_json.lookup("result")?;

        let gameservers = result
            .get("gameservers")
            .ok_or(BF1ApiSubError::JsonError(
                "Couldn't get gameservers".to_string(),
            ))?
            .as_array()
            .ok_or(BF1ApiSubError::JsonError(
                "Couldn't parse to array".to_string(),
            ))?
            .get(0)
            .ok_or(BF1ApiSubError::JsonError(
                "Couldn't get get first result".to_string(),
            ))?;

        let mut server = ServerDetails::default();
        let slots = gameservers.lookup("slots")?;
        server.name = gameservers.lookup("name")?.as_str().unwrap().to_string();
        server.max_player_count = slots.lookup("Soldier")?.lookup("max")?.as_u64().unwrap();
        server.max_spectator_count = slots.lookup("Spectator")?.lookup("max")?.as_u64().unwrap();

        server.decode_server_details_json(gameservers, slots)?;

        server.update_players(&self).await?;

        Ok(server)
    }

    pub async fn get_server_by_game_id(&self, game_id: &String) -> Result<Value, BF1ApiError> {
        let json_body = rpc_request(
            "GameServer.getServerDetails".to_string(),
            HashMap::from([
                ("game", Value::String("tunguska".to_string())),
                ("gameId", Value::String(game_id.to_string())),
            ]),
        );

        let response = self
            .client
            .post(endpoints::RPC_HOST)
            .headers(self.rpc_header.clone())
            .json(&json_body)
            .send()
            .await?
            .error_for_status()?;

        let response_json: Map<String, Value> =
            serde_json::from_str(response.text().await?.as_str())?;

        Ok(response_json
            .get("result")
            .ok_or(BF1ApiSubError::JsonError("Couldn't get result".to_string()))?
            .clone())
    }

    pub async fn leave_game(&self, game_id: String) -> Result<(), BF1ApiError> {
        let params: HashMap<&str, Value> = HashMap::from([
            ("game", Value::String("tunguska".to_string())),
            ("gameId", Value::String(game_id)),
        ]);

        let body = rpc_request("Game.joinGame".to_string(), params);

        let response = self
            .client
            .post(endpoints::RPC_HOST)
            .headers(self.rpc_header.clone())
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        println!("Response: {}", response.text().await?);

        Ok(())
    }
}

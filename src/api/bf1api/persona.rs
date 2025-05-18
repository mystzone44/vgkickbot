use crate::api::bf1api::{rpc_request, BF1Api, Lookup};
use crate::api::endpoints;
use crate::api::errors::{BF1ApiError, BF1ApiSubError};
use crate::errors::KickbotError;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::str::FromStr;

impl BF1Api {
    pub async fn get_display_names_by_persona_ids(
        &self,
        persona_ids: Vec<&str>,
    ) -> Result<Vec<String>, BF1ApiError> {
        let values = persona_ids
            .iter()
            .map(|id| Value::String(id.to_string()))
            .collect();
        let params: HashMap<&str, Value> = HashMap::from([
            ("game", Value::String("tunguska".to_string())),
            ("personaIds", values),
        ]);
        let body = rpc_request("RSP.getPersonasByIds".to_string(), params);

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

        let mut display_names: Vec<String> = Vec::new();

        for persona_id in persona_ids {
            let persona = result
                .get(persona_id)
                .ok_or(BF1ApiSubError::JsonError(format!(
                    "Didn't find persona with id {} in 'result' field",
                    persona_id
                )))?;
            let display_name = persona
                .get("displayName")
                .ok_or(BF1ApiSubError::JsonError(format!(
                    "Display name field not found for persona with id {}",
                    persona_id
                )))?
                .as_str()
                .ok_or(BF1ApiSubError::JsonError(format!(
                    "Couldn't parse 'displayName' field as string for persona with id {}",
                    persona_id
                )))?
                .to_string();
            display_names.push(display_name);
        }

        Ok(display_names)
    }

    pub async fn get_player_persona_by_name(
        &self,
        player_name: &str,
    ) -> Result<String, BF1ApiError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_str("X-Expand-Results").unwrap(),
            HeaderValue::from_str("True").unwrap(),
        );
        headers.insert(
            HeaderName::from_str("Authorization").unwrap(),
            HeaderValue::from_str(format!("Bearer {}", self.access_token.access_token).as_str())
                .unwrap(),
        );
        let response = self
            .client
            .get(format!("{}{}", endpoints::IDENTITY_HOST, player_name))
            .headers(headers)
            .send()
            .await?
            .error_for_status()?;

        let response_json: Map<String, Value> =
            serde_json::from_str(response.text().await?.as_str())?;

        let persona = response_json
            .lookup("personas")?
            .lookup("persona")?
            .as_array()
            .ok_or(BF1ApiSubError::JsonError(
                "Couldn't parse persona as array".to_string(),
            ))?[0]
            .as_object()
            .ok_or(BF1ApiSubError::JsonError(
                "Couldn't parse as object".to_string(),
            ))?;
        let persona_id = persona.lookup("personaId")?;

        Ok(persona_id.to_string())
    }

    pub async fn get_servers_by_persona_ids(
        &self,
        persona_ids: Vec<String>,
    ) -> Result<(), BF1ApiError> {
        let json_body = rpc_request(
            "GameServer.getServersByPersonaIds".to_string(),
            HashMap::from([
                ("game", Value::String("tunguska".to_string())),
                (
                    "personaIds",
                    Value::Array(persona_ids.iter().map(|id| Value::String(id.clone())).collect()),
                ),
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

        println!("{:?}", response_json);

        Ok(())
    }
}

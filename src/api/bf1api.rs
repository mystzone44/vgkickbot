use crate::api::endpoints;
use crate::api::errors::{ApiResultExt, BF1ApiError, BF1ApiSubError};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, COOKIE};
use reqwest::redirect::Policy;
use reqwest::{cookie, Client, StatusCode};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::env;
use std::str::FromStr;
use uuid::Uuid;

mod kick;
mod persona;
pub mod server;

trait Lookup {
    fn lookup(&self, search_term: &str) -> Result<&Value, BF1ApiSubError>;
}

impl Lookup for Map<String, Value> {
    fn lookup(&self, search_term: &str) -> Result<&Value, BF1ApiSubError> {
        self.get(search_term)
            .ok_or(BF1ApiSubError::JsonError(format!(
                "Didn't find '{}' field",
                search_term
            )))
    }
}

impl Lookup for Value {
    fn lookup(&self, search_term: &str) -> Result<&Value, BF1ApiSubError> {
        self.as_object().unwrap().lookup(search_term)
    }
}

#[derive(Deserialize, Debug)]
struct AccessToken {
    access_token: String,
}

struct ResponseAuth {
    remid: String,
    sid: String,
    code: String,
}

async fn get_auth_code(
    client: &Client,
    rest_headers: HeaderMap,
) -> Result<ResponseAuth, BF1ApiError> {
    let response = client
        .get(endpoints::AUTH_HOST)
        .headers(rest_headers)
        .send()
        .await?
        .error_for_status()?;
    let mut resp_auth = ResponseAuth {
        remid: String::from(""),
        sid: String::from(""),
        code: String::from(""),
    };
    if response.status() == StatusCode::FOUND {
        let location = response
            .headers()
            .get("location")
            .unwrap()
            .to_str()
            .unwrap();
        if location.contains("127.0.0.1/success?code=") {
            let cookies: Vec<cookie::Cookie> = response.cookies().collect();
            if cookies.len() == 2 {
                (resp_auth.remid, resp_auth.sid) = (
                    cookies[0].value().to_string(),
                    cookies[1].value().to_string(),
                );
            } else if cookies.len() == 1 {
                resp_auth.sid = cookies[0].value().to_string();
            }
        }
        resp_auth.code = location.replace("http://127.0.0.1/success?code=", "");
    }

    Ok(resp_auth)
}

#[derive(Serialize, Debug)]
struct RPC {
    id: String,
    jsonrpc: String,
    method: String,
    params: HashMap<String, Value>,
}

pub fn rpc_request(method: String, params: HashMap<&str, Value>) -> RPC {
    let params = params
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    RPC {
        id: Uuid::new_v4().to_string(),
        jsonrpc: "2.0".to_string(),
        method,
        params,
    }
}

async fn get_session_and_persona_ids_by_authcode(
    client: &Client,
    auth_code: &str,
) -> Result<(String, String), BF1ApiError> {
    let params: HashMap<&str, Value> = HashMap::from([
        ("authCode", Value::String(auth_code.to_string())),
        ("locale", Value::String("en-GB".to_string())),
    ]);
    let body = rpc_request("Authentication.getEnvIdViaAuthCode".to_string(), params);
    let response = client
        .post(endpoints::RPC_HOST)
        .json(&body)
        .send()
        .await?
        .error_for_status()?;

    let mut response_json: Map<String, Value> =
        serde_json::from_str(response.text().await?.as_str())?;

    let result = response_json.lookup("result")?.as_object().unwrap();

    let session_id = result
        .lookup("sessionId")?
        .as_str()
        .ok_or(BF1ApiSubError::JsonError(
            "Couldn't parse 'sessionId' field as string".to_string(),
        ))?
        .to_string();

    let persona_id = result
        .lookup("personaId")?
        .as_str()
        .ok_or(BF1ApiSubError::JsonError(
            "Couldn't parse 'personaId' field as string".to_string(),
        ))?
        .to_string();

    Ok((session_id, persona_id))
}

async fn get_access_token(
    client: &Client,
    rest_headers: HeaderMap,
) -> Result<AccessToken, BF1ApiError> {
    let response = client
        .get(endpoints::ACCESS_HOST)
        .headers(rest_headers)
        .send()
        .await?
        .error_for_status()?;

    Ok(response.json::<AccessToken>().await?)
}

#[derive(Debug)]
pub struct BF1Api {
    client: Client,
    rest_headers: HeaderMap,
    access_token: AccessToken,
    rpc_header: HeaderMap,
    session_id: String,
    persona_id: String,
}

impl BF1Api {
    pub async fn new() -> Result<BF1Api, BF1ApiError> {
        if let None = dotenv::dotenv().ok() {
            return Err(BF1ApiSubError::EnvError(String::from("No .env file found")).into());
        }
        let sid = env::var("SID").map_err(|err| BF1ApiSubError::VarError {
            var: String::from("SID"),
            err,
        })?;
        let remid = env::var("REMID").map_err(|err| BF1ApiSubError::VarError {
            var: String::from("REMID"),
            err,
        })?;

        let mut rest_headers = HeaderMap::new();
        rest_headers.insert(
            COOKIE,
            format!("remid={};sid={};", remid, sid).parse().unwrap(),
        );
        let client = Client::new();
        let access_token = get_access_token(&client, rest_headers.clone())
            .await
            .provide_api_function("Get Access Token")?;

        println!("Access Token: {}", access_token.access_token);

        let auth_code_client = Client::builder().redirect(Policy::none()).build().unwrap();
        let resp_auth = get_auth_code(&auth_code_client, rest_headers.clone())
            .await
            .provide_api_function("Get Auth Code")?;

        println!("Resp Auth Remid: {}", resp_auth.remid);
        println!("Resp Auth Sid: {}", resp_auth.sid);
        println!("Resp Auth Code: {}", resp_auth.code);
        let (session_id, persona_id) =
            get_session_and_persona_ids_by_authcode(&client, resp_auth.code.as_str()).await?;

        println!("Session ID: {}, Persona ID: {}", session_id, persona_id);

        let mut rpc_header = HeaderMap::new();
        rpc_header.insert(
            HeaderName::from_str("X-GatewaySession").unwrap(),
            HeaderValue::from_str(session_id.clone().as_str()).unwrap(),
        );

        Ok(BF1Api {
            client,
            rest_headers,
            access_token,
            rpc_header,
            session_id,
            persona_id,
        })
    }

    pub fn persona_id(&self) -> String {
        self.persona_id.clone()
    }
}

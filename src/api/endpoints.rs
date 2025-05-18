// get_access_token
pub const ACCESS_HOST: &str = "https://accounts.ea.com/connect/auth?response_type=token&locale=en-US&client_id=ORIGIN_JS_SDK&redirect_uri=nucleus%3Arest";

// get_auth_code
pub const AUTH_HOST: &str = "https://accounts.ea.com/connect/auth?client_id=sparta-backend-as-user-pc&response_type=code&release_type=none";

// get_full_server_details_by_game_id, get_persona_by_id, get_servers_by_persona_id, get_session_id_by_authcode, kick_player, search_server_by_name
pub const RPC_HOST: &str = "https://sparta-gw.battlelog.com/jsonrpc/pc/api";

// get_player_persona_by_name
pub const IDENTITY_HOST: &str =
    "https://gateway.ea.com/proxy/identity/personas?namespaceName=cem_ea_id&displayName=";

// get_players_by_game_id
pub const GAMETOOLS: &str = "https://api.gametools.network/bf1/players/";

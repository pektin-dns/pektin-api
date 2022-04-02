use std::time::{SystemTime, UNIX_EPOCH};

use actix_web::HttpRequest;

use crate::{
    return_if_err,
    ribston::{self, RibstonRequestData},
    types::{AppState, AuthAnswer, RequestBody},
    vault,
};

pub async fn auth(
    vault_endpoint: &str,
    vault_api_pw: &str,
    ribston_endpoint: &str,
    client_username: &str,
    confidant_password: &str,
    ribston_request_data: RibstonRequestData,
) -> AuthAnswer {
    // TODO reuse reqwest::Client, caching, await concurrently where possible

    // cache until invalid
    let api_token = return_if_err!(
        vault::login_userpass(vault_endpoint, "pektin-api", vault_api_pw).await,
        err,
        format!("Could not get Vault token for pektin-api: {}", err)
    );

    /*
    struct ClientCache {
        // evict after 10min
        confidant_token: Option<(Instant, String)>,
        client_policy: String,
        policy_results: HashMap<Request, RibstonResponseData>,
    }
    */

    // cache for some amount of time (10min-30min)
    let confidant_token = return_if_err!(
        vault::login_userpass(
            vault_endpoint,
            &format!("pektin-client-{}-confidant", client_username),
            confidant_password
        )
        .await,
        err,
        format!("Could not get Vault token for confidant: {}", err)
    );

    // cache until restart
    let client_policy = return_if_err!(
        vault::get_policy(vault_endpoint, &api_token, client_username).await,
        err,
        format!("Could not get client policy: {}", err)
    );

    if client_policy.contains("@skip-ribston") {
        return AuthAnswer {
            success: true,
            message: "Skipped evaluating policy".into(),
        };
    }

    let ribston_answer = return_if_err!(
        ribston::evaluate(ribston_endpoint, &client_policy, ribston_request_data).await,
        err,
        format!("Could not evaluate client policy: {}", err)
    );

    AuthAnswer {
        success: ribston_answer.status == "SUCCESS" && ribston_answer.data.status == "SUCCESS",
        message: if ribston_answer.status == "SUCCESS" {
            format!(
                "Ribston policy evaluation returned: {}",
                ribston_answer.data.message
            )
        } else {
            ribston_answer.message
        },
    }
}

pub async fn auth_ok(
    req: &HttpRequest,
    request_body: RequestBody,
    state: &AppState,
    client_username: &str,
    confidant_password: &str,
) -> AuthAnswer {
    if "yes, I really want to disable authentication" == state.skip_auth {
        return AuthAnswer {
            success: true,
            message: "Skipped authentication because SKIP_AUTH is set".into(),
        };
    }

    let start = SystemTime::now();
    let utc_millis = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis();

    let api_method = match request_body {
        RequestBody::Get { .. } => "get",
        RequestBody::GetZoneRecords { .. } => "get-zone-records",
        RequestBody::Set { .. } => "set",
        RequestBody::Delete { .. } => "delete",
        RequestBody::Search { .. } => "search",
        RequestBody::Health => "health",
    }
    .into();

    let res = auth(
        &state.vault_uri,
        &state.vault_password,
        &state.ribston_uri,
        client_username,
        confidant_password,
        RibstonRequestData {
            api_method,
            ip: req
                .connection_info()
                .realip_remote_addr()
                .map(|s| s.to_string()),
            user_agent: "TODO user agent".into(),
            utc_millis,
            request_body,
        },
    )
    .await;
    dbg!(&res);
    res
}
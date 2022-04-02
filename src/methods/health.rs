use std::ops::Deref;

use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use serde_json::json;

use crate::{
    auth::auth_ok,
    errors_and_responses::success_with_toplevel_data,
    ribston,
    types::{AppState, HealthRequestBody},
    vault,
};

#[post("/health")]
pub async fn health(
    req: HttpRequest,
    req_body: web::Json<HealthRequestBody>,
    state: web::Data<AppState>,
) -> impl Responder {
    let mut auth = auth_ok(
        &req,
        req_body.clone().into(),
        state.deref(),
        &req_body.client_username,
        &req_body.confidant_password,
    )
    .await;
    if auth.success {
        let redis_con = state.redis_pool.get().await;
        let vault_status = vault::get_health(&state.vault_uri).await;
        let ribston_status = ribston::get_health(&state.ribston_uri).await;

        let all_ok = redis_con.is_ok() && vault_status == 200 && ribston_status == 200;

        let mut message =
            String::from("Pektin API is healthy but lonely without a good relation with");

        if redis_con.is_err() && vault_status != 200 && ribston_status != 200 {
            message = format!("{} {}", message, "Redis, Vault, and Ribston.")
        } else if redis_con.is_err() && vault_status != 200 {
            message = format!("{} {}", message, "Redis and Vault.")
        } else if redis_con.is_err() && ribston_status != 200 {
            message = format!("{} {}", message, "Redis and Ribston.")
        } else if vault_status != 200 && ribston_status != 200 {
            message = format!("{} {}", message, "Vault and Ribston.")
        } else if redis_con.is_err() {
            message = format!("{} {}", message, "Redis.")
        } else if vault_status != 200 {
            message = format!("{} {}", message, "Vault.")
        } else if ribston_status != 200 {
            message = format!("{} {}", message, "Ribston.")
        } else {
            message = String::from("Pektin API is feelin' good today.")
        };

        success_with_toplevel_data(
            message,
            json!({
                "api": true,
                "db": redis_con.is_ok(),
                "vault": vault_status,
                "ribston": ribston_status,
                "all": all_ok,
            }),
        )
    } else {
        auth.message.push('\n');
        HttpResponse::Unauthorized().body(auth.message)
    }
}

use worker::*;
use serde::Deserialize;
use crate::{error, read_config, attach_origin_to_header};

use crate::constants::*;
use crate::prompt::Config;
use serde_json::json;

async fn verify_identity(req: &Request, env: &Env) -> crate::Result<()> {
    if env.var("DEV_MODE")?.to_string() == "1" {
        console_log!("DEV_MODE is on, skipping identity verification");
        return Ok(());
    }
    let admin_name = env
        .var("ADMIN_NAME")
        .expect("ADMIN_NAME not provided")
        .to_string();
    let admin_emails_concat = env
        .var("ADMIN_EMAILS")
        .expect("ADMIN_EMAILS not provided")
        .to_string(); // comma separated
    let admin_emails: Vec<&str> = admin_emails_concat.split(',').collect();

    let api = format!(
        "https://{}.cloudflareaccess.com/cdn-cgi/access/get-identity",
        admin_name
    );

    // for now, we trust Cloudflare and send all cookies for simplicity
    let cookie = req
        .headers()
        .get("Cookie")?
        .ok_or_else(|| error::Error::InvalidRequest("Missing Cookies".to_string()))?;
    let mut headers = Headers::new();
    headers.set("Cookie", &cookie)?;
    let req = Request::new_with_init(
        &api,
        RequestInit::new()
            .with_method(Method::Get)
            .with_headers(headers),
    )?;
    let mut resp = Fetch::Request(req).send().await?;
    #[derive(Deserialize)]
    struct Identity {
        err: Option<String>,
        email: Option<String>,
    }
    let identity: Identity = resp.json().await?;
    if let Some(_) = identity.err {
        return Err(error::Error::Forbidden);
    }
    let email = identity.email.ok_or_else(|| error::Error::Forbidden)?;

    if !admin_emails.contains(&email.as_str()) {
        console_log!("{} tried to access admin panel but failed", email);
        return Err(error::Error::Forbidden);
    }

    Ok(())
}

async fn set_config(config: &Config, ctx: &RouteContext<()>) -> Result<()> {
    let kv = ctx.kv(KV_BINDING)?;
    kv.put("config", config)?.execute().await?;
    Ok(())
}

async fn set_config_backup(config: &Config, ctx: &RouteContext<()>) -> Result<()> {
    let time_string = Date::now().to_string();
    let kv = ctx.kv(KV_BINDING)?;
    kv.put(&format!("config_backup_{}", time_string), config)?
        // expire in 1 year
        .expiration_ttl(60 * 60 * 24 * 365)
        .execute()
        .await?;
    Ok(())
}

pub async fn handle_config_get(req: Request, ctx: RouteContext<()>) -> crate::Result<Response> {
    verify_identity(&req, &ctx.env).await?;

    let config = read_config(&ctx).await?;
    let mut resp = Response::from_json(&config)?;
    attach_origin_to_header(&req, resp.headers_mut())?;
    Ok(resp)
}

pub async fn handle_config_post(mut req: Request, ctx: RouteContext<()>) -> crate::Result<Response> {
    verify_identity(&req, &ctx.env).await?;

    // get old config
    let old_config = read_config(&ctx).await?;
    // backup old config
    set_config_backup(&old_config, &ctx).await?;

    let config: Config = req.json().await?;
    set_config(&config, &ctx).await?;

    let mut resp = Response::from_json(&json!({
        "success": true,
    }))?;
    attach_origin_to_header(&req, resp.headers_mut())?;
    console_log!("config updated");
    Ok(resp)
}
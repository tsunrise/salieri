use crate::error::Result;
use futures_util::StreamExt;
use prompt::{Config, Prompt, UserRequest};
use rand::{seq::SliceRandom, SeedableRng};
use serde_json::json;
use wasm_bindgen_futures::spawn_local;
use worker::{
    console_error, console_log, event, js_sys::encode_uri_component, wasm_bindgen::JsValue,
    wasm_bindgen_futures, Date, Env, Fetch, Headers, Method, Request, RequestInit, Response,
    Result as WorkerResult, RouteContext, Router, WebSocket, WebSocketPair, WebsocketEvent,
};

mod error;
mod prompt;
mod stream_parser;
mod utils;
mod admin;
mod constants;
mod id;

use constants::*;

use crate::{prompt::RequestToOpenAI, stream_parser::StreamItem};

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or_else(|| "unknown region".into())
    );
}


async fn read_config(ctx: &RouteContext<()>) -> Result<Config> {
    // toml::from_str::<Config>(include_str!("../config.toml")).expect("invalid config.toml")
    let kv = ctx.kv(KV_BINDING)?;
    let config = kv
        .get("config")
        .json::<Config>()
        .await?
        .ok_or_else(|| error::Error::InternalError("config not found".into()))?;
    Ok(config)
}

const ALLOWED_ORIGINS: [&str; 3] = [
    "https://tomshen.io",
    "http://localhost:3000",
    "https://salieri-admin.tomshen.io",
];

fn allowed_origin_header(origin: &str) -> Result<String> {
    if ALLOWED_ORIGINS.contains(&origin) {
        Ok(origin.to_string())
    } else {
        Err(error::Error::InvalidRequest(format!(
            "origin {} is not allowed",
            origin
        )))
    }
}

fn attach_origin_to_header(req: &Request, header: &mut Headers) -> Result<()> {
    let origin = req.headers().get("Origin")?;
    match origin {
        Some(x) => {
            header.append("Access-Control-Allow-Origin", &allowed_origin_header(&x)?)?;
            Ok(())
        }
        None => {
            // probably from curl or direct access from browser, append the first allowed origin
            header.append("Access-Control-Allow-Origin", &ALLOWED_ORIGINS[0])?;
            Ok(())
        }
    }
}

fn attach_origin_header_to_resp(req: &Request, resp: &mut Response) -> Result<()> {
    let mut headers = resp.headers_mut();
    Ok(attach_origin_to_header(req, &mut headers)?)
}

#[derive(serde::Deserialize)]
struct CaptchaResponse {
    success: bool,
    #[serde(rename = "error-codes")]
    error_codes: Vec<String>,
}

fn encode_form_data(s: &str) -> Result<String> {
    Ok(encode_uri_component(s)
        .as_string()
        .ok_or(error::Error::InvalidRequest(
            "invalid form data".to_string(),
        ))?)
}
async fn verify_captcha(
    token: &str,
    turnstile_secret_key: &str,
    remote_ip: &str,
) -> Result<CaptchaResponse> {
    // let mut form_data = FormData::new();
    // form_data.append("secret", turnstile_secret_key)?;
    // form_data.append("token", &token)?;
    // form_data.append("remoteip", remote_ip)?;
    // we write form body manually because RequestInit::withbody is not supported with FormDat

    let mut form_body = String::new();
    form_body.push_str("secret=");
    form_body.push_str(&encode_form_data(turnstile_secret_key)?);
    form_body.push_str("&response=");
    form_body.push_str(&encode_form_data(token)?);
    form_body.push_str("&remoteip=");
    form_body.push_str(&encode_form_data(remote_ip)?);

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_body(Some(JsValue::from_str(&form_body)));
    let mut req = Request::new_with_init(
        "https://challenges.cloudflare.com/turnstile/v0/siteverify",
        &init,
    )?;
    req.headers_mut()?
        .set("Content-Type", "application/x-www-form-urlencoded")?;

    Ok(Fetch::Request(req)
        .send()
        .await?
        .json::<CaptchaResponse>()
        .await?)
}

#[derive(serde::Serialize)]
pub struct EndMessage{
    pub id: String
}

// TODO: enforce length limit, and cloudflare turnstile
pub async fn serve_chat_in_ws(
    openai_key: &str,
    turnstile_secret_key: &str,
    remote_ip: &str,
    location: &str,
    timezone: impl chrono::TimeZone,
    server: WebSocket,
    prompt: Prompt,
    log_kv: &worker::kv::KvStore,
) -> Result<()> {
    // wait for the first message from the client
    let first_msg = server.events()?.next().await.unwrap()?;
    let user_request = match first_msg {
        WebsocketEvent::Message(msg) => msg.json::<UserRequest>()?,
        WebsocketEvent::Close(_) => {
            // server.close::<String>(None, None)?;
            return Ok(());
        }
    };

    // verify captcha
    let captcha_token = user_request
        .captcha_token
        .ok_or(error::Error::InvalidRequest(
            "captcha token is missing".to_string(),
        ))?;
    let captcha_resp = verify_captcha(&captcha_token, turnstile_secret_key, remote_ip).await?;
    if !captcha_resp.success {
        return Err(error::Error::InvalidRequest(format!(
            "captcha verification failed: {:?}",
            captcha_resp.error_codes
        )));
    }

    let request_to_openai = RequestToOpenAI::new(prompt, user_request.question.clone(), timezone)?;
    server.send(&StreamItem::Start(request_to_openai.max_tokens))?;

    let auth_text = "Bearer ".to_string() + openai_key;

    let mut headers = Headers::new();
    headers.append("Content-Type", "application/json")?;
    headers.append("Authorization", &auth_text)?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    let body = serde_json::to_string(&request_to_openai)?;
    init.with_body(Some(JsValue::from_str(&body)));

    let request = Request::new_with_init("https://api.openai.com/v1/chat/completions", &init)?;

    let mut response = Fetch::Request(request).send().await?;
    if response.status_code() != 200 {
        return Err(error::Error::OpenAIError(
            response.status_code(),
            response.text().await?,
        ));
    }
    let body = response.stream()?;

    let mut json_stream = stream_parser::ChatStreamParser::parse_byte_stream(body);
    let mut chatbot_answer = String::new();

    while let Some(msg) = json_stream.next().await {
        match msg {
            Err(_) => {
                server.send(&StreamItem::Finish(
                    stream_parser::FinishReason::Unavailable,
                ))?;
            }
            Ok(StreamItem::RoleMsg) => continue,
            Ok(msg) => {
                if let StreamItem::Delta(delta) = &msg {
                    chatbot_answer.push_str(&delta);
                }
                server.send(&msg)?
            }
        }
    }

    // create an ID for this chat
    let id = id::make_id();
    let end_message = EndMessage{id: id.clone()};
    let timestamp = id::get_utc_timestamp_sec();

    // log the chat to KV
    log_kv.put(&id, json!({
        "question": user_request.question,
        "response": chatbot_answer,
        "remote_ip": remote_ip,
        "location": location,
        "timestamp": timestamp,
    }))?.execute().await?;

    // send the end message
    server.send(&end_message)?;

    Ok(())
}

pub async fn handle_chat(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    req.cf().timezone();
    let upgrade_header = req.headers().get("Upgrade")?;
    match upgrade_header {
        Some(x) if x == "websocket" => {
            // sounds good
        }
        _ => {
            return Ok(Response::error("Expected Upgrade: websocket", 426)?);
        }
    }

    let ws_pair = WebSocketPair::new()?;
    let server = ws_pair.server;
    let client = ws_pair.client;
    server.accept()?;

    let openai_key = ctx.var("OPENAI_API_KEY")?.to_string();
    let turnstile_secret_key = ctx.var("TURNSTILE_SECRET_KEY")?.to_string();
    let remote_ip = req.headers().get("CF-Connecting-IP")?.unwrap();
    let cf = req.cf();
    let location = format!(
        "{} - {} - {} - {:?}",
        cf.colo(),
        cf.country().unwrap_or_else(|| "unknown".to_string()),
        cf.city().unwrap_or_else(|| "unknown".to_string()),
        cf.coordinates().unwrap_or_else(|| (0., 0.)),
    );
    let timezone = cf.timezone();

    let config = read_config(&ctx).await?;
    let prompt = config.prompt;

    let log_kv = ctx.kv(KV_LOG_BINDING)?;

    spawn_local(async move {
        let server_clone = server.clone();

        match serve_chat_in_ws(
            &openai_key,
            &turnstile_secret_key,
            &remote_ip,
            &location,
            timezone,
            server,
            prompt,
            &log_kv,
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                console_log!("error: {:?}", e);
                server_clone.send(&StreamItem::from(e)).unwrap();
            }
        }

        server_clone.close(Some(1000), Some("done")).unwrap();
    });

    let mut resp = Response::from_websocket(client)?;
    attach_origin_header_to_resp(&req, &mut resp)?;
    Ok(resp)
}

pub async fn handle_hint(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = read_config(&ctx).await?;
    let mut questions = config.questions;

    const NUM_QUESTIONS_SAMPLED: usize = 3;
    let seed = Date::now().as_millis();
    let mut rng = rand_xorshift::XorShiftRng::seed_from_u64(seed);
    questions.shuffle(&mut rng);
    let sampled_questions = &questions[..NUM_QUESTIONS_SAMPLED];

    let mut resp = Response::from_json(&json!({
        "welcome": config.welcome,
        "suggested_questions": sampled_questions,
        "announcement": config.announcement,
    }))?;
    attach_origin_to_header(&req, resp.headers_mut())?;
    Ok(resp)
}

fn result_to_response(result: Result<Response>) -> Response {
    match result {
        Ok(response) => response,
        Err(e) => {
            console_error!("error: {:?}", e);
            Response::from(e)
        }
    }
}

fn handle_options(req: Request) -> Result<Response> {
    let mut resp = Response::empty()?;
    attach_origin_to_header(&req, resp.headers_mut())?;
    Ok(resp)
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> WorkerResult<Response> {
    log_request(&req);

    // Optionally, get more helpful error messages written to the console in the case of a panic.
    utils::set_panic_hook();

    let router = Router::new();

    router
        .get_async("/api/salieri/chat", |req, ctx| async move {
            let result = handle_chat(req, ctx).await;
            Ok(result_to_response(result))
        })
        .get_async("/api/salieri/hint", |req, ctx| async move {
            let result = handle_hint(req, ctx).await;
            Ok(result_to_response(result))
        })
        .get_async("/api/salieri/config", |req, ctx| async move {
            let result = admin::handle_config_get(req, ctx).await;
            Ok(result_to_response(result))
        })
        .post_async("/api/salieri/config", |req, ctx| async move {
            let result: std::result::Result<Response, error::Error> = admin::handle_config_post(req, ctx).await;
            Ok(result_to_response(result))
        })
        .options_async("/api/salieri/:any", |req, _| async move {
            let result = handle_options(req);
            Ok(result_to_response(result))
        })
        .run(req, env)
        .await
}

use crate::error::Result;
use futures_util::StreamExt;
use prompt::{Config, Prompt, UserRequest};
use rand::{seq::IteratorRandom, SeedableRng};
use serde_json::json;
use wasm_bindgen_futures::spawn_local;
use worker::{
    console_error, console_log, event, wasm_bindgen::JsValue, wasm_bindgen_futures, Date, Env,
    Fetch, Headers, Method, Request, RequestInit, Response, Result as WorkerResult, RouteContext,
    Router, WebSocket, WebSocketPair, WebsocketEvent,
};

mod error;
mod prompt;
mod stream_parser;
mod utils;

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

fn read_config() -> Config {
    toml::from_str::<Config>(include_str!("../config.toml")).expect("invalid config.toml")
}

const ALLOWED_ORIGINS: [&str; 3] = [
    "https://tomshen.io",
    "https://v2.tomshen.pages.dev",
    "http://localhost:3000",
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
    form_body.push_str(turnstile_secret_key);
    form_body.push_str("&response=");
    form_body.push_str(&token);
    form_body.push_str("&remoteip=");
    form_body.push_str(remote_ip);

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

// TODO: enforce length limit, and cloudflare turnstile
pub async fn serve_chat_in_ws(
    openai_key: &str,
    turnstile_secret_key: &str,
    remote_ip: &str,
    server: WebSocket,
    prompt: Prompt,
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

    let request_to_openai = RequestToOpenAI::new(prompt, user_request.question)?;
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

    while let Some(msg) = json_stream.next().await {
        match msg {
            Err(_) => {
                server.send(&StreamItem::Finish(
                    stream_parser::FinishReason::Unavailable,
                ))?;
            }
            Ok(StreamItem::RoleMsg) => continue,
            Ok(msg) => server.send(&msg)?,
        }
    }

    Ok(())
}

pub fn handle_chat(req: Request, ctx: RouteContext<()>) -> Result<Response> {
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

    let config = read_config();
    let prompt = config.prompt;

    spawn_local(async move {
        let server_clone = server.clone();

        match serve_chat_in_ws(
            &openai_key,
            &turnstile_secret_key,
            &remote_ip,
            server,
            prompt,
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                console_log!("error: {:?}", e);
                server_clone.send(&StreamItem::from(e)).unwrap();
            }
        }
    });

    let mut resp = Response::from_websocket(client)?;
    attach_origin_header_to_resp(&req, &mut resp)?;
    Ok(resp)
}

pub async fn handle_hint(req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    let config = read_config();
    let questions = config.questions;

    const NUM_QUESTIONS_SAMPLED: usize = 3;
    let seed = Date::now().as_millis();
    let mut rng = rand_xorshift::XorShiftRng::seed_from_u64(seed);
    let sampled_questions = questions
        .iter()
        .choose_multiple(&mut rng, NUM_QUESTIONS_SAMPLED);

    let mut resp = Response::from_json(&json!({
        "hint": sampled_questions,
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

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> WorkerResult<Response> {
    log_request(&req);

    // Optionally, get more helpful error messages written to the console in the case of a panic.
    utils::set_panic_hook();

    let router = Router::new();

    router
        .get("/chat", |req, ctx| {
            let result = handle_chat(req, ctx);
            Ok(result_to_response(result))
        })
        .get_async("/hint", |req, ctx| async move {
            let result = handle_hint(req, ctx).await;
            Ok(result_to_response(result))
        })
        .run(req, env)
        .await
}

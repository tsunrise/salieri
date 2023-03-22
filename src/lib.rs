use futures_util::StreamExt;
use prompt::UserRequest;
use wasm_bindgen_futures::spawn_local;
use worker::{wasm_bindgen::JsValue, *};
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

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    log_request(&req);

    // Optionally, get more helpful error messages written to the console in the case of a panic.
    utils::set_panic_hook();

    // Optionally, use the Router to handle matching endpoints, use ":name" placeholders, or "*name"
    // catch-alls to match on specific patterns. Alternatively, use `Router::with_data(D)` to
    // provide arbitrary data that will be accessible in each route via the `ctx.data()` method.
    let router = Router::new();

    // Add as many routes as your Worker needs! Each route will get a `Request` for handling HTTP
    // functionality and a `RouteContext` which you can use to  and get route parameters and
    // Environment bindings like KV Stores, Durable Objects, Secrets, and Variables.
    router
        .get("/", |req, ctx| {
            let upgrade_header = req.headers().get("Upgrade")?;
            match upgrade_header {
                Some(x) if x == "websocket" => {
                    // sounds good
                }
                _ => {
                    return Response::error("Expected Upgrade: websocket", 426);
                }
            }

            let ws_pair = WebSocketPair::new()?;
            let server = ws_pair.server;
            let client = ws_pair.client;
            server.accept()?;

            let openai_key = ctx.var("OPENAI_API_KEY")?.to_string();
            spawn_local(async move {
                route_chat_to_ws(&openai_key, server)
                    .await
                    .expect("route_chat_to_ws failed");
            });

            Response::from_websocket(client)
        })
        .run(req, env)
        .await
}

pub async fn route_chat_to_ws(openai_key: &str, server: WebSocket) -> worker::Result<()> {
    // wait for the first message from the client
    let first_msg = server.events()?.next().await.unwrap()?;
    let user_request = match first_msg {
        WebsocketEvent::Message(msg) => msg.json::<UserRequest>()?,
        WebsocketEvent::Close(_) => {
            server.close::<String>(None, None)?;
            return Ok(());
        }
    };

    let prompt = toml::from_str::<prompt::Prompt>(include_str!("../prompt.toml")).unwrap();
    let request_to_openai = RequestToOpenAI::new(prompt, user_request.question);
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
        server.send(&StreamItem::Finish(
            stream_parser::FinishReason::Unavailable,
        ))?;
        server.close::<String>(None, None)?;
        return Ok(());
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

    server.close::<String>(None, None)?;

    Ok(())
}

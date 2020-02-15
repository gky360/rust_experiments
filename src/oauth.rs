use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;

use anyhow::Context as _;
use dropbox_sdk::check::{self, EchoArg};
use dropbox_sdk::{ErrorKind, HyperClient, Oauth2AuthorizeUrlBuilder, Oauth2Type};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode, Uri};
use lazy_static::lazy_static;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use tokio::sync::broadcast::{self, Sender};
use url::form_urlencoded;

use crate::{Error, Result};

static DBX_APP_KEY: &str = env!("ACICK_DBX_APP_KEY");
static DBX_APP_SECRET: &str = env!("ACICK_DBX_APP_SECRET");
static REDIRECT_PORT: u16 = 4100;
const REDIRECT_PATH: &str = "/oauth2/callback";

lazy_static! {
    static ref REDIRECT_URI: String =
        format!("http://localhost:{}{}", REDIRECT_PORT, REDIRECT_PATH);
}

fn gen_random_state() -> String {
    static STATE_LEN: usize = 16;
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(STATE_LEN)
        .collect()
}

fn get_params(uri: &Uri) -> HashMap<String, String> {
    uri.query()
        .map(|query_str| {
            form_urlencoded::parse(query_str.as_bytes())
                .into_owned()
                .collect()
        })
        .unwrap_or_else(HashMap::new)
}

fn respond_param_missing(name: &str) -> Response<Body> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(Body::from(format!("Missing parameter: {}", name)))
        .unwrap()
}

fn handle_callback(req: Request<Body>, tx: Sender<String>, state_expected: &str) -> Response<Body> {
    let mut params = get_params(req.uri());
    let code = match params.remove("code") {
        Some(code) => code,
        None => return respond_param_missing("code"),
    };
    let state = match params.remove("state") {
        Some(state) => state,
        None => return respond_param_missing("state"),
    };
    if state != state_expected {
        return Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(Body::from("Invalid parameter: state"))
            .unwrap();
    }
    tx.send(code).unwrap_or(0);

    Response::new(Body::from(
        "Successfully completed authorization. Go back to acick on your terminal.",
    ))
}

async fn respond(
    req: Request<Body>,
    tx: Sender<String>,
    state: String,
) -> std::result::Result<Response<Body>, Infallible> {
    eprintln!("{:?}", req);
    let res = match (req.method(), req.uri().path()) {
        (&Method::GET, REDIRECT_PATH) => handle_callback(req, tx, &state),
        _ => {
            // Return 404 not found response.
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap()
        }
    };
    Ok(res)
}

async fn receive_auth_code() -> Result<String> {
    let (tx, mut rx) = broadcast::channel::<String>(1);

    let state = gen_random_state();

    // start local server
    let addr = SocketAddr::from(([127, 0, 0, 1], REDIRECT_PORT));
    let make_service = make_service_fn(|_conn| {
        let tx = tx.clone();
        let state = state.clone();
        async {
            Ok::<_, Infallible>(service_fn(move |req| {
                respond(req, tx.clone(), state.clone())
            }))
        }
    });
    let server = Server::bind(&addr).serve(make_service);

    // open auth url in browser
    let auth_url = Oauth2AuthorizeUrlBuilder::new(DBX_APP_KEY, Oauth2Type::AuthorizationCode)
        .redirect_uri(&REDIRECT_URI)
        .state(&state)
        .build();
    eprintln!("{}", auth_url);

    // wait for code to arrive and shutdown server
    let graceful = server.with_graceful_shutdown(async {
        let mut rx = tx.subscribe();
        rx.recv().await.unwrap();
        eprintln!("Shutting down server ...");
    });
    graceful.await?;

    Ok(rx.recv().await?)
}

#[tokio::main]
pub async fn run() -> Result<()> {
    let code = receive_auth_code().await.unwrap();
    eprintln!("Received code: {}", code);

    let token = HyperClient::oauth2_token_from_authorization_code(
        DBX_APP_KEY,
        DBX_APP_SECRET,
        &code,
        Some(&REDIRECT_URI),
    )
    .map_err(|err| Error::msg(err.to_string()))?;

    eprintln!("{}", token);

    let client = HyperClient::new(token);
    let is_valid = match check::user(&client, &EchoArg { query: "".into() }) {
        Ok(Ok(_)) => Ok(true),
        Ok(Err(())) => Ok(false),
        Err(dropbox_sdk::Error(ErrorKind::InvalidToken(_), ..)) => Ok(false),
        Err(err) => Err(Error::msg(err.to_string())),
    }
    .context("Could not validate access token")?;

    Ok(())
}

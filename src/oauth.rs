use dropbox_sdk::{Oauth2AuthorizeUrlBuilder, Oauth2Type};
use rand::distributions::Alphanumeric;
use std::convert::Infallible;
use std::net::SocketAddr;

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use rand::{thread_rng, Rng};
use tokio::sync::mpsc::{self, Sender};

use crate::Result;

static DBX_APP_KEY: &str = env!("ACICK_DBX_APP_KEY");
static DBX_APP_SECRET: &str = env!("ACICK_DBX_APP_SECRET");
static PORT: u16 = 4100;

fn gen_random_state() -> String {
    static STATE_LEN: usize = 16;
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(STATE_LEN)
        .collect()
}

async fn respond(
    req: Request<Body>,
    mut tx: Sender<()>,
) -> std::result::Result<Response<Body>, Infallible> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/oauth2/callback") => {
            tx.send(()).await.unwrap_or(());
            Ok(Response::new(Body::from("Hello, world!")))
        }
        _ => {
            // Return 404 not found response.
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap())
        }
    }
}

#[tokio::main]
pub async fn run() -> Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], PORT));
    let (tx, mut rx) = mpsc::channel::<()>(1);
    let make_service = make_service_fn(|_conn| {
        let tx = tx.clone();
        async { Ok::<_, Infallible>(service_fn(move |req| respond(req, tx.clone()))) }
    });
    let server = Server::bind(&addr).serve(make_service);

    let graceful = server.with_graceful_shutdown(async {
        rx.recv().await;
    });

    let auth_url = Oauth2AuthorizeUrlBuilder::new(DBX_APP_KEY, Oauth2Type::AuthorizationCode)
        .redirect_uri(&format!("http://localhost:{}/oauth2/callback", PORT))
        .state(&gen_random_state())
        .build();
    eprintln!("{}", auth_url);

    if let Err(e) = graceful.await {
        eprintln!("server error: {}", e);
    }

    Ok(())
}

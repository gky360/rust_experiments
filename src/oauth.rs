use dropbox_sdk::{Oauth2AuthorizeUrlBuilder, Oauth2Type};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};

use crate::Result;

static DBX_APP_KEY: &str = env!("ACICK_DBX_APP_KEY");
static DBX_APP_SECRET: &str = env!("ACICK_DBX_APP_SECRET");

fn gen_random_state() -> String {
    static STATE_LEN: usize = 16;
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(STATE_LEN)
        .collect()
}

pub fn run() -> Result<()> {
    let auth_url = Oauth2AuthorizeUrlBuilder::new(DBX_APP_KEY, Oauth2Type::AuthorizationCode)
        .redirect_uri("http://localhost:3000/oauth2/callback")
        .state(&gen_random_state())
        .build();
    eprintln!("{}", auth_url);
    Ok(())
}

// Copyright (c) 2019 Dropbox, Inc.

use std::convert::TryFrom as _;
use std::io::{self, Read as _};
use std::str;

use dropbox_sdk::client_trait::{Endpoint, HttpClient, HttpRequestResultRaw, Style};
use dropbox_sdk::ErrorKind;
use hyper::header::*;
// use hyper::header::{
//     Authorization, Bearer, ByteRangeSpec, Connection, ContentLength, ContentType, Range,
// };
use hyper::client::HttpConnector;
use hyper::{Request, Uri};
use hyper_rustls::HttpsConnector;
use serde_json;
use url::form_urlencoded::Serializer as UrlEncoder;

const USER_AGENT: &str = concat!("Dropbox-APIv2-Rust/", env!("CARGO_PKG_VERSION"));

pub struct HyperClient {
    client: hyper::client::Client<HttpsConnector<HttpConnector>>,
    token: String,
}

impl HyperClient {
    pub fn new(token: String) -> HyperClient {
        HyperClient {
            client: Self::http_client(),
            token,
        }
    }

    /// Given an authorization code, request an OAuth2 token from Dropbox API.
    /// Requires the App ID and secret, as well as the redirect URI used in the prior authorize
    /// request, if there was one.
    pub fn oauth2_token_from_authorization_code(
        client_id: &str,
        client_secret: &str,
        authorization_code: &str,
        redirect_uri: Option<&str>,
    ) -> dropbox_sdk::Result<String> {
        let client = Self::http_client();
        let url = Uri::from_static("https://api.dropboxapi.com/oauth2/token");

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, USER_AGENT.parse().unwrap());

        // This endpoint wants parameters using URL-encoding instead of JSON.
        headers.insert(
            CONTENT_TYPE,
            "application/x-www-form-urlencoded".parse().unwrap(),
        );
        let mut params = UrlEncoder::new(String::new());
        params.append_pair("code", authorization_code);
        params.append_pair("grant_type", "authorization_code");
        params.append_pair("client_id", client_id);
        params.append_pair("client_secret", client_secret);
        if let Some(value) = redirect_uri {
            params.append_pair("redirect_uri", value);
        }
        let body = params.finish();
        let req = Request::builder()
            .method("POST")
            .header(USER_AGENT, USER_AGENT.parse().unwrap())
            .header(
                CONTENT_TYPE,
                "application/x-www-form-urlencoded".parse().unwrap(),
            )
            .body(body.into())
            .unwrap();

        match client.request(req) {
            Ok(mut resp) => {
                if !resp.status.is_success() {
                    let (code, status) = (resp.status.as_u16(), resp.status.as_str().to_owned());
                    let mut body = String::new();
                    resp.read_to_string(&mut body)?;
                    // debug!("error body: {}", body);
                    Err(ErrorKind::GeneralHttpError(code, status, body).into())
                } else {
                    let body = serde_json::from_reader(resp)?;
                    // debug!("response: {:?}", body);
                    match body {
                        serde_json::Value::Object(mut map) => match map.remove("access_token") {
                            Some(serde_json::Value::String(token)) => Ok(token),
                            // _ => bail!("no access token in response!"),
                            _ => Err("no access token in response!".into()),
                        },
                        // _ => bail!("invalid response from server"),
                        _ => Err("invalid response from server".into()),
                    }
                }
            }
            Err(e) => {
                // error!("error getting OAuth2 token: {}", e);
                Err(e.into())
            }
        }
    }

    fn http_client() -> hyper::client::Client<HttpsConnector<HttpConnector>> {
        // let tls = hyper_native_tls::NativeTlsClient::new().unwrap();
        // let https_connector = hyper::net::HttpsConnector::new(tls);
        // let pool_connector = hyper::client::pool::Pool::with_connector(
        //     hyper::client::pool::Config { max_idle: 1 },
        //     https_connector,
        // );
        // hyper::client::Client::with_connector(pool_connector)

        let https = HttpsConnector::new();
        hyper::client::Client::builder()
            .max_idle_per_host(1)
            .build(https)
    }
}

impl HttpClient for HyperClient {
    fn request(
        &self,
        endpoint: Endpoint,
        style: Style,
        function: &str,
        params_json: String,
        body: Option<&[u8]>,
        range_start: Option<u64>,
        range_end: Option<u64>,
    ) -> dropbox_sdk::Result<HttpRequestResultRaw> {
        let url =
            Uri::try_from(format!("{}/{}", endpoint.url(), function)).expect("invalid request URL");
        // debug!("request for {:?}", url);

        loop {
            let mut builder = self.client.post(url.clone());

            let mut headers = HeaderMap::new();
            headers.insert(USER_AGENT, USER_AGENT.parse().unwrap());
            headers.insert(
                AUTHORIZATION,
                format!("Bearer {}", self.token).parse().unwrap(),
            );
            headers.insert(CONNECTION, "Keep-Alive".parse().unwrap());

            if let Some(start) = range_start {
                if let Some(end) = range_end {
                    // headers.set(Range::Bytes(vec![ByteRangeSpec::FromTo(start, end)]));
                    headers.insert(RANGE, format!("bytes={}-{}", start, end).parse().unwrap());
                } else {
                    // headers.set(Range::Bytes(vec![ByteRangeSpec::AllFrom(start)]));
                    headers.insert(RANGE, format!("bytes={}-", start).parse().unwrap());
                }
            } else if let Some(end) = range_end {
                // headers.set(Range::Bytes(vec![ByteRangeSpec::Last(end)]));
                headers.insert(RANGE, format!("bytes=-{}", end).parse().unwrap());
            }

            // If the params are totally empt, don't send any arg header or body.
            if !params_json.is_empty() {
                match style {
                    Style::Rpc => {
                        // Send params in the body.
                        // headers.set(ContentType::json());
                        headers.insert(
                            CONTENT_TYPE,
                            "application/json; charset=utf-8".parse().unwrap(),
                        );
                        builder = builder.body(params_json.as_bytes());
                        assert_eq!(None, body);
                    }
                    Style::Upload | Style::Download => {
                        // Send params in a header.
                        headers.insert(
                            HeaderName::from_static("Dropbox-API-Arg"),
                            HeaderValue::from_bytes(params_json.as_bytes()).unwrap(),
                        );
                        if style == Style::Upload {
                            // headers.set(ContentType(hyper::mime::Mime(
                            //     hyper::mime::TopLevel::Application,
                            //     hyper::mime::SubLevel::OctetStream,
                            //     vec![],
                            // )));
                            headers
                                .insert(CONTENT_TYPE, "application/octet-stream".parse().unwrap());
                        }
                        if let Some(body) = body {
                            builder = builder.body(body);
                        }
                    }
                }
            }

            let mut resp = match builder.headers(headers).send() {
                Ok(resp) => resp,
                Err(hyper::error::Error::Io(ref ioerr))
                    if ioerr.kind() == io::ErrorKind::ConnectionAborted =>
                {
                    // debug!("connection closed; retrying...");
                    continue;
                }
                Err(other) => {
                    // error!("request failed: {}", other);
                    return Err(other.into());
                }
            };

            if !resp.status.is_success() {
                // let (code, status) = {
                //     let &hyper::http::RawStatus(ref code, ref status) = resp.status_raw();
                //     use std::ops::Deref;
                //     (*code, status.deref().to_owned())
                // };
                let (code, status) = (resp.as_u16(), resp.as_str().to_owned());
                let mut json = String::new();
                resp.read_to_string(&mut json)?;
                return Err(ErrorKind::GeneralHttpError(code, status, json).into());
            }

            return match style {
                Style::Rpc | Style::Upload => {
                    // Get the response from the body; return no body stream.
                    let mut s = String::new();
                    resp.read_to_string(&mut s)?;
                    Ok(HttpRequestResultRaw {
                        result_json: s,
                        content_length: None,
                        body: None,
                    })
                }
                Style::Download => {
                    // Get the response from a header; return the body stream.
                    let s = match resp.headers.get_raw("Dropbox-API-Result") {
                        Some(values) => String::from_utf8(values[0].clone())?,
                        None => {
                            // bail!(ErrorKind::UnexpectedError(
                            //     "missing Dropbox-API-Result header"
                            // ));
                            return Err(ErrorKind::UnexpectedError(
                                "missing Dropbox-API-Result header",
                            ));
                        }
                    };

                    let len = resp.headers.get(header::CONTENT_LENGTH).map(|h| h.0);

                    Ok(HttpRequestResultRaw {
                        result_json: s,
                        content_length: len,
                        body: Some(Box::new(resp)),
                    })
                }
            };
        }
    }
}

/// Builds a URL that can be given to the user to visit to have Dropbox authorize your app.
#[derive(Debug)]
pub struct Oauth2AuthorizeUrlBuilder<'a> {
    client_id: &'a str,
    response_type: &'a str,
    force_reapprove: bool,
    force_reauthentication: bool,
    disable_signup: bool,
    redirect_uri: Option<&'a str>,
    state: Option<&'a str>,
    require_role: Option<&'a str>,
    locale: Option<&'a str>,
}

/// Which type of OAuth2 flow to use.
#[derive(Debug, Copy, Clone)]
pub enum Oauth2Type {
    /// Authorization yields a temporary authorization code which must be turned into an OAuth2
    /// token by making another call. This can be used without a redirect URI, where the user inputs
    /// the code directly into the program.
    AuthorizationCode,

    /// Authorization directly returns an OAuth2 token. This can only be used with a redirect URI
    /// where the Dropbox server redirects the user's web browser to the program.
    ImplicitGrant,
}

impl Oauth2Type {
    pub fn as_str(self) -> &'static str {
        match self {
            Oauth2Type::AuthorizationCode => "code",
            Oauth2Type::ImplicitGrant => "token",
        }
    }
}

impl<'a> Oauth2AuthorizeUrlBuilder<'a> {
    pub fn new(client_id: &'a str, oauth2_type: Oauth2Type) -> Self {
        Self {
            client_id,
            response_type: oauth2_type.as_str(),
            force_reapprove: false,
            force_reauthentication: false,
            disable_signup: false,
            redirect_uri: None,
            state: None,
            require_role: None,
            locale: None,
        }
    }

    pub fn force_reapprove(mut self, value: bool) -> Self {
        self.force_reapprove = value;
        self
    }

    pub fn force_reauthentication(mut self, value: bool) -> Self {
        self.force_reauthentication = value;
        self
    }

    pub fn disable_signup(mut self, value: bool) -> Self {
        self.disable_signup = value;
        self
    }

    pub fn redirect_uri(mut self, value: &'a str) -> Self {
        self.redirect_uri = Some(value);
        self
    }

    pub fn state(mut self, value: &'a str) -> Self {
        self.state = Some(value);
        self
    }

    pub fn require_role(mut self, value: &'a str) -> Self {
        self.require_role = Some(value);
        self
    }

    pub fn locale(mut self, value: &'a str) -> Self {
        self.locale = Some(value);
        self
    }

    pub fn build(self) -> Uri {
        let mut url = Uri::parse("https://www.dropbox.com/oauth2/authorize").unwrap();
        {
            let mut params = url.query_pairs_mut();
            params.append_pair("response_type", self.response_type);
            params.append_pair("client_id", self.client_id);
            if self.force_reapprove {
                params.append_pair("force_reapprove", "true");
            }
            if self.force_reauthentication {
                params.append_pair("force_reauthentication", "true");
            }
            if self.disable_signup {
                params.append_pair("disable_signup", "true");
            }
            if let Some(value) = self.redirect_uri {
                params.append_pair("redirect_uri", value);
            }
            if let Some(value) = self.state {
                params.append_pair("state", value);
            }
            if let Some(value) = self.require_role {
                params.append_pair("require_role", value);
            }
            if let Some(value) = self.locale {
                params.append_pair("locale", value);
            }
        }
        url
    }
}

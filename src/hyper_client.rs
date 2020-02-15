use dropbox_sdk::client_trait::{Endpoint, HttpClient, HttpRequestResultRaw, Style};
use dropbox_sdk::ErrorKind;
use hyper::body::Body;
use hyper::client::{Client, HttpConnector};
use hyper::header::{AUTHORIZATION, CONNECTION, CONTENT_TYPE, RANGE, USER_AGENT};
use hyper::{Request, Response};
use hyper_rustls::HttpsConnector;
use url::Url;

static DBX_USER_AGENT: &str = concat!("Dropbox-APIv2-Rust/", env!("CARGO_PKG_VERSION"));

type Connector = HttpsConnector<HttpConnector>;

pub struct HyperClient {
    client: Client<Connector>,
    token: String,
}

impl HyperClient {
    #[tokio::main]
    async fn async_request(&self, req: Request<Body>) -> hyper::Result<Response<Body>> {
        self.client.request(req).await
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
        let url = Url::parse(endpoint.url())
            .unwrap()
            .join(function)
            .expect("invalid request URL");

        loop {
            let mut builder = Request::post(url.as_ref());

            // set common headers
            builder = builder
                .header(USER_AGENT, DBX_USER_AGENT)
                .header(AUTHORIZATION, format!("Bearer {}", self.token))
                .header(CONNECTION, "Keep-Alive");

            // set range header
            if let Some(start) = range_start {
                if let Some(end) = range_end {
                    builder = builder.header(RANGE, format!("bytes={}-{}", start, end));
                } else {
                    builder = builder.header(RANGE, format!("bytes={}-", start));
                }
            } else if let Some(end) = range_end {
                builder = builder.header(RANGE, format!("bytes=-{}", end));
            }

            let req: Request<Body> = if params_json.is_empty() {
                // If the params are totally empt, don't send any arg header or body.
                builder.body(Body::empty())
            } else {
                match style {
                    Style::Rpc => {
                        // Send params in the body.
                        assert_eq!(None, body);
                        builder
                            .header(CONTENT_TYPE, "application/json; charset=utf-8")
                            .body(params_json.into())
                    }
                    Style::Upload | Style::Download => {
                        // Send params in a header.
                        builder = builder.header("Dropbox-API-Arg", params_json.as_bytes());
                        if style == Style::Upload {
                            builder =
                                builder.header(CONTENT_TYPE, "application/json; charset=utf-8");
                        }
                        if let Some(body) = body {
                            builder.body(body.to_owned().into())
                        } else {
                            builder.body(Body::empty())
                        }
                    }
                }
            }
            .map_err(|err| dropbox_sdk::Error::from_kind(ErrorKind::Msg(err.to_string())))?;

            let mut resp = match self.async_request(req) {
                Ok(resp) => resp,
                Err(err) if err.is_closed() || err.is_connect() => {
                    continue;
                }
                Err(other) => {
                    return Err(dropbox_sdk::Error::from_kind(ErrorKind::Msg()));
                }
            };
        }
    }
}

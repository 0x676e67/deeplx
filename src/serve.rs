use std::{
    fs::File,
    io::BufReader,
    path::Path,
    sync::{
        atomic::{AtomicU32, Ordering},
        OnceLock,
    },
    time::Duration,
};

use crate::BootArgs;
use actix_web::{
    error,
    http::header::HeaderMap,
    middleware::Logger,
    web::{self, Json},
    App, Error, HttpRequest, HttpResponse, HttpServer, Responder,
};
use anyhow::Result;
use rand::Rng;
use reqwest::{
    header::{self, HeaderValue},
    impersonate::Impersonate,
    StatusCode,
};
use rustls::ServerConfig;
use serde_json::{json, Value};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const KEEP_ALIVE: u8 = 75;
const CONNECTION_TIMEOUT: u8 = 10;
const TIMEOUT: u16 = 360;

const HTML: &'static str = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Login Form</title>
    <style>
        body {
            font-family: Arial, sans-serif;
            margin: 0;
            padding: 0;
            background-color: #f1f1f1;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
        }

        .container {
            width: 300px;
            padding: 20px;
            background-color: #fff;
            border-radius: 5px;
            box-shadow: 0 2px 5px rgba(0, 0, 0, 0.1);
        }

        h2 {
            text-align: center;
            margin-bottom: 20px;
        }

        input[type="email"],
        input[type="password"],
        input[type="submit"] {
            width: 100%;
            padding: 10px;
            margin-bottom: 15px;
            border: 1px solid #ccc;
            border-radius: 3px;
            box-sizing: border-box;
        }

        input[type="submit"] {
            background-color: #0a180b;
            color: white;
            cursor: pointer;
        }

        input[type="submit"]:hover {
            background-color: #454545;
        }
    </style>
</head>
<body>
    <div class="container">
        <h2>Contribute Deepl Pro</h2>
        <form method="post">
            <label for="email">Email:</label>
            <input type="email" id="email" name="email" required>
            <label for="password">Password:</label>
            <input type="password" id="password" name="password" required>
            <input type="submit" value="Submit">
        </form>
    </div>
</body>
</html>
"#;

// This struct represents state
struct AppState {
    api_key: Option<String>,
}

pub struct Serve(pub BootArgs);

impl Serve {
    #[actix_web::main]
    pub async fn run(&self) -> Result<()> {
        // Init tracing
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "RUST_LOG=info".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();

        // Init client pool
        let client = Client::new(self.0.proxies.clone())?;
        let _ = CLIENT.set(client);

        // Init rearrange db
        let _ = db::rearrange().map_err(|err| {
            tracing::error!("Failed to rearrange dl_session: {err}");
        });

        // Init dl_session
        self.0.dl_session.as_ref().map(|dl_session| {
            let _ = db::insert_dl_session(dl_session).map_err(|err| {
                tracing::error!("Failed to insert dl_session: {err}");
            });
        });

        let api_key = self.0.api_key.clone();

        api_key.as_ref().map(|_| {
            tracing::info!("API key is required");
        });

        tracing::info!("Starting server at {}", self.0.bind);

        // Start server
        let builder = HttpServer::new(move || {
            App::new()
                .wrap(
                    actix_cors::Cors::default()
                        .supports_credentials()
                        .allow_any_origin()
                        .allow_any_header()
                        .allow_any_method()
                        .max_age(3600),
                )
                .wrap(Logger::default())
                .app_data(web::Data::new(AppState {
                    api_key: api_key.clone(),
                }))
                .route("/", web::get().to(manual_hello))
                .route("/pool", web::get().to(get_pool))
                .route("/pool", web::post().to(post_pool))
                .route("/translate", web::post().to(translate))
        })
        .client_request_timeout(Duration::from_secs(TIMEOUT as u64))
        .client_disconnect_timeout(Duration::from_secs(CONNECTION_TIMEOUT as u64))
        .keep_alive(Duration::from_secs(KEEP_ALIVE as u64));

        match (&self.0.tls_cert, &self.0.tls_key) {
            (Some(cert), Some(key)) => {
                let tls_config = Self::load_rustls_config(cert, key).await?;
                builder
                    .bind_rustls_0_22(self.0.bind, tls_config)?
                    .run()
                    .await?
            }
            _ => builder.bind(self.0.bind)?.run().await?,
        }

        Ok(())
    }

    async fn load_rustls_config<P: AsRef<Path>>(tls_cert: P, tls_key: P) -> Result<ServerConfig> {
        use rustls_pemfile::{certs, private_key};

        // load TLS key/cert files
        let cert_file = &mut BufReader::new(File::open(tls_cert)?);
        let key_file = &mut BufReader::new(File::open(tls_key)?);

        // load TLS certs and key
        // to create a self-signed temporary cert for testing:
        // `openssl req -x509 -newkey rsa:4096 -nodes -keyout key.pem -out cert.pem -days 365 -subj '/CN=localhost'`
        let tls_certs = certs(cert_file).collect::<Result<Vec<_>, _>>().unwrap();

        let keys = private_key(key_file)?
            .ok_or_else(|| anyhow::anyhow!("Could not locate EC/PKCS8/RSA private keys."))?;

        // set up TLS config options
        let tls_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(tls_certs, keys)
            .map_err(|e| anyhow::anyhow!("Could not set up TLS config: {}", e))?;

        Ok(tls_config)
    }
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("DeepL Free API, Developed by gngpp. Go to /translate with POST. http://github.com/gngpp/deeplx")
}

async fn get_pool() -> impl Responder {
    HttpResponse::Ok().content_type("text/html").body(HTML)
}

#[derive(Deserialize)]
struct LoginForm {
    email: String,
    password: String,
}

async fn post_pool(form: web::Form<LoginForm>) -> actix_web::Result<impl Responder> {
    let dl_session = auth::login(&form.email, &form.password).await?;

    // If dl_session is empty, return an error
    if dl_session.is_empty() {
        return Ok(HttpResponse::Ok().json(json!({
            "code": StatusCode::BAD_REQUEST.as_u16(),
            "message": "Failed to get dl_session",
        })));
    }

    // Insert dl_session
    db::insert_dl_session(&dl_session).map_err(error::ErrorInternalServerError)?;

    // Count dl_session
    let total = db::count_dl_session().map_err(error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(json!({
        "code": StatusCode::OK.as_u16(),
        "message": "Successfully",
        "total": total,
    })))
}

async fn translate(
    req: HttpRequest,
    bdoy: Json<PayloadFree>,
    state: web::Data<AppState>,
) -> actix_web::Result<impl Responder> {
    // Verify the API key
    verify_api_key(req.headers(), &state).await?;
    let id = get_random_number() + 1;
    let number_alternative = 0.clamp(0, 3);

    let post_data = json!({
        "jsonrpc": "2.0",
        "method": "LMT_handle_texts",
        "id": id,
        "params": {
            "texts": [{
                "text": bdoy.text,
                "requestAlternatives": number_alternative
            }],
            "splitting": "newlines",
            "lang": {
                "source_lang_user_selected": bdoy.source_lang.to_uppercase(),
                "target_lang": bdoy.target_lang.to_uppercase(),
            },
            "timestamp": get_timestamp(get_i_count(&bdoy.text))?,
            "commonJobParams": {
                "wasSpoken": false,
                "transcribe_as": ""
            }
        },
    });

    let mut body = serde_json::to_string(&post_data)?;

    if (id + 5) % 29 == 0 || (id + 3) % 13 == 0 {
        body = body.replace("\"method\":\"", "\"method\" : \"");
    } else {
        body = body.replace("\"method\":\"", "\"method\": \"");
    }

    let dl_session = db::round_robin_dl_session().map_err(error::ErrorExpectationFailed)?;

    let resp = get_client()?
        .post("https://api.deepl.com/jsonrpc")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::COOKIE, format!("dl_session={dl_session};",))
        .body(body)
        .send()
        .await
        .map_err(error::ErrorBadGateway)?;

    match resp.status() {
        StatusCode::TOO_MANY_REQUESTS => {
            return Err(error::ErrorTooManyRequests(
                "Too many requests, your IP has been blocked by DeepL temporarily, please don't request it frequently in a short time."
            ));
        }
        // If the dl_session is invalid, remove it from the database
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            if let Some(err) = db::remove_dl_session(dl_session).err() {
                tracing::error!("Failed to remove dl_session: {err}");
            }
            return Err(error::ErrorFailedDependency(
                "Failed dependency, please check your request and try again.",
            ));
        }
        _ => {}
    }

    let body = resp
        .error_for_status()
        .map_err(error::ErrorInternalServerError)?
        .json::<Value>()
        .await
        .map_err(error::ErrorBadGateway)?;

    let mut alternatives = Vec::new();

    let texts_zero = body
        .get("result")
        .map(|v| v.get("texts").map(|arr| arr.as_array()).flatten())
        .map(|v| v.map(|v| v.get(0)).flatten())
        .flatten();

    texts_zero.map(|v| v.as_array()).flatten().map(|arr| {
        for value in arr {
            value
                .get("alternatives")
                .map(|v| v.as_array())
                .flatten()
                .map(|arr| {
                    for value in arr {
                        value
                            .get("text")
                            .map(|v| v.as_str())
                            .flatten()
                            .map(|s| alternatives.push(s));
                    }
                });
        }
    });

    let data = texts_zero
        .map(|v| v.get("text"))
        .flatten()
        .map(|v| v.as_str())
        .flatten()
        .unwrap_or_default();

    let response = json!({
        "code": StatusCode::OK.as_u16(),
        "id": id,
        "data": data,
        "alternatives": alternatives,
        "source_lang": bdoy.source_lang,
        "target_lang": bdoy.target_lang,
        "method": "Free",
    });

    // json rpc translate
    Ok(HttpResponse::Ok().json(response))
}

/// Verify the API key
async fn verify_api_key(headers: &HeaderMap, state: &web::Data<AppState>) -> Result<(), Error> {
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim_start_matches("Bearer "));

    // Check if the API key is valid
    if let (Some(auth), Some(ref api_key)) = (authorization, &state.api_key) {
        if auth.ne(api_key) {
            return Err(actix_web::error::ErrorUnauthorized(
                "You are not authorized",
            ));
        }
    }

    Ok(())
}

use std::time::{SystemTime, UNIX_EPOCH};

/// Get i count
pub fn get_i_count(translate_text: &str) -> usize {
    translate_text.matches('i').count()
}

/// Get random number
pub fn get_random_number() -> u64 {
    let mut rng = rand::thread_rng();
    let num: u64 = rng.gen_range(0..99999) + 8300000;
    num * 1000
}

/// Get timestamp
pub fn get_timestamp(i_count: usize) -> actix_web::Result<u128> {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .map_err(error::ErrorInternalServerError)?;
    let in_ms = since_the_epoch.as_millis();

    if i_count == 0 {
        Ok(in_ms)
    } else {
        let i_count = i_count as u128;
        Ok(in_ms - (in_ms % i_count) + i_count)
    }
}

static CLIENT: OnceLock<Client> = OnceLock::new();

fn get_client() -> actix_web::Result<reqwest::Client> {
    Ok(CLIENT
        .get()
        .ok_or_else(|| error::ErrorInternalServerError("Failed to get the client"))?
        .next())
}

struct Client(AtomicU32, Vec<reqwest::Client>);

impl Client {
    fn new(proxies: Option<Vec<String>>) -> Result<Self> {
        let mut clients = Vec::new();

        if let Some(proxies) = proxies {
            for proxy in proxies {
                let client = Self::build_client(Some(proxy))?;
                clients.push(client);
            }
        } else {
            let client = Self::build_client(None)?;
            clients.push(client);
        }
        Ok(Self(AtomicU32::new(0), clients))
    }

    fn build_client(proxy: Option<String>) -> Result<reqwest::Client> {
        let mut builder = reqwest::Client::builder()
            .impersonate(Impersonate::Edge122)
            .default_headers((|| {
                let mut headers = header::HeaderMap::new();
                headers.insert(header::USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36"));
                headers.insert(header::ACCEPT, HeaderValue::from_static("*/*"));
                headers.insert(
                    header::ACCEPT_LANGUAGE,
                    HeaderValue::from_static("en-US,en;q=0.9"),
                );
                headers.insert(
                    header::ACCEPT_ENCODING,
                    HeaderValue::from_static("gzip, deflate, br"),
                );
                headers.insert(
                    header::ORIGIN,
                    HeaderValue::from_static("https://www.deepl.com"),
                );
                headers.insert(
                    header::REFERER,
                    HeaderValue::from_static("https://www.deepl.com/"),
                );
                headers
            })())
            .timeout(Duration::from_secs(TIMEOUT as u64))
            .connect_timeout(Duration::from_secs(CONNECTION_TIMEOUT as u64))
            .tcp_keepalive(Duration::from_secs(KEEP_ALIVE as u64))
            .redirect(reqwest::redirect::Policy::none());

        if let Some(proxy) = proxy {
            builder = builder.proxy(reqwest::Proxy::all(&proxy)?);
        }

        builder.build().map_err(Into::into)
    }
    // Round-robin client
    fn next(&self) -> reqwest::Client {
        let pool = &self.1;
        if self.1.len() == 1 {
            self.1[0].clone()
        } else {
            let len = self.1.len() as u32;
            let mut old = self.0.load(Ordering::Relaxed);
            let mut new;
            loop {
                new = (old + 1) % len;
                match self
                    .0
                    .compare_exchange_weak(old, new, Ordering::SeqCst, Ordering::Relaxed)
                {
                    Ok(_) => break,
                    Err(x) => old = x,
                }
            }
            pool[new as usize].clone()
        }
    }
}

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PayloadFree {
    pub text: String,
    #[serde(default = "default_source_text")]
    pub source_lang: String,
    #[serde(default = "default_target_lang")]
    pub target_lang: String,
}

fn default_source_text() -> String {
    String::from("AUTO")
}

fn default_target_lang() -> String {
    String::from("ZH")
}

mod db {
    use anyhow::Result;
    use redb::{Database, ReadableTable, TableDefinition};
    use std::{
        path::PathBuf,
        sync::{
            atomic::{AtomicU32, Ordering},
            OnceLock,
        },
    };
    const TABLE: TableDefinition<u32, &str> = TableDefinition::new("dl_session");
    static DB: OnceLock<(AtomicU32, Database)> = OnceLock::new();

    fn get_db() -> &'static (AtomicU32, Database) {
        DB.get_or_init(|| {
            let binding = std::env::current_exe().expect("Failed to get current directory");
            let dir = binding.parent().expect("Failed to get parent directory");
            (
                AtomicU32::new(0),
                Database::create(PathBuf::from(dir).join("deepl.db"))
                    .expect("Failed to create database"),
            )
        })
    }

    /// Count dl_session
    pub fn count_dl_session() -> Result<u32> {
        let (_, db) = get_db();
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(TABLE)?;
        Ok(table.len()? as u32)
    }

    /// Round-robin dl_session
    pub fn round_robin_dl_session() -> Result<String> {
        let (index, db) = get_db();
        let read_txn = db.begin_read()?;
        let table = read_txn.open_table(TABLE)?;
        if table.is_empty()? {
            return Err(anyhow::anyhow!("Failed to get dl_session"));
        };

        // round-robin
        let len = table.len()? as u32;

        let new = if len == 1 {
            0
        } else {
            let mut old = index.load(Ordering::Relaxed);
            let mut new;
            loop {
                new = (old + 1) % len;
                match index.compare_exchange_weak(old, new, Ordering::SeqCst, Ordering::Relaxed) {
                    Ok(_) => break,
                    Err(x) => old = x,
                }
            }
            new
        };

        // The ID of the dl_session is new + 1
        let dl_session = table
            .get(new + 1)?
            .ok_or_else(|| anyhow::anyhow!("Failed to get dl_session"))?;
        Ok(dl_session.value().to_owned())
    }

    fn get_dl_session_index(dl_session: &str) -> Result<(u32, Option<(u32, String)>)> {
        let (_, db) = get_db();
        let read_txn = db.begin_read()?;
        let read_table = read_txn.open_table(TABLE)?;
        for value in read_table.iter()? {
            let value = value?;
            if value.1.value().eq(dl_session) {
                let last = read_table
                    .last()?
                    .map(|(k, v)| (k.value(), v.value().to_owned()));
                return Ok((value.0.value(), last));
            }
        }
        Err(anyhow::anyhow!("Failed to get dl_session"))
    }

    // Insert dl_session
    pub fn insert_dl_session(dl_session: &str) -> Result<()> {
        let (_, db) = get_db();
        let write_txn = db.begin_write()?;
        // Check dl_session exists, if not, insert
        {
            let table = write_txn.open_table(TABLE)?;
            let exists = table
                .iter()?
                .find(|v| v.as_ref().is_ok_and(|v| v.1.value().eq(dl_session)));

            if exists.map(|v| v.ok()).flatten().is_some() {
                return Ok(());
            }
        }
        {
            let mut table = write_txn.open_table(TABLE)?;
            let len = table.len()?;
            table.insert((len + 1) as u32, dl_session)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    // Remove dl_session
    pub fn remove_dl_session(dl_session: String) -> Result<()> {
        let (_, db) = get_db();

        let (key, last_key) = get_dl_session_index(&dl_session)?;

        match last_key {
            None => return Ok(()),
            Some((last_key, last_value)) => {
                // write transaction
                let write_txn = db.begin_write()?;
                {
                    let mut write_table = write_txn.open_table(TABLE)?;

                    // reinsert last key
                    write_table.insert(key, last_value.as_str())?;

                    // remove last key
                    write_table.remove(last_key)?;
                }

                write_txn.commit()?;
            }
        }

        Ok(())
    }

    pub fn rearrange() -> Result<()> {
        let (_, db) = get_db();
        let mut new_table = Vec::new();
        let write_txn = db.begin_write()?;
        let read_txn = db.begin_read()?;
        let read_table = read_txn.open_table(TABLE)?;

        {
            let mut write_table = write_txn.open_table(TABLE)?;
            // reduce dl_session
            for value in read_table.iter()? {
                let (key, value) = value?;
                write_table.remove(key.value())?;
                new_table.push(value.value().to_owned());
            }

            // reinsert dl_session
            for (i, value) in new_table.iter().enumerate() {
                write_table.insert((i + 1) as u32, value.as_str())?;
            }
        }

        write_txn.commit()?;
        Ok(())
    }
}

mod auth {
    use actix_web::error::{ErrorBadGateway, ErrorBadRequest};
    use reqwest::header;
    use serde_json::json;

    use super::get_client;

    pub async fn login(email: &str, password: &str) -> actix_web::Result<String> {
        let client = get_client()?;

        let resp = client
            .get("https://clearance.deepl.com/token")
            .send()
            .await
            .map_err(ErrorBadGateway)?;

        let dl_clearance = resp
            .cookies()
            .find(|c| c.name() == "dl_clearance")
            .map(|c| c.value().to_owned())
            .ok_or_else(|| ErrorBadGateway("Failed login to get dl_clearance".to_string()))?;

        let body = json!(
            {
                "id": 53080001,
                "jsonrpc": "2.0",
                "method": "login",
                "params": {
                    "clearanceInfo": {
                        "status": 200,
                        "duration": 819
                    },
                    "referrer": "https://www.deepl.com/es/login/",
                    "email": email,
                    "password": password,
                    "version": "44",
                    "loginDomain": "default"
                }
            }
        );

        let resp = client
            .post("https://w.deepl.com/account?request_type=jsonrpc&il=es&method=login")
            .header(header::COOKIE, format!("dl_clearance={dl_clearance};"))
            .json(&body)
            .send()
            .await
            .map_err(ErrorBadGateway)?;

        // Extract dl_session
        let dl_session = resp
            .cookies()
            .find(|c| c.name() == "dl_session")
            .map(|c| c.value().to_owned())
            .ok_or_else(|| ErrorBadGateway("Failed to get dl_session".to_string()))?;

        if dl_session.contains(".") {
            return Err(ErrorBadRequest("Your account is not a Deepl Pro account, please use a Deepl Pro account to contribute.".to_string()));
        }

        Ok(dl_session)
    }
}

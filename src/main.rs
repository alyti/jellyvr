//! Run with
//!
//! ```not_rust
//! nix run .#watch
//! ```

use axum::{
    async_trait,
    body::{Body, Bytes},
    extract::{
        FromRef, FromRequest, FromRequestParts, Host, MatchedPath, Path, Request as ExtractRequest,
        State,
    },
    http::{request::Parts, HeaderMap, Request, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use axum_embed::ServeEmbed;
use axum_extra::extract::cookie::{Cookie, CookieJar};
use http_body_util::BodyExt;
use listenfd::ListenFd;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use surrealdb::{
    engine::local::{Db, RocksDb},
    Surreal,
};
use tokio::net::TcpListener;
use tower_http::{classify::ServerErrorsFailureClass, trace::TraceLayer};
use tracing::{info_span, Span};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod heresphere;
mod index;
mod jellyfin;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // axum logs rejections from built-in extractors with the `axum::rejection`
                // target, at `TRACE` level. `axum::rejection=trace` enables showing those events
                "jellyvr=debug,tower_http=debug,axum::rejection=trace".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Create database connection
    let db = Surreal::new::<RocksDb>(".jellyvr-db").await?;
    db.use_ns("jellyvr").use_db("jellyvr").await?;

    // Sorry it's hardcoded for now
    let config = AppConfig {
        jellyfin_base_url: "https://jellyfin.alyti.dev".to_string(),
        cache_lifetime: Duration::from_secs(60 * 2),
        prefered_subtitles_language: Some("eng".to_string()),
        watchtime_tracking: false,
    };

    let app_state = AppState {
        jellyfin: JellyfinState {
            client: jellyfin::JellyfinClient::new(jellyfin::JellyfinConfig::new(
                config.jellyfin_base_url.clone(),
            )),
        },
        db: db.clone(),
        config,
    };

    let heresphere_api = Router::new()
        .route("/", post(heresphere_libraries))
        .route("/scan", post(heresphere_scan))
        .route("/:id", post(heresphere_video))
        .route("/events/:sid/:vid", post(heresphere_event));

    let app = Router::new()
        .route("/", get(root))
        .nest("/heresphere", heresphere_api)
        .nest_service("/assets", ServeEmbed::<Assets>::new())
        // .route("/heresphere/scan", post(heresphere_scan))
        .with_state(app_state.clone())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<_>| {
                    let matched_path = request
                        .extensions()
                        .get::<MatchedPath>()
                        .map(MatchedPath::as_str);

                    info_span!(
                        "http_request",
                        method = ?request.method(),
                        matched_path,
                        some_other_field = tracing::field::Empty,
                    )
                })
                .on_request(|_request: &Request<_>, _span: &Span| {
                    // You can use `_span.record("some_other_field", value)` in one of these
                    // closures to attach a value to the initially empty field in the info_span
                    // created above.
                })
                .on_response(|_response: &Response, _latency: Duration, _span: &Span| {})
                .on_body_chunk(|_chunk: &Bytes, _latency: Duration, _span: &Span| {})
                .on_eos(
                    |_trailers: Option<&HeaderMap>, _stream_duration: Duration, _span: &Span| {},
                )
                .on_failure(
                    |_error: ServerErrorsFailureClass, _latency: Duration, _span: &Span| {},
                ),
        )
        .fallback(handler_404);

    let mut listenfd = ListenFd::from_env();
    let listener = match listenfd.take_tcp_listener(0)? {
        // if we are given a tcp listener on listen fd 0, we use that one
        Some(listener) => {
            listener.set_nonblocking(true)?;
            TcpListener::from_std(listener)?
        }
        // otherwise fall back to local listening
        None => TcpListener::bind("0.0.0.0:3000").await?,
    };

    // start a background task that updates the progress of the current playback
    tokio::spawn(async move {
        let app_state_local = app_state.clone();
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            if let Err(e) = progress_update_routine(&app_state_local).await {
                tracing::error!(error = ?e, "Failed to update progress");
            }
        }
    });

    // run it
    tracing::debug!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(RustEmbed, Clone)]
#[folder = "assets/"]
struct Assets;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AppConfig {
    jellyfin_base_url: String,
    cache_lifetime: Duration,
    prefered_subtitles_language: Option<String>,
    watchtime_tracking: bool,
}

// the application state
#[derive(Clone)]
struct AppState {
    jellyfin: JellyfinState,
    db: Surreal<Db>,
    config: AppConfig,
}

// jellyfin specific state
#[derive(Clone)]
struct JellyfinState {
    client: jellyfin::JellyfinClient,
}

// support converting an `AppState` in an `ApiState`
impl FromRef<AppState> for JellyfinState {
    fn from_ref(app_state: &AppState) -> JellyfinState {
        app_state.jellyfin.clone()
    }
}

// Make our own error that wraps `anyhow::Error`.
struct AppError(eyre::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response<Body> {
        (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()).into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<eyre::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

async fn handler_404(request: ExtractRequest) -> Result<impl IntoResponse, Response> {
    let (parts, body) = request.into_parts();
    let bytes = body
        .collect()
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response())?
        .to_bytes();

    tracing::debug!(
        method = ?parts.method,
        path = ?parts.uri.path(),
        headers = ?parts.headers,
        body = ?bytes,
        "Unknown route or method"
    );
    Ok((StatusCode::NOT_FOUND, "nothing to see here"))
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct User {
    user_id: String,
    token: String,
    username: String,
    jellyvr_password: String,
    last_known_playback: Option<Playback>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Playback {
    play_session_id: String,
    video_id: String,
    duration: i64,
    position_estimate: i64,
    speed: f64,
    started_at: chrono::DateTime<chrono::Utc>,
    last_update: chrono::DateTime<chrono::Utc>,
    is_paused: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct QuickConnect {
    secret: String,
    code: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum Session {
    QuickConnect(QuickConnect),
    User(User),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SessionState {
    id: Option<surrealdb::sql::Thing>,
    session: Session,
}

impl AppState {
    async fn new_session(&self) -> eyre::Result<SessionState> {
        let new_qc = self.jellyfin.client.new_quick_connect().await?;
        let session: Vec<SessionState> = self
            .db
            .create("session")
            .content(&SessionState {
                id: None,
                session: Session::QuickConnect(QuickConnect {
                    secret: new_qc.secret,
                    code: new_qc.code,
                }),
            })
            .await?;
        tracing::info!("Created new session: {:?}", session);
        Ok(session.first().expect("No session created").clone())
    }

    async fn update_session(&self, session: SessionState) -> eyre::Result<SessionState> {
        let session: Option<SessionState> = self.db.update(session.id.as_ref().unwrap()).content(&session).await?;
        match session {
            Some(state) => Ok(state),
            None => Err(eyre::eyre!("Failed to update session")),
        }
    }

    async fn handle_session(&self, session: Option<String>) -> eyre::Result<SessionState> {
        let existing_state = match session {
            Some(cookie) => {
                let session: Option<SessionState> = self.db.select(("session", cookie)).await?;
                match session {
                    Some(state) => state.clone(),
                    None => self.new_session().await?,
                }
            }
            None => self.new_session().await?,
        };

        match &existing_state.session {
            Session::QuickConnect(QuickConnect { secret, code }) => {
                let qc = self.jellyfin.client.resume_quick_connect(&secret, &code);
                let resp = qc.poll().await?;
                if resp {
                    let resp = qc.auth().await?;
                    let jellyvr_short_password = gen_short_password(6);
                    let session: Vec<SessionState> = self
                        .db
                        .update("session")
                        .content(&SessionState {
                            id: existing_state.id,
                            session: Session::User(User {
                                user_id: resp.id,
                                token: resp.token,
                                username: resp.username,
                                jellyvr_password: jellyvr_short_password,
                                last_known_playback: None,
                            }),
                        })
                        .await?;
                    Ok(session.first().expect("No session created").clone())
                } else {
                    Ok(existing_state)
                }
            }
            Session::User(User { .. }) => Ok(existing_state),
        }
    }

    async fn get_session_from_heresphere_request(
        &self,
        req: &heresphere::Request,
    ) -> eyre::Result<SessionState>  {
        // query db for session using username&password from request
        let session: Option<SessionState> = self.db.query("SELECT * FROM session WHERE session.User.username = $username AND session.User.jellyvr_password = $password LIMIT 1").bind(req).await?.take(0)?;
        match session {
            Some(state) => Ok(state),
            None => Err(eyre::eyre!("No session found for request")),
        }
    }

    async fn get_session_from_heresphere_event(
        &self,
        sid: &str,
    ) -> eyre::Result<SessionState> {
        let session: Option<SessionState> = self
            .db
            .query("SELECT * FROM type::thing('session', $sid)")
            .bind(("sid", sid))
            .await?
            .take(0)?;
        match session {
            Some(state) => Ok(state),
            None => Err(eyre::eyre!("No session found for request")),
        }
    }
}

fn gen_short_password(arg: i32) -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut password = String::new();
    for _ in 0..arg {
        password.push(rng.gen_range('a'..='z'));
    }
    password
}

async fn root(State(app): State<AppState>, jar: CookieJar) -> Result<impl IntoResponse, AppError> {
    let state = app
        .handle_session(jar.get("jellyvr_session").map(|c| c.value().to_string()))
        .await?;
    let d = serde_json::to_string_pretty(&state).map_err(|err| AppError(err.into()))?;
    tracing::debug!(
        state = ?d,
        "Resolved state"
    );
    // TODO: Rewrite this to something nicer maybe...
    Ok((jar.add(Cookie::new("jellyvr_session", state.id.unwrap().id.to_string())), Html(format!(r#"
<!DOCTYPE html>
<html>
    <head>
        <meta http-equiv="refresh" content="5" />
    </head>
    <body>
        {}
    </body>
</html>
"#, match state.session {
        Session::QuickConnect(QuickConnect{ code, ..}) => format!("<h1>Code: {}</h1>", code),
        Session::User(User{username, jellyvr_password, ..}) => format!("<h1>User: {}</h1></br><h1>Pass: {}</h1></br><h2><a href=\"/heresphere\">Heresphere!</a></h2>", username, jellyvr_password),
    }))))
}

/// Extractor for a Heresphere session
struct HeresphereSession {
    request: Json<heresphere::Request>,
    session_state: SessionState,
    user: User,
}

#[async_trait]
impl FromRequest<AppState> for HeresphereSession {
    type Rejection = Response;

    async fn from_request(req: Request<Body>, state: &AppState) -> Result<Self, Self::Rejection> {
        let body = Json::<heresphere::Request>::from_request(req, state)
            .await
            .map_err(IntoResponse::into_response)?;
        let session =
            match state.get_session_from_heresphere_request(&body).await {
                Ok(session) => session,
                Err(err) => {
                    tracing::warn!(
                        error = ?err,
                        "Failed to resolve state"
                    );
                    return Err((
                [
                    (heresphere::MAGIC_HEADER, "1"),
                    ("Content-Type", "application/json"),
                ],
                format!(
                    r#"{{"access": -1, "library": [{{"name": "Login pls", "list": []}},]}}"#,
                )).into_response());
                }
            };

        let user =
            match &session {
                SessionState {
                    session: Session::User(user),
                    ..
                } => user.clone(),
                _ => return Err((
                    [
                        (heresphere::MAGIC_HEADER, "1"),
                        ("Content-Type", "application/json"),
                    ],
                    format!(
                        r#"{{"access": -1, "library": [{{"name": "Login pls", "list": []}},]}}"#,
                    ),
                )
                    .into_response()),
            };

        Ok(Self {
            request: body.clone(),
            session_state: session,
            user: user,
        })
    }
}

struct ProtoHost(String);

#[async_trait]
impl<S> FromRequestParts<S> for ProtoHost
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let host = Host::from_request_parts(parts, state)
            .await
            .map(|host| Self(host.0))
            .map_err(IntoResponse::into_response);

        // If the service is running behind a reverse proxy we need to
        // use the `x-forwarded-proto` header to get the real proto of the request
        // so our references to the service are correct for the client
        let scheme = match parts.headers.get("x-forwarded-proto") {
            Some(scheme) => scheme.to_str().map_err(|_| {
                (StatusCode::BAD_REQUEST, "Invalid x-forwarded-proto header").into_response()
            })?,
            None => "http",
        };

        Ok(Self(format!("{}://{}", scheme, host?.0)))
    }
}

async fn heresphere_libraries(
    State(app): State<AppState>,
    ProtoHost(host): ProtoHost,
    HeresphereSession { user, .. }: HeresphereSession,
) -> Result<impl IntoResponse, AppError> {
    let cache =
        index::HeresphereIndex::prime_data_maybe(&app, &host, &user.user_id, &user.token).await?;
    Ok((
        [
            (heresphere::MAGIC_HEADER, "1"),
            ("Content-Type", "application/json"),
        ],
        format!(
            r#"{{"access": 1, "library": {}}}"#,
            serde_json::to_string_pretty(&cache.libraries).map_err(|err| AppError(err.into()))?,
        ),
    ))
}

async fn heresphere_scan(
    State(app): State<AppState>,
    ProtoHost(host): ProtoHost,
    HeresphereSession { user, .. }: HeresphereSession,
) -> Result<impl IntoResponse, AppError> {
    let cache =
        index::HeresphereIndex::prime_data_maybe(&app, &host, &user.user_id, &user.token).await?;
    Ok((
        [
            (heresphere::MAGIC_HEADER, "1"),
            ("Content-Type", "application/json"),
        ],
        serde_json::to_string_pretty(&cache.scan).map_err(|err| AppError(err.into()))?,
    ))
}

async fn heresphere_video(
    State(app): State<AppState>,
    ProtoHost(host): ProtoHost,
    Path(vid): Path<String>,
    HeresphereSession {
        user,
        request,
        session_state,
    }: HeresphereSession,
) -> Result<impl IntoResponse, AppError> {
    let mut video = index::HeresphereIndex::get_video(&app.db, &user.user_id, &vid).await?; //.ok_or(AppError(eyre::eyre!("No video found")))?;
    if let Some(true) = request.needs_media_source {
        let jellyfin_user = app.jellyfin.client.resume_user(&user.user_id, &user.token);
        let playback_info = jellyfin_user.playback_info(&vid).await?;
        let play_session = playback_info
            .play_session_id
            .ok_or(AppError(eyre::eyre!("Failed to get play session ID")))?;
        let new_media_source = if let Some(transcoding_url) = playback_info
            .media_sources
            .first()
            .and_then(|source| source.transcoding_url.as_ref())
        {
            transcoding_url.clone()
        } else {
            format!(
                "/Videos/{}/master.m3u8?playSessionId={}&api_key={}&mediaSourceId={}",
                vid,
                play_session,
                user.token,
                match playback_info.media_sources.first() {
                    Some(source) => source.id.as_ref().unwrap_or(&vid),
                    None => &vid,
                }
            )
        };
        if let Some(old_pid) = user.last_known_playback {
            if old_pid.play_session_id != play_session {
                tracing::debug!(
                    "Updating play session ID from {} to {} (TODO: send jellyfin stopped)",
                    old_pid.play_session_id,
                    play_session
                );
            }
        }
        video.data.event_server = Some(format!(
            "{}/heresphere/events/{}/{}",
            host,
            session_state
                .id.clone()
                .expect("Failed to get session ID")
                .id
                .to_raw(),
            vid
        ));
        video.data.media[0].sources[0].url =
            format!("{}{}", app.config.jellyfin_base_url, new_media_source);
        let new_session_state = SessionState {
            id: session_state.id,
            session: Session::User(User {
                last_known_playback: Some(Playback {
                    play_session_id: play_session.clone(),
                    video_id: vid.clone(),
                    duration: (video.data.duration * 10000.0) as i64,
                    position_estimate: 0,
                    speed: 1.0,
                    started_at: chrono::Utc::now(),
                    last_update: chrono::Utc::now(),
                    is_paused: true,
                }),
                ..user
            }),
        };
        app.update_session(new_session_state).await?;
        jellyfin_user.playback_start(&vid, &play_session).await?;
    }

    tracing::debug!(video = ?video, "Found video");
    Ok((
        [
            (heresphere::MAGIC_HEADER, "1"),
            ("Content-Type", "application/json"),
        ],
        serde_json::to_string_pretty(&video.data).map_err(|err| AppError(err.into()))?,
    ))
}

async fn heresphere_event(
    State(app): State<AppState>,
    ProtoHost(host): ProtoHost,
    Path((sid, vid)): Path<(String, String)>,
    Json(event): Json<heresphere::Event>,
) -> Result<(), AppError> {
    tracing::debug!(event = ?event, sid = ?sid, "Received event");
    match app.get_session_from_heresphere_event(&sid).await {
        Ok(SessionState {
            session: Session::User(user),
            id,
        }) => {
            tracing::debug!(user = ?user, "Got user session");
            match event.event {
                heresphere::EventType::Open => {
                    // NO OP
                },
                heresphere::EventType::Play => {
                    // Update last known playback with is_paused = false, speed = event.speed, time = event.time
                    let new_session_state = SessionState {
                        id,
                        session: Session::User(User {
                            last_known_playback: Some(Playback {
                                is_paused: false,
                                speed: event.speed,
                                position_estimate: (event.time * 10000.0) as i64,
                                last_update: chrono::Utc::now(),
                                ..user.last_known_playback.unwrap()
                            }),
                            ..user
                        }),
                    };
                    app.update_session(new_session_state).await?;
                },
                heresphere::EventType::Pause => {
                    // Update last known playback with is_paused = true, speed = event.speed, time = event.time
                    let new_session_state = SessionState {
                        id,
                        session: Session::User(User {
                            last_known_playback: Some(Playback {
                                is_paused: true,
                                speed: event.speed,
                                position_estimate: (event.time * 10000.0) as i64,
                                last_update: chrono::Utc::now(),
                                ..user.last_known_playback.unwrap()
                            }),
                            ..user
                        }),
                    };
                    app.update_session(new_session_state).await?;
                },
                heresphere::EventType::Close => {
                    // Doesn't get called often enough to be useful currently
                },
            };
            Ok(())
        }

        Ok(_) => Ok(()),
        Err(err) => {
            tracing::warn!(
                error = ?err,
                "Failed to resolve session"
            );
            Err(AppError(err))
        },
    }
}

async fn progress_update_routine(app: &AppState) -> eyre::Result<()> {
    let mut updated = 0;
    let sessions: Vec<SessionState> = app.db.query("SELECT * FROM session").await?.check()?.take(0)?;
    for session in sessions {
        match session.session {
            Session::User(user) => {
                if let Some(playback) = user.last_known_playback {
                    if playback.is_paused {
                        continue;
                    }
                    let new_position = playback.position_estimate
                        + ((chrono::Utc::now().signed_duration_since(playback.last_update).num_milliseconds() as f64) * playback.speed).round() as i64 * 10000;
                    if playback.duration > 0 && new_position > playback.duration {
                        tracing::debug!(
                            video_id = &playback.video_id,
                            play_session_id = &playback.play_session_id,
                            "Playback position predicted to be greater than duration, stopping playback"
                        );
                        let new_session_state = SessionState {
                            id: session.id,
                            session: Session::User(User {
                                last_known_playback: Some(Playback {
                                    is_paused: true,
                                    last_update: chrono::Utc::now(),
                                    ..playback
                                }),
                                ..user
                            }),
                        };
                        app.update_session(new_session_state).await?;
                        continue;
                    }
                    tracing::debug!(
                        video_id = &playback.video_id,
                        play_session_id = &playback.play_session_id,
                        "Updating playback position from {} to {}",
                        playback.position_estimate,
                        new_position
                    );
                    let jellyfin_user = app.jellyfin.client.resume_user(&user.user_id, &user.token);
                    jellyfin_user.playback_progress(&playback.video_id, &playback.play_session_id, new_position, playback.is_paused, playback.started_at).await?;
                    let new_session_state = SessionState {
                        id: session.id,
                        session: Session::User(User {
                            last_known_playback: Some(Playback {
                                position_estimate: new_position,
                                last_update: chrono::Utc::now(),
                                ..playback
                            }),
                            ..user
                        }),
                    };
                    app.update_session(new_session_state).await?;
                    updated += 1;
                }
            }
            _ => {}
        }
    }
    tracing::info!(updated, "Updated playback positions");
    Ok(())
}

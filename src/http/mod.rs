//! HTTP skin. Loads values, calls a leaf, applies the effect, renders HTML.

mod auth;
mod churches;
mod context;
mod forms;
mod needs;
mod people;
mod push;
mod voice;

use axum::Router;
use axum::http::StatusCode;
use axum::middleware;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::db::Db;
use crate::leaf::{DemoSeat, Effect, Posture, VoiceKind};
use crate::sdk::judge::JudgeHub;
use crate::sdk::limit::{RateDecision, RateGate, RateKind};
use crate::sdk::push::PushHub;
use crate::sdk::refine::RefineHub;
use crate::sdk::session::{self, CookieTransport, Session};
use crate::views;
use axum_extra::extract::cookie::CookieJar;

pub use context::{security_headers, soften_form_errors};

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub secret: String,
    pub cookie: CookieTransport,
    pub demo: DemoSeat,
    pub push: PushHub,
    pub judge: JudgeHub,
    pub refine: RefineHub,
    pub gate: RateGate,
}

impl AppState {
    /// Persist the effect, then fan notices to installed phones.
    /// Push failure does not roll back the write.
    pub(crate) async fn commit(&self, effect: &Effect) -> Result<(), AppError> {
        self.db.apply(effect).await?;
        self.push.dispatch(&self.db, &effect.notices).await;
        Ok(())
    }

    pub(crate) async fn weigh(&self, kind: VoiceKind, parts: &[&str]) -> Posture {
        self.judge.weigh(kind, parts).await
    }

    pub(crate) fn awaiting_review(&self, pass: &str, parts: &[&str]) -> bool {
        self.refine.is_live()
            && crate::leaf::VoicePass::parse(pass) == crate::leaf::VoicePass::Review
            && parts.iter().any(|part| !part.trim().is_empty())
    }

    pub(crate) async fn polish(&self, kind: VoiceKind, text: &str) -> String {
        match self.refine.rewrite(kind, text).await {
            Ok(text) => text,
            Err(error) => {
                tracing::warn!("rewrite failed: {error:#}");
                text.trim().to_string()
            }
        }
    }

    pub(crate) fn put_session(&self, jar: CookieJar, session: &Session) -> CookieJar {
        session::put(jar, &self.secret, session, self.cookie)
    }

    pub(crate) fn clear_session(&self, jar: CookieJar) -> CookieJar {
        session::clear(jar, self.cookie)
    }

    pub(crate) fn decide_rate(&self, kind: RateKind, who: &str) -> RateDecision {
        self.gate.decide(kind, who)
    }
}

pub(crate) struct AppError(anyhow::Error);

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(error: E) -> Self {
        Self(error.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("{0:#}", self.0);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(
                views::error_page("Something broke on our end. Try again in a moment.")
                    .into_string(),
            ),
        )
            .into_response()
    }
}

pub async fn serve() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ecclesia=info,tower_http=info".into()),
        )
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://ecclesia.db".into());
    let secret = session_secret();
    let cookie = CookieTransport::from_env_value(std::env::var("ECCLESIA_SECURE").ok().as_deref());
    let demo = DemoSeat::from_env_value(std::env::var("ECCLESIA_DEMO").ok().as_deref());
    let db = Db::connect(&database_url).await?;
    let push = PushHub::load();
    let judge = JudgeHub::load();
    let refine = RefineHub::load();
    let state = AppState {
        db,
        secret,
        cookie,
        demo,
        push,
        judge,
        refine,
        gate: RateGate::new(),
    };

    let app = router(state);

    let addr = listen_addr();
    tracing::info!("ecclesia listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

fn session_secret() -> String {
    match std::env::var("ECCLESIA_SECRET") {
        Ok(value) if !value.is_empty() => {
            if value == "dev-only-change-me" {
                tracing::warn!(
                    "ECCLESIA_SECRET is the published development key. Anyone who knows it can forge a cookie."
                );
            }
            value
        }
        _ => {
            tracing::warn!(
                "ECCLESIA_SECRET is unset; cookies use a one-shot key. Sessions die on restart."
            );
            session::mint_secret()
        }
    }
}

fn listen_addr() -> SocketAddr {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(43781);
    SocketAddr::from(([0, 0, 0, 0], port))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(auth::landing))
        .route("/session", post(auth::start_session))
        .route("/session/logout", post(auth::logout))
        .route("/register", post(auth::register_user))
        .route("/refine", post(voice::refine_words))
        .route("/home", get(auth::home))
        .route(
            "/churches",
            get(churches::churches_index).post(churches::create_church),
        )
        .route("/churches/new", get(churches::church_new))
        .route("/churches/{id}", get(churches::church_show))
        .route("/churches/{id}/join", post(churches::join_church))
        .route("/churches/{id}/invite", post(churches::invite))
        .route("/invites/redeem", post(churches::redeem))
        .route(
            "/memberships/{id}/approve",
            post(churches::approve_membership_http),
        )
        .route(
            "/memberships/{id}/decline",
            post(churches::decline_membership_http),
        )
        .route(
            "/memberships/{id}/accept-invite",
            post(churches::accept_invite_http),
        )
        .route("/needs/new", get(needs::need_new))
        .route("/needs", post(needs::create_need))
        .route("/needs/{id}", get(needs::need_show))
        .route("/needs/{id}/apply", post(needs::apply_need))
        .route("/needs/{id}/close", post(needs::close_need_http))
        .route(
            "/applications/{id}/accept",
            post(needs::accept_application_http),
        )
        .route(
            "/applications/{id}/decline",
            post(needs::decline_application_http),
        )
        .route("/members/{id}", get(people::member_show))
        .route("/members/{id}/endorse", post(people::endorse_member))
        .route(
            "/endorsements/{id}/accept",
            post(people::accept_endorsement_http),
        )
        .route(
            "/endorsements/{id}/decline",
            post(people::decline_endorsement_http),
        )
        .route("/inbox", get(people::inbox))
        .route("/me", get(people::me).post(people::update_me))
        .route("/me/gifts", post(people::add_gift_http))
        .route("/me/gifts/{id}/remove", post(people::remove_gift_http))
        .route("/the-body", get(people::the_body))
        .route("/push/vapid", get(push::vapid_public))
        .route("/push/subscribe", post(push::subscribe))
        .route("/push/unsubscribe", post(push::unsubscribe))
        .route("/push/device", post(push::register_device))
        .route("/sw.js", get(push::service_worker))
        .nest_service(
            "/static",
            ServeDir::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static")),
        )
        .fallback(people::fallback)
        .layer(middleware::from_fn(soften_form_errors))
        .layer(middleware::from_fn(security_headers))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

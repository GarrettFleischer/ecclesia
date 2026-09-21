//! Optional rewrite. CSRF is enough; guests write a bio before they have a seat.

use axum::Json;
use axum::extract::{Form, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum_extra::extract::cookie::CookieJar;
use serde::Serialize;

use crate::leaf::{VoiceKind, rewrite_text};
use crate::sdk::limit::{RateDecision, RateKind};

use super::context::{ClientKey, bind_session, with_cookie};
use super::forms::RefineForm;
use super::{AppError, AppState};

#[derive(Serialize)]
struct RefineReply {
    text: String,
    seat: &'static str,
}

pub async fn refine_words(
    State(state): State<AppState>,
    who: ClientKey,
    jar: CookieJar,
    Form(form): Form<RefineForm>,
) -> Result<Response, AppError> {
    let (session, jar) = bind_session(jar, &state);
    if !session.check_csrf(&form.csrf) {
        return Ok(with_cookie(jar, refine_status(StatusCode::FORBIDDEN, "")));
    }
    if let RateDecision::Refuse = state.decide_rate(RateKind::Refine, &who.0) {
        return Ok(with_cookie(
            jar,
            refine_status(StatusCode::TOO_MANY_REQUESTS, ""),
        ));
    }
    let Some(kind) = VoiceKind::parse(form.kind.trim()) else {
        return Ok(with_cookie(
            jar,
            refine_status(StatusCode::BAD_REQUEST, &form.text),
        ));
    };
    let Ok(source) = rewrite_text(&form.text) else {
        return Ok(with_cookie(jar, refine_status(StatusCode::BAD_REQUEST, "")));
    };
    let text = match state.refine.rewrite(kind, &source).await {
        Ok(text) => text,
        Err(error) => {
            tracing::warn!("rewrite failed: {error:#}");
            source
        }
    };
    let seat = if state.refine.is_live() {
        "live"
    } else {
        "echo"
    };
    Ok(with_cookie(jar, Json(RefineReply { text, seat })))
}

fn refine_status(status: StatusCode, text: &str) -> (StatusCode, Json<RefineReply>) {
    (
        status,
        Json(RefineReply {
            text: text.to_string(),
            seat: "echo",
        }),
    )
}

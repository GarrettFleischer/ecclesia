use axum::extract::{Form, Path, Query, State};
use axum::response::{Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};

use crate::views;
use ecclesia_sdk::password::hash_token;
use ecclesia_sdk::prelude::{
    Place, PrayerMarkKind, PrayerProof, Viewer, VoiceKind, coordinates,
};
use ecclesia_sdk::story::{self, PrayerDeck, PrayerName};

use super::context::{
    html, leaf_err, redirect_err, redirect_ok, signed_form, signed_in, story_redirect, unread,
    viewer_for, with_cookie,
};
use super::forms::{FlashQuery, PlaceForm, PraiseForm, PrayerForm};
use super::{AppError, AppState};

pub async fn nearby(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    let place = shared_place(flash.lat.as_deref(), flash.lng.as_deref());
    let (needs, prayers) = match place {
        Some(place) => {
            let feed = story::nearby_feed(&state.sdk, &viewer, place).await?;
            (feed.needs, feed.prayers)
        }
        None => (Vec::new(), Vec::new()),
    };
    let fields = match (place, flash.lat.as_deref(), flash.lng.as_deref()) {
        (Some(_), Some(lat), Some(lng)) => Some((lat, lng)),
        _ => None,
    };
    Ok(with_cookie(
        signed.jar,
        html(views::nearby_page(
            &viewer,
            views::flash_from(flash.ok, flash.err),
            &needs,
            &prayers,
            fields,
            count,
            &signed.session.csrf,
        )),
    ))
}

pub async fn pray(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    let place = shared_place(flash.lat.as_deref(), flash.lng.as_deref());
    let deck = story::prayer_deck(&state.sdk, &viewer, place).await?;
    let fields = text_place(place, flash.lat.as_deref(), flash.lng.as_deref());
    let (card, source, empty, controls) = match deck {
        PrayerDeck::Card { card, source } => {
            let controls = controls_for(&state, &signed.jar, &viewer, &card).await?;
            (
                Some(card),
                Some(source),
                views::PrayEmpty::Finished,
                controls,
            )
        }
        PrayerDeck::SharePlace => (
            None,
            None,
            views::PrayEmpty::SharePlace,
            views::PrayerControls::Quiet,
        ),
        PrayerDeck::Finished => (
            None,
            None,
            empty_for(&viewer),
            views::PrayerControls::Quiet,
        ),
    };
    Ok(with_cookie(
        signed.jar,
        html(views::pray_page(
            &viewer,
            views::flash_from(flash.ok, flash.err),
            card.as_ref(),
            source,
            empty,
            controls,
            count,
            &signed.session.csrf,
            fields,
        )),
    ))
}

pub async fn prayer_new(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    let Some(church_id) = viewer.user.church_id.clone() else {
        return Ok(with_cookie(signed.jar, Redirect::to("/churches/join")));
    };
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    Ok(with_cookie(
        signed.jar,
        html(views::prayer_new(
            &viewer,
            count,
            views::flash_from(flash.ok, flash.err),
            &signed.session.csrf,
            &views::PrayerDraft::blank(&church_id),
        )),
    ))
}

pub async fn create_prayer(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<PrayerForm>,
) -> Result<Response, AppError> {
    let signed = match signed_form(&state, jar, &form.csrf, "/prayers/new").await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    if state.awaiting_review(&form.pass, &[&form.body]) {
        let body = state.polish(VoiceKind::Prayer, &form.body).await;
        let count = unread(&state.sdk.db, &viewer.user.id).await?;
        return Ok(with_cookie(
            signed.jar,
            html(views::prayer_new(
                &viewer,
                count,
                None,
                &signed.session.csrf,
                &views::PrayerDraft {
                    church_id: &form.church_id,
                    body: &body,
                    byline: &form.byline,
                    kind: views::DraftKind::Review,
                },
            )),
        ));
    }
    let name = match form.byline.as_str() {
        "unnamed" => PrayerName::Unnamed,
        _ => PrayerName::Signed,
    };
    match story::post_prayer(&state.sdk, &viewer, &form.church_id, &form.body, name).await? {
        Ok(posted) => {
            let dest = format!("/prayers/{}", posted.prayer_id);
            let mut jar = signed.jar;
            if let Some(token) = posted.manage_token {
                jar = remember_prayer(jar, &posted.prayer_id, &token);
            }
            Ok(with_cookie(jar, redirect_ok(&dest, "prayer_posted")))
        }
        Err(error) => Ok(with_cookie(signed.jar, leaf_err("/prayers/new", error))),
    }
}

pub async fn prayer_show(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(flash): Query<FlashQuery>,
) -> Result<Response, AppError> {
    let signed = match signed_in(&state, jar).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    paint_prayer(
        &state,
        signed.jar,
        viewer_for(&state.sdk.db, signed.user).await?,
        &id,
        &signed.session.csrf,
        views::flash_from(flash.ok, flash.err),
        shared_place(flash.lat.as_deref(), flash.lng.as_deref()),
        flash.lat.as_deref(),
        flash.lng.as_deref(),
        "",
        views::DraftKind::Blank,
    )
    .await
}

pub async fn pray_mark(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<PlaceForm>,
) -> Result<Response, AppError> {
    mark(&state, jar, &id, form, PrayerMarkKind::Prayed, "prayed").await
}

pub async fn next_mark(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<PlaceForm>,
) -> Result<Response, AppError> {
    mark(&state, jar, &id, form, PrayerMarkKind::Seen, "next").await
}

pub async fn answer_prayer(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Form(form): Form<PraiseForm>,
) -> Result<Response, AppError> {
    let dest = format!("/prayers/{id}");
    let signed = match signed_form(&state, jar, &form.csrf, &dest).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    if state.awaiting_review(&form.pass, &[&form.praise]) {
        let praise = state.polish(VoiceKind::Prayer, &form.praise).await;
        return paint_prayer(
            &state,
            signed.jar,
            viewer,
            &id,
            &signed.session.csrf,
            None,
            None,
            None,
            None,
            &praise,
            views::DraftKind::Review,
        )
        .await;
    }
    let proof = proof_for(&state, &signed.jar, &viewer, &id).await?;
    let Some(proof) = proof else {
        return Ok(with_cookie(signed.jar, redirect_err(&dest, "not_yours")));
    };
    story_redirect(
        signed.jar,
        &dest,
        story::answer_prayer(&state.sdk, &viewer, &id, &form.praise, proof).await?,
        "answered",
    )
}

pub async fn churches_nearby_redirect() -> Redirect {
    Redirect::to("/nearby")
}

async fn mark(
    state: &AppState,
    jar: CookieJar,
    id: &str,
    form: PlaceForm,
    kind: PrayerMarkKind,
    flash: &str,
) -> Result<Response, AppError> {
    let dest = pray_dest(&form.lat, &form.lng);
    let signed = match signed_form(state, jar, &form.csrf, &dest).await {
        Ok(signed) => signed,
        Err(response) => return Ok(response),
    };
    let viewer = viewer_for(&state.sdk.db, signed.user).await?;
    story_redirect(
        signed.jar,
        &dest,
        story::mark_prayer(
            &state.sdk,
            &viewer,
            id,
            shared_place(Some(&form.lat), Some(&form.lng)),
            kind,
        )
        .await?,
        flash,
    )
}

async fn paint_prayer(
    state: &AppState,
    jar: CookieJar,
    viewer: Viewer,
    id: &str,
    csrf: &str,
    flash: Option<views::Flash>,
    place: Option<Place>,
    lat: Option<&str>,
    lng: Option<&str>,
    praise: &str,
    kind: views::DraftKind,
) -> Result<Response, AppError> {
    let count = unread(&state.sdk.db, &viewer.user.id).await?;
    let Some(card) = state.sdk.db.prayer_card(id).await? else {
        return Ok(with_cookie(
            jar,
            html(views::sorry_page(
                "We couldn't find that prayer.",
                views::SorrySeat::Member {
                    user: &viewer.user,
                    unread: count,
                    csrf,
                },
            )),
        ));
    };
    let controls = controls_for(state, &jar, &viewer, &card).await?;
    Ok(with_cookie(
        jar,
        html(views::prayer_show(
            &viewer,
            flash,
            &card,
            controls,
            count,
            csrf,
            text_place(place, lat, lng),
            praise,
            kind,
        )),
    ))
}

async fn controls_for(
    state: &AppState,
    jar: &CookieJar,
    viewer: &Viewer,
    card: &ecclesia_sdk::prelude::PrayerCard,
) -> Result<views::PrayerControls, AppError> {
    if !card.is_open() {
        return Ok(views::PrayerControls::Quiet);
    }
    let can_mark = card.author_id.as_deref() != Some(viewer.user.id.as_str());
    let can_answer = match card.author_id.as_deref() {
        Some(author_id) => author_id == viewer.user.id,
        None => token_matches(state, jar, &card.id).await?,
    };
    Ok(match (can_mark, can_answer) {
        (true, true) => views::PrayerControls::MarkAndAnswer,
        (true, false) => views::PrayerControls::Mark,
        (false, true) => views::PrayerControls::Answer,
        (false, false) => views::PrayerControls::Quiet,
    })
}

async fn proof_for(
    state: &AppState,
    jar: &CookieJar,
    viewer: &Viewer,
    prayer_id: &str,
) -> Result<Option<PrayerProof>, AppError> {
    let Some(prayer) = state.sdk.db.prayer(prayer_id).await? else {
        return Ok(None);
    };
    if prayer.author_id.as_deref() == Some(viewer.user.id.as_str()) {
        return Ok(Some(PrayerProof::Author));
    }
    if token_matches(state, jar, prayer_id).await? {
        return Ok(Some(PrayerProof::Token));
    }
    Ok(None)
}

async fn token_matches(state: &AppState, jar: &CookieJar, prayer_id: &str) -> Result<bool, AppError> {
    let Some(token) = prayer_token(jar, prayer_id) else {
        return Ok(false);
    };
    let Some(prayer) = state.sdk.db.prayer(prayer_id).await? else {
        return Ok(false);
    };
    let hashed = hash_token(&token);
    Ok(prayer.manage_hash.as_deref() == Some(hashed.as_str()))
}

fn prayer_token(jar: &CookieJar, prayer_id: &str) -> Option<String> {
    jar.get(&format!("ecclesia_prayer_{prayer_id}"))
        .map(|cookie| cookie.value().to_string())
}

fn remember_prayer(jar: CookieJar, prayer_id: &str, token: &str) -> CookieJar {
    let mut cookie = Cookie::new(format!("ecclesia_prayer_{prayer_id}"), token.to_string());
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookie.set_same_site(SameSite::Lax);
    cookie.make_permanent();
    jar.add(cookie)
}

fn empty_for(viewer: &Viewer) -> views::PrayEmpty {
    if viewer.is_active_anywhere() {
        views::PrayEmpty::Finished
    } else {
        views::PrayEmpty::WaitingChurch
    }
}

fn shared_place(lat: Option<&str>, lng: Option<&str>) -> Option<Place> {
    let latitude = lat?.parse().ok()?;
    let longitude = lng?.parse().ok()?;
    coordinates(latitude, longitude)
        .ok()
        .map(|(latitude, longitude)| Place {
            latitude,
            longitude,
        })
}

fn text_place<'a>(
    place: Option<Place>,
    lat: Option<&'a str>,
    lng: Option<&'a str>,
) -> Option<(&'a str, &'a str)> {
    match (place, lat, lng) {
        (Some(_), Some(lat), Some(lng)) => Some((lat, lng)),
        _ => None,
    }
}

fn pray_dest(lat: &str, lng: &str) -> String {
    match shared_place(Some(lat), Some(lng)) {
        Some(place) => format!("/pray?lat={}&lng={}", place.latitude, place.longitude),
        None => "/pray".into(),
    }
}

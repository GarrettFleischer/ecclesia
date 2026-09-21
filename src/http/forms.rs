use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct FlashQuery {
    pub ok: Option<String>,
    pub err: Option<String>,
}

#[derive(Deserialize)]
pub struct SessionForm {
    pub csrf: String,
    pub user_id: String,
}

#[derive(Deserialize)]
pub struct CsrfForm {
    pub csrf: String,
}

#[derive(Deserialize)]
pub struct RegisterForm {
    pub csrf: String,
    pub name: String,
    pub email: String,
    pub city: String,
    pub region: String,
    #[serde(default)]
    pub bio: String,
}

#[derive(Deserialize)]
pub struct ChurchForm {
    pub csrf: String,
    pub name: String,
    pub city: String,
    pub region: String,
    #[serde(default)]
    pub gathering: String,
    pub description: String,
}

#[derive(Deserialize)]
pub struct InviteForm {
    pub csrf: String,
    pub email: String,
}

#[derive(Deserialize)]
pub struct RedeemForm {
    pub csrf: String,
    pub code: String,
}

#[derive(Deserialize)]
pub struct NeedForm {
    pub csrf: String,
    pub church_id: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub gift_id: String,
    pub scope: String,
}

#[derive(Deserialize)]
pub struct NeedQuery {
    pub church_id: Option<String>,
}

#[derive(Deserialize)]
pub struct ApplyForm {
    pub csrf: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct EndorseForm {
    pub csrf: String,
    pub skill: String,
    pub note: String,
}

#[derive(Deserialize)]
pub struct ProfileForm {
    pub csrf: String,
    pub name: String,
    pub city: String,
    pub region: String,
    #[serde(default)]
    pub bio: String,
}

#[derive(Deserialize)]
pub struct GiftForm {
    pub csrf: String,
    pub gift_id: String,
    #[serde(default)]
    pub note: String,
}

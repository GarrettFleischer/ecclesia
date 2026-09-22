use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct FlashQuery {
    pub ok: Option<String>,
    pub err: Option<String>,
    pub after: Option<String>,
    pub members_after: Option<String>,
}

#[derive(Deserialize)]
pub struct SessionForm {
    pub csrf: String,
    pub email: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Deserialize)]
pub struct TokenQuery {
    pub t: Option<String>,
}

#[derive(Deserialize)]
pub struct ResetCompleteForm {
    pub csrf: String,
    pub t: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct PasswordForm {
    pub csrf: String,
    pub current: String,
    pub password: String,
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
    #[serde(default)]
    pub pass: String,
    #[serde(default)]
    pub password: String,
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
    #[serde(default)]
    pub pass: String,
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
    #[serde(default)]
    pub pass: String,
}

#[derive(Deserialize)]
pub struct NeedQuery {
    pub church_id: Option<String>,
}

#[derive(Deserialize)]
pub struct ApplyForm {
    pub csrf: String,
    pub message: String,
    #[serde(default)]
    pub pass: String,
}

#[derive(Deserialize)]
pub struct EndorseForm {
    pub csrf: String,
    pub skill: String,
    pub note: String,
    #[serde(default)]
    pub pass: String,
}

#[derive(Deserialize)]
pub struct ProfileForm {
    pub csrf: String,
    pub name: String,
    pub city: String,
    pub region: String,
    #[serde(default)]
    pub bio: String,
    #[serde(default)]
    pub pass: String,
}

#[derive(Deserialize)]
pub struct RefineForm {
    pub csrf: String,
    pub kind: String,
    pub text: String,
}

#[derive(Deserialize)]
pub struct GiftForm {
    pub csrf: String,
    pub gift_id: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub pass: String,
}

#[derive(Deserialize)]
pub struct PushSubscribeForm {
    pub csrf: String,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

#[derive(Deserialize)]
pub struct PushUnsubscribeForm {
    pub csrf: String,
    pub endpoint: String,
}

#[derive(Deserialize)]
pub struct PushDeviceForm {
    pub csrf: String,
    pub token: String,
    pub platform: String,
}

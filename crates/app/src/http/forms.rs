use serde::Deserialize;

#[derive(Deserialize, Default)]
pub struct FlashQuery {
    pub ok: Option<String>,
    pub err: Option<String>,
    pub after: Option<String>,
    pub members_after: Option<String>,
    pub lat: Option<String>,
    pub lng: Option<String>,
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
pub struct TransferForm {
    pub csrf: String,
    pub church_id: String,
    pub email: String,
}

#[derive(Deserialize)]
pub struct ChurchPost {
    pub csrf: String,
    pub church_id: String,
}

#[derive(Deserialize)]
pub struct ImportNeedsForm {
    pub csrf: String,
    pub source_church_id: String,
    pub church_id: String,
}

#[derive(Deserialize)]
pub struct RegisterForm {
    pub csrf: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub code: String,
}

#[derive(Deserialize)]
pub struct RegisterQuery {
    #[serde(default)]
    pub ok: Option<String>,
    #[serde(default)]
    pub err: Option<String>,
    #[serde(default)]
    pub code: String,
}

#[derive(Deserialize)]
pub struct ChurchForm {
    pub csrf: String,
    pub name: String,
    #[serde(rename = "address-line1")]
    pub address_line1: String,
    #[serde(default, rename = "address-line2")]
    pub address_line2: String,
    #[serde(rename = "address-level2")]
    pub city: String,
    #[serde(rename = "address-level1")]
    pub address_state: String,
    #[serde(rename = "postal-code")]
    pub postal_code: String,
    pub ein: String,
    pub registry_state: String,
    pub registry_number: String,
    #[serde(default)]
    pub service_day_0: String,
    #[serde(default)]
    pub service_day_1: String,
    #[serde(default)]
    pub service_day_2: String,
    #[serde(default)]
    pub service_day_3: String,
    #[serde(default)]
    pub service_day_4: String,
    #[serde(default)]
    pub service_day_5: String,
    #[serde(default)]
    pub service_day_6: String,
    #[serde(default)]
    pub service_day_7: String,
    #[serde(default)]
    pub service_time_0: String,
    #[serde(default)]
    pub service_time_1: String,
    #[serde(default)]
    pub service_time_2: String,
    #[serde(default)]
    pub service_time_3: String,
    #[serde(default)]
    pub service_time_4: String,
    #[serde(default)]
    pub service_time_5: String,
    #[serde(default)]
    pub service_time_6: String,
    #[serde(default)]
    pub service_time_7: String,
    pub description: String,
    #[serde(default)]
    pub pass: String,
}

#[derive(Deserialize)]
pub struct JoinChurchForm {
    pub csrf: String,
    pub church_id: String,
}

#[derive(Deserialize)]
pub struct JoinQuery {
    #[serde(default)]
    pub ok: Option<String>,
    #[serde(default)]
    pub err: Option<String>,
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub lat: String,
    #[serde(default)]
    pub lng: String,
}

pub struct InviteForm {
    pub csrf: String,
    pub email: Vec<String>,
}

pub fn invite_form(body: &str) -> InviteForm {
    let mut form = InviteForm {
        csrf: String::new(),
        email: Vec::new(),
    };
    collect_invite_fields(&mut form, body);
    form
}

fn collect_invite_fields(form: &mut InviteForm, body: &str) {
    if body.is_empty() {
        return;
    }
    for pair in body.split('&') {
        push_invite_field(form, pair);
    }
}

fn push_invite_field(form: &mut InviteForm, pair: &str) {
    let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
    match form_decode(key).as_str() {
        "csrf" => form.csrf = form_decode(value),
        "email" => form.email.push(form_decode(value)),
        _ => {}
    }
}

fn form_decode(raw: &str) -> String {
    let mut bytes = Vec::new();
    push_form_bytes(&mut bytes, raw);
    String::from_utf8_lossy(&bytes).into_owned()
}

fn push_form_bytes(bytes: &mut Vec<u8>, raw: &str) {
    let input = raw.as_bytes();
    let mut index = 0;
    while index < input.len() {
        match input[index] {
            b'+' => {
                bytes.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < input.len() => match hex_byte(input[index + 1], input[index + 2]) {
                Some(byte) => {
                    bytes.push(byte);
                    index += 3;
                }
                None => {
                    bytes.push(b'%');
                    index += 1;
                }
            },
            byte => {
                bytes.push(byte);
                index += 1;
            }
        }
    }
}

fn hex_byte(hi: u8, lo: u8) -> Option<u8> {
    Some(hex_value(hi)? << 4 | hex_value(lo)?)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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
pub struct ReplyForm {
    pub csrf: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub pass: String,
    #[serde(default)]
    pub lat: String,
    #[serde(default)]
    pub lng: String,
    #[serde(default)]
    pub met: String,
}

#[derive(Deserialize)]
pub struct ShareMintForm {
    pub csrf: String,
    #[serde(default)]
    pub lat: String,
    #[serde(default)]
    pub lng: String,
}

#[derive(Deserialize)]
pub struct PrayerForm {
    pub csrf: String,
    pub church_id: String,
    pub body: String,
    pub byline: String,
    #[serde(default)]
    pub pass: String,
}

#[derive(Deserialize)]
pub struct PraiseForm {
    pub csrf: String,
    pub praise: String,
    #[serde(default)]
    pub pass: String,
}

#[derive(Deserialize)]
pub struct PlaceForm {
    pub csrf: String,
    #[serde(default)]
    pub lat: String,
    #[serde(default)]
    pub lng: String,
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
    pub first_name: String,
    pub last_name: String,
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

#[cfg(test)]
mod tests {
    use super::invite_form;

    #[test]
    fn invite_form_keeps_every_email() {
        let form = invite_form("csrf=abc&email=ada%40church.org&email=&email=james%40stlukes.test");
        assert_eq!(form.csrf, "abc");
        assert_eq!(
            form.email,
            vec![
                "ada@church.org".to_string(),
                String::new(),
                "james@stlukes.test".to_string()
            ]
        );
    }
}

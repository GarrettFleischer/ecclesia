use super::model::DomainError;

pub const NAME_MAX: usize = 80;
pub const EMAIL_MAX: usize = 120;
pub const ADDRESS_MAX: usize = 400;
const BIO_MAX: usize = 800;
const TITLE_MAX: usize = 120;
pub const GATHERING_MAX: usize = 240;
const SERVICE_LIMIT: usize = 8;
const BODY_MAX: usize = 2000;
const NOTE_MAX: usize = 600;

pub fn require_text(value: &str, max: usize) -> Result<String, DomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > max {
        Err(DomainError::InvalidInput)
    } else {
        Ok(trimmed.to_string())
    }
}

pub fn optional_text(value: &str, max: usize) -> Result<String, DomainError> {
    let trimmed = value.trim();
    if trimmed.chars().count() > max {
        Err(DomainError::InvalidInput)
    } else {
        Ok(trimmed.to_string())
    }
}

pub fn normalize_email(value: &str) -> Result<String, DomainError> {
    let email = require_text(value, EMAIL_MAX)?.to_lowercase();
    if email.chars().any(email_forbidden) {
        return Err(DomainError::InvalidEmail);
    }
    let (local, domain) = one_at(&email).ok_or(DomainError::InvalidEmail)?;
    if !local_ok(local) || !domain_ok(domain) {
        return Err(DomainError::InvalidEmail);
    }
    Ok(email)
}

fn email_forbidden(ch: char) -> bool {
    ch.is_whitespace() || ch.is_control()
}

fn one_at(email: &str) -> Option<(&str, &str)> {
    let (local, domain) = email.split_once('@')?;
    if domain.contains('@') {
        return None;
    }
    Some((local, domain))
}

fn local_ok(local: &str) -> bool {
    !local.is_empty()
        && local
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '+' | '-' | '_'))
        && !local.starts_with('.')
        && !local.ends_with('.')
}

fn domain_ok(domain: &str) -> bool {
    if !domain.contains('.') || domain.contains("..") {
        return false;
    }
    let mut labels = domain.split('.');
    labels.all(label_ok)
}

fn label_ok(label: &str) -> bool {
    !label.is_empty()
        && label
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        && !label.starts_with('-')
        && !label.ends_with('-')
}

pub fn person_fields(
    first_name: &str,
    last_name: &str,
    email: &str,
) -> Result<(String, String, String), DomainError> {
    Ok((
        require_text(first_name, NAME_MAX)?,
        require_text(last_name, NAME_MAX)?,
        normalize_email(email)?,
    ))
}

pub fn profile_fields(
    first_name: &str,
    last_name: &str,
    bio: &str,
) -> Result<(String, String, String), DomainError> {
    Ok((
        require_text(first_name, NAME_MAX)?,
        require_text(last_name, NAME_MAX)?,
        optional_text(bio, BIO_MAX)?,
    ))
}

pub const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

pub const US_STATES: &[(&str, &str)] = &[
    ("AL", "Alabama"),
    ("AK", "Alaska"),
    ("AZ", "Arizona"),
    ("AR", "Arkansas"),
    ("CA", "California"),
    ("CO", "Colorado"),
    ("CT", "Connecticut"),
    ("DE", "Delaware"),
    ("DC", "District of Columbia"),
    ("FL", "Florida"),
    ("GA", "Georgia"),
    ("HI", "Hawaii"),
    ("ID", "Idaho"),
    ("IL", "Illinois"),
    ("IN", "Indiana"),
    ("IA", "Iowa"),
    ("KS", "Kansas"),
    ("KY", "Kentucky"),
    ("LA", "Louisiana"),
    ("ME", "Maine"),
    ("MD", "Maryland"),
    ("MA", "Massachusetts"),
    ("MI", "Michigan"),
    ("MN", "Minnesota"),
    ("MS", "Mississippi"),
    ("MO", "Missouri"),
    ("MT", "Montana"),
    ("NE", "Nebraska"),
    ("NV", "Nevada"),
    ("NH", "New Hampshire"),
    ("NJ", "New Jersey"),
    ("NM", "New Mexico"),
    ("NY", "New York"),
    ("NC", "North Carolina"),
    ("ND", "North Dakota"),
    ("OH", "Ohio"),
    ("OK", "Oklahoma"),
    ("OR", "Oregon"),
    ("PA", "Pennsylvania"),
    ("RI", "Rhode Island"),
    ("SC", "South Carolina"),
    ("SD", "South Dakota"),
    ("TN", "Tennessee"),
    ("TX", "Texas"),
    ("UT", "Utah"),
    ("VT", "Vermont"),
    ("VA", "Virginia"),
    ("WA", "Washington"),
    ("WV", "West Virginia"),
    ("WI", "Wisconsin"),
    ("WY", "Wyoming"),
];

pub fn state_label(code: &str) -> &str {
    US_STATES
        .iter()
        .find(|(abbrev, _)| *abbrev == code)
        .map(|(_, name)| *name)
        .unwrap_or(code)
}

pub fn postal_address(
    line1: &str,
    line2: &str,
    city: &str,
    state: &str,
    postal: &str,
) -> Result<String, DomainError> {
    let line1 = require_text(line1, 100)?;
    let line2 = optional_text(line2, 100)?;
    let city = require_text(city, 80)?;
    let state = us_state(state)?;
    let postal = postal_code(postal)?;
    let mut address = line1;
    if !line2.is_empty() {
        address.push('\n');
        address.push_str(&line2);
    }
    address.push('\n');
    address.push_str(&city);
    address.push_str(", ");
    address.push_str(&state);
    address.push(' ');
    address.push_str(&postal);
    if address.chars().count() > ADDRESS_MAX {
        return Err(DomainError::InvalidInput);
    }
    Ok(address)
}

fn postal_code(value: &str) -> Result<String, DomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DomainError::InvalidInput);
    }
    let digits = trimmed.as_bytes();
    let shaped = match digits.len() {
        5 => digits.iter().all(u8::is_ascii_digit),
        10 => {
            digits[5] == b'-'
                && digits[..5].iter().all(u8::is_ascii_digit)
                && digits[6..].iter().all(u8::is_ascii_digit)
        }
        _ => false,
    };
    if shaped {
        Ok(trimmed.to_string())
    } else {
        Err(DomainError::InvalidPostal)
    }
}

pub fn registration_fields(
    ein: &str,
    registry_state: &str,
    registry_number: &str,
) -> Result<(String, String, String), DomainError> {
    Ok((
        employer_id(ein)?,
        us_state(registry_state)?,
        state_registry_number(registry_number)?,
    ))
}

fn employer_id(value: &str) -> Result<String, DomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DomainError::InvalidInput);
    }
    if trimmed
        .chars()
        .any(|ch| !ch.is_ascii_digit() && ch != '-' && !ch.is_whitespace())
    {
        return Err(DomainError::InvalidEin);
    }
    let digits: String = trimmed.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.len() != 9 || digits.starts_with("00") {
        return Err(DomainError::InvalidEin);
    }
    Ok(format!("{}-{}", &digits[..2], &digits[2..]))
}

fn us_state(value: &str) -> Result<String, DomainError> {
    let code = value.trim().to_ascii_uppercase();
    if let Some((abbrev, _)) = US_STATES.iter().find(|(abbrev, _)| *abbrev == code) {
        Ok((*abbrev).to_string())
    } else {
        Err(DomainError::InvalidInput)
    }
}

fn state_registry_number(value: &str) -> Result<String, DomainError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DomainError::InvalidInput);
    }
    if trimmed.chars().count() > 40
        || !trimmed.chars().any(|ch| ch.is_ascii_digit())
        || !trimmed.chars().all(registry_char)
    {
        return Err(DomainError::InvalidRegistry);
    }
    Ok(trimmed.to_string())
}

fn registry_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == ' '
}

pub fn service_schedule(days: &[&str], times: &[&str]) -> Result<String, DomainError> {
    if days.len() != times.len() || days.len() > SERVICE_LIMIT {
        return Err(DomainError::InvalidInput);
    }
    let gathering = schedule_lines(days, times)?.join("\n");
    if gathering.chars().count() > GATHERING_MAX {
        return Err(DomainError::InvalidInput);
    }
    Ok(gathering)
}

fn schedule_lines(days: &[&str], times: &[&str]) -> Result<Vec<String>, DomainError> {
    let mut lines = Vec::new();
    for (day, time) in days.iter().zip(times.iter()) {
        if let Some(line) = service_line(day, time)? {
            lines.push(line);
        }
    }
    Ok(lines)
}

fn service_line(day: &str, time: &str) -> Result<Option<String>, DomainError> {
    let day = day.trim();
    let time = time.trim();
    if day.is_empty() && time.is_empty() {
        return Ok(None);
    }
    let day = weekday(day)?;
    let label = clock_label(clock_minutes(time)?);
    Ok(Some(format!("{day} at {label}")))
}

fn weekday(value: &str) -> Result<&'static str, DomainError> {
    WEEKDAYS
        .into_iter()
        .find(|day| day.eq_ignore_ascii_case(value.trim()))
        .ok_or(DomainError::InvalidService)
}

fn clock_minutes(value: &str) -> Result<u16, DomainError> {
    let Some((hour, rest)) = value.split_once(':') else {
        return Err(DomainError::InvalidService);
    };
    let minute = match rest.split_once(':') {
        Some((minute, seconds))
            if !seconds.is_empty() && seconds.chars().all(|ch| ch.is_ascii_digit()) =>
        {
            minute
        }
        Some(_) => return Err(DomainError::InvalidService),
        None => rest,
    };
    let hour: u16 = hour.parse().map_err(|_| DomainError::InvalidService)?;
    let minute: u16 = minute.parse().map_err(|_| DomainError::InvalidService)?;
    if hour > 23 || minute > 59 {
        return Err(DomainError::InvalidService);
    }
    Ok(hour * 60 + minute)
}

fn clock_label(minutes: u16) -> String {
    let hour24 = minutes / 60;
    let minute = minutes % 60;
    let suffix = if hour24 < 12 { "a.m." } else { "p.m." };
    let hour = match hour24 % 12 {
        0 => 12,
        hour => hour,
    };
    if minute == 0 {
        format!("{hour} {suffix}")
    } else {
        format!("{hour}:{minute:02} {suffix}")
    }
}

pub fn church_fields(
    name: &str,
    address: &str,
    latitude: f64,
    longitude: f64,
    description: &str,
    gathering: &str,
) -> Result<(String, String, f64, f64, String, String), DomainError> {
    let name = require_text(name, TITLE_MAX)?;
    if !name.chars().any(|ch| ch.is_ascii_alphanumeric()) {
        return Err(DomainError::InvalidInput);
    }
    let (latitude, longitude) = coordinates(latitude, longitude)?;
    Ok((
        name,
        require_text(address, ADDRESS_MAX)?,
        latitude,
        longitude,
        require_text(description, BODY_MAX)?,
        optional_text(gathering, GATHERING_MAX)?,
    ))
}

pub fn coordinates(latitude: f64, longitude: f64) -> Result<(f64, f64), DomainError> {
    if !latitude.is_finite()
        || !longitude.is_finite()
        || !(-90.0..=90.0).contains(&latitude)
        || !(-180.0..=180.0).contains(&longitude)
    {
        return Err(DomainError::InvalidInput);
    }
    Ok((latitude, longitude))
}

pub fn need_fields(
    title: &str,
    body: &str,
    scope: &str,
) -> Result<(String, String, crate::NeedScope), DomainError> {
    let scope = crate::NeedScope::parse(scope).ok_or(DomainError::InvalidInput)?;
    Ok((
        require_text(title, TITLE_MAX)?,
        require_text(body, BODY_MAX)?,
        scope,
    ))
}

pub fn note_field(value: &str) -> Result<String, DomainError> {
    require_text(value, NOTE_MAX)
}

pub fn prayer_body(value: &str) -> Result<String, DomainError> {
    require_text(value, BODY_MAX)
}

pub fn skill_field(value: &str) -> Result<String, DomainError> {
    require_text(value, TITLE_MAX)
}

pub fn optional_note(value: &str) -> Result<String, DomainError> {
    optional_text(value, NOTE_MAX)
}

pub fn rewrite_text(value: &str) -> Result<String, DomainError> {
    optional_text(value, BODY_MAX)
}

pub fn https_endpoint(value: &str) -> Result<String, DomainError> {
    let trimmed = require_text(value, 2048)?;
    let rest = trimmed
        .strip_prefix("https://")
        .ok_or(DomainError::InvalidInput)?;
    if rest.is_empty() || rest.chars().any(endpoint_forbidden) {
        return Err(DomainError::InvalidInput);
    }
    Ok(trimmed)
}

fn endpoint_forbidden(ch: char) -> bool {
    ch.is_whitespace() || ch == '<' || ch == '>' || ch == '"'
}

pub fn push_key(value: &str) -> Result<String, DomainError> {
    let trimmed = require_text(value, 256)?;
    if !trimmed.chars().all(push_key_char) {
        return Err(DomainError::InvalidInput);
    }
    Ok(trimmed)
}

fn push_key_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '=' | '+' | '/')
}

pub fn device_token(value: &str) -> Result<String, DomainError> {
    require_text(value, 512)
}

pub fn push_platform(value: &str) -> Result<&str, DomainError> {
    match value.trim() {
        "web" | "ios" | "android" => Ok(value.trim()),
        _ => Err(DomainError::InvalidInput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_val_01_rejects_empty_and_overlong() {
        assert_eq!(require_text("  ", 10), Err(DomainError::InvalidInput));
        assert_eq!(
            require_text(&"x".repeat(11), 10),
            Err(DomainError::InvalidInput)
        );
        assert_eq!(require_text(" Ruth ", 10).unwrap(), "Ruth");
    }

    #[test]
    fn us_val_01_email_must_look_like_an_address() {
        assert_eq!(
            normalize_email("not-an-email"),
            Err(DomainError::InvalidEmail)
        );
        assert_eq!(normalize_email("a@b"), Err(DomainError::InvalidEmail));
        assert_eq!(
            normalize_email(" Miriam@Grace.Test ").unwrap(),
            "miriam@grace.test"
        );
        assert_eq!(
            normalize_email("ada@@nope.com"),
            Err(DomainError::InvalidEmail)
        );
        assert_eq!(
            normalize_email("ada@nope.com@x.com"),
            Err(DomainError::InvalidEmail)
        );
        assert_eq!(
            normalize_email("ada@nope..com"),
            Err(DomainError::InvalidEmail)
        );
    }

    #[test]
    fn us_val_01_church_name_needs_a_letter() {
        assert_eq!(
            church_fields(
                "!!!",
                "100 Main Street",
                42.53,
                -92.45,
                "Sunday gathering.",
                ""
            ),
            Err(DomainError::InvalidInput)
        );
        assert!(
            church_fields(
                "House of Bread",
                "100 Main Street",
                42.53,
                -92.45,
                "Sunday gathering.",
                ""
            )
            .is_ok()
        );
        assert_eq!(
            church_fields(
                "Grace",
                "100 Main Street",
                91.0,
                0.0,
                "Sunday gathering.",
                ""
            ),
            Err(DomainError::InvalidInput)
        );
    }

    #[test]
    fn us_sec_08_rewrite_and_push_fields_cap() {
        assert_eq!(
            rewrite_text(&"x".repeat(2001)),
            Err(DomainError::InvalidInput)
        );
        assert_eq!(rewrite_text("  She stayed.  ").unwrap(), "She stayed.");
        assert_eq!(
            https_endpoint("http://push.example/a"),
            Err(DomainError::InvalidInput)
        );
        assert_eq!(
            https_endpoint("https://push.example/a").unwrap(),
            "https://push.example/a"
        );
        assert_eq!(push_platform("android").unwrap(), "android");
        assert_eq!(push_platform("desktop"), Err(DomainError::InvalidInput));
    }

    #[test]
    fn us_val_02_registration_normalizes_the_government_ids() {
        assert_eq!(
            registration_fields(" 123456789 ", "ia", " CH-12 ").unwrap(),
            ("12-3456789".into(), "IA".into(), "CH-12".into())
        );
        assert_eq!(
            registration_fields("", "IA", "123456"),
            Err(DomainError::InvalidInput)
        );
        assert_eq!(
            registration_fields("12-345678", "IA", "123456"),
            Err(DomainError::InvalidEin)
        );
        assert_eq!(
            registration_fields("00-1234567", "IA", "123456"),
            Err(DomainError::InvalidEin)
        );
        assert_eq!(
            registration_fields("12-3456789", "ZZ", "123456"),
            Err(DomainError::InvalidInput)
        );
        assert_eq!(
            registration_fields("12-3456789", "IA", "abcdef"),
            Err(DomainError::InvalidRegistry)
        );
        assert_eq!(state_label("IA"), "Iowa");
    }

    #[test]
    fn us_val_03_postal_address_keeps_each_line() {
        assert_eq!(
            postal_address(" 100 Main Street ", "", "Cedar Falls", "ia", "50613").unwrap(),
            "100 Main Street\nCedar Falls, IA 50613"
        );
        assert_eq!(
            postal_address(
                "100 Main Street",
                "Suite 2",
                "Cedar Falls",
                "IA",
                "50613-1234"
            )
            .unwrap(),
            "100 Main Street\nSuite 2\nCedar Falls, IA 50613-1234"
        );
        assert_eq!(
            postal_address("", "", "Cedar Falls", "IA", "50613"),
            Err(DomainError::InvalidInput)
        );
        assert_eq!(
            postal_address("100 Main Street", "", "Cedar Falls", "IA", "506"),
            Err(DomainError::InvalidPostal)
        );
    }

    #[test]
    fn us_val_04_service_schedule_names_each_clock() {
        assert_eq!(
            service_schedule(&["Sunday"], &["10:00"]).unwrap(),
            "Sunday at 10 a.m."
        );
        assert_eq!(
            service_schedule(&["Sunday", "Wednesday"], &["10:00", "18:30"]).unwrap(),
            "Sunday at 10 a.m.\nWednesday at 6:30 p.m."
        );
        assert_eq!(service_schedule(&["", ""], &["", ""]).unwrap(), "");
        assert_eq!(
            service_schedule(&["Sunday"], &["12:00"]).unwrap(),
            "Sunday at 12 p.m."
        );
        assert_eq!(
            service_schedule(&[" sunday "], &["00:00:00"]).unwrap(),
            "Sunday at 12 a.m."
        );
        assert_eq!(
            service_schedule(&["Sunday"], &[""]),
            Err(DomainError::InvalidService)
        );
        assert_eq!(
            service_schedule(&[""], &["10:00"]),
            Err(DomainError::InvalidService)
        );
        assert_eq!(
            service_schedule(&["Funday"], &["10:00"]),
            Err(DomainError::InvalidService)
        );
        assert_eq!(
            service_schedule(&["Sunday"], &["25:00"]),
            Err(DomainError::InvalidService)
        );
        assert_eq!(
            service_schedule(&["Sunday"; 9], &["10:00"; 9]),
            Err(DomainError::InvalidInput)
        );
    }
}

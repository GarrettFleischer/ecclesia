use super::model::DomainError;

const NAME_MAX: usize = 80;
const EMAIL_MAX: usize = 120;
const PLACE_MAX: usize = 80;
const BIO_MAX: usize = 800;
const TITLE_MAX: usize = 120;
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
    let (local, domain) = email.split_once('@').ok_or(DomainError::InvalidEmail)?;
    if local.is_empty() || !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.')
    {
        return Err(DomainError::InvalidEmail);
    }
    if email.chars().any(|c| c.is_whitespace()) {
        return Err(DomainError::InvalidEmail);
    }
    Ok(email)
}

pub fn person_fields(
    name: &str,
    email: &str,
    city: &str,
    region: &str,
    bio: &str,
) -> Result<(String, String, String, String, String), DomainError> {
    Ok((
        require_text(name, NAME_MAX)?,
        normalize_email(email)?,
        require_text(city, PLACE_MAX)?,
        require_text(region, PLACE_MAX)?,
        optional_text(bio, BIO_MAX)?,
    ))
}

pub fn profile_fields(
    name: &str,
    city: &str,
    region: &str,
    bio: &str,
) -> Result<(String, String, String, String), DomainError> {
    Ok((
        require_text(name, NAME_MAX)?,
        require_text(city, PLACE_MAX)?,
        require_text(region, PLACE_MAX)?,
        optional_text(bio, BIO_MAX)?,
    ))
}

pub fn church_fields(
    name: &str,
    city: &str,
    region: &str,
    description: &str,
    gathering: &str,
) -> Result<(String, String, String, String, String), DomainError> {
    Ok((
        require_text(name, TITLE_MAX)?,
        require_text(city, PLACE_MAX)?,
        require_text(region, PLACE_MAX)?,
        require_text(description, BODY_MAX)?,
        optional_text(gathering, TITLE_MAX)?,
    ))
}

pub fn need_fields(
    title: &str,
    body: &str,
    scope: &str,
) -> Result<(String, String, crate::leaf::NeedScope), DomainError> {
    let scope = crate::leaf::NeedScope::parse(scope).ok_or(DomainError::InvalidInput)?;
    Ok((
        require_text(title, TITLE_MAX)?,
        require_text(body, BODY_MAX)?,
        scope,
    ))
}

pub fn note_field(value: &str) -> Result<String, DomainError> {
    require_text(value, NOTE_MAX)
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
}

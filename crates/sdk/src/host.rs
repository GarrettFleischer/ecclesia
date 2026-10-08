//! Public host checks. Fly or ECCLESIA_PUBLIC=1.

pub fn is_public_host() -> bool {
    std::env::var("FLY_APP_NAME").is_ok()
        || std::env::var("ECCLESIA_PUBLIC").ok().as_deref() == Some("1")
}

#[derive(Debug)]
pub struct MailEnv {
    pub api_key: Option<String>,
    pub from: Option<String>,
    pub origin: String,
}

pub fn load_mail_env(listen_origin: &str) -> anyhow::Result<MailEnv> {
    let kind = if is_public_host() {
        HostKind::Public
    } else {
        HostKind::Local
    };
    mail_env_from(
        kind,
        std::env::var("RESEND_API_KEY")
            .ok()
            .filter(|v| !v.is_empty()),
        std::env::var("RESEND_FROM").ok().filter(|v| !v.is_empty()),
        std::env::var("ECCLESIA_PUBLIC_URL")
            .ok()
            .map(|value| value.trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty()),
        listen_origin,
    )
}

pub fn mail_env_from(
    public: HostKind,
    api_key: Option<String>,
    from: Option<String>,
    public_url: Option<String>,
    listen_origin: &str,
) -> anyhow::Result<MailEnv> {
    match public {
        HostKind::Public => {
            let api_key = api_key
                .ok_or_else(|| anyhow::anyhow!("RESEND_API_KEY is required on a public host"))?;
            let from =
                from.ok_or_else(|| anyhow::anyhow!("RESEND_FROM is required on a public host"))?;
            let origin = public_url.ok_or_else(|| {
                anyhow::anyhow!("ECCLESIA_PUBLIC_URL is required on a public host")
            })?;
            Ok(MailEnv {
                api_key: Some(api_key),
                from: Some(from),
                origin,
            })
        }
        HostKind::Local => Ok(MailEnv {
            api_key,
            from,
            origin: public_url.unwrap_or_else(|| listen_origin.trim_end_matches('/').to_string()),
        }),
    }
}

#[derive(Clone, Copy)]
pub enum HostKind {
    Public,
    Local,
}

/// R2 settings for a public host. Secrets stay in this value and are not logged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct R2Config {
    pub account_id: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket: String,
}

/// Which R2 variable a public host still needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaEnvError {
    MissingAccountId,
    MissingAccessKeyId,
    MissingSecretAccessKey,
    MissingBucket,
}

impl std::fmt::Display for MediaEnvError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::MissingAccountId => "missing_r2_account_id",
            Self::MissingAccessKeyId => "missing_r2_access_key_id",
            Self::MissingSecretAccessKey => "missing_r2_secret_access_key",
            Self::MissingBucket => "missing_r2_bucket",
        })
    }
}

impl std::error::Error for MediaEnvError {}

/// Whether this process must talk to R2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaEnv {
    /// A local host stores files on disk. R2 variables are not required.
    NotRequired,
    R2(R2Config),
}

/// Checks public-host R2 configuration.
///
/// # Parameters
/// Empty strings count as missing. A local host does not require the four values.
///
/// # Returns
/// [`MediaEnv::NotRequired`] on a local host, or the four R2 values on a public host.
///
/// # Errors
/// The first missing public-host variable, in account, access key, secret, bucket order.
///
/// # Notes
/// This does not read the process environment and does not run at boot.
/// [`load_media_host`] reads the environment when a media operation needs a store.
///
/// # Examples
/// ```
/// use ecclesia_sdk::host::{media_env, HostKind, MediaEnv, MediaEnvError};
///
/// let local = media_env(HostKind::Local, None, None, None, None).unwrap();
/// assert_eq!(local, MediaEnv::NotRequired);
/// let missing = media_env(
///     HostKind::Public,
///     None,
///     Some("key".into()),
///     Some("secret".into()),
///     Some("bucket".into()),
/// );
/// assert_eq!(missing.unwrap_err(), MediaEnvError::MissingAccountId);
/// ```
pub fn media_env(
    kind: HostKind,
    account_id: Option<String>,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    bucket: Option<String>,
) -> Result<MediaEnv, MediaEnvError> {
    match kind {
        HostKind::Local => Ok(MediaEnv::NotRequired),
        HostKind::Public => Ok(MediaEnv::R2(require_r2(
            account_id,
            access_key_id,
            secret_access_key,
            bucket,
        )?)),
    }
}

/// Reads `R2_*` when this process is a public host. A local host returns
/// [`MediaEnv::NotRequired`] even when those variables are absent.
pub fn load_media_host() -> Result<MediaEnv, MediaEnvError> {
    let kind = if is_public_host() {
        HostKind::Public
    } else {
        HostKind::Local
    };
    media_env(
        kind,
        env_nonempty("R2_ACCOUNT_ID"),
        env_nonempty("R2_ACCESS_KEY_ID"),
        env_nonempty("R2_SECRET_ACCESS_KEY"),
        env_nonempty("R2_BUCKET"),
    )
}

/// `ECCLESIA_MEDIA_DIR` when it is set, otherwise `var/ecclesia-media`.
pub fn local_media_directory(explicit: Option<&str>) -> std::path::PathBuf {
    match explicit.map(str::trim).filter(|path| !path.is_empty()) {
        Some(path) => std::path::PathBuf::from(path),
        None => std::path::PathBuf::from("var/ecclesia-media"),
    }
}

/// The local media directory from the environment.
pub fn local_media_directory_from_env() -> std::path::PathBuf {
    local_media_directory(std::env::var("ECCLESIA_MEDIA_DIR").ok().as_deref())
}

fn require_r2(
    account_id: Option<String>,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    bucket: Option<String>,
) -> Result<R2Config, MediaEnvError> {
    Ok(R2Config {
        account_id: required(account_id, MediaEnvError::MissingAccountId)?,
        access_key_id: required(access_key_id, MediaEnvError::MissingAccessKeyId)?,
        secret_access_key: required(secret_access_key, MediaEnvError::MissingSecretAccessKey)?,
        bucket: required(bucket, MediaEnvError::MissingBucket)?,
    })
}

fn required(value: Option<String>, missing: MediaEnvError) -> Result<String, MediaEnvError> {
    match value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
    {
        Some(text) => Ok(text),
        None => Err(missing),
    }
}

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_host_public_mail_refuses_without_from() {
        let error = mail_env_from(
            HostKind::Public,
            Some("re_test".into()),
            None,
            Some("https://ecclesia.example".into()),
            "http://127.0.0.1:3000",
        )
        .unwrap_err();
        assert!(error.to_string().contains("RESEND_FROM"));
    }

    #[test]
    fn us_host_local_mail_uses_listen_origin() {
        let env = mail_env_from(HostKind::Local, None, None, None, "http://127.0.0.1:43781/")
            .expect("local mail");
        assert_eq!(env.origin, "http://127.0.0.1:43781");
        assert!(env.api_key.is_none());
    }

    #[test]
    fn us_host_public_mail_uses_public_url() {
        let env = mail_env_from(
            HostKind::Public,
            Some("re_test".into()),
            Some("Ecclesia <mail@ecclesia.test>".into()),
            Some("https://ecclesia.example".into()),
            "http://127.0.0.1:3000",
        )
        .expect("public mail");
        assert_eq!(env.origin, "https://ecclesia.example");
        assert_eq!(env.from.as_deref(), Some("Ecclesia <mail@ecclesia.test>"));
    }

    #[test]
    fn public_media_env_names_the_first_missing_variable() {
        assert_eq!(
            media_env(HostKind::Public, None, None, None, None).unwrap_err(),
            MediaEnvError::MissingAccountId
        );
        assert_eq!(
            media_env(
                HostKind::Public,
                Some("account".into()),
                Some("  ".into()),
                Some("secret".into()),
                Some("bucket".into()),
            )
            .unwrap_err(),
            MediaEnvError::MissingAccessKeyId
        );
        assert_eq!(
            media_env(
                HostKind::Public,
                Some("account".into()),
                Some("key".into()),
                None,
                Some("bucket".into()),
            )
            .unwrap_err(),
            MediaEnvError::MissingSecretAccessKey
        );
        assert_eq!(
            media_env(
                HostKind::Public,
                Some("account".into()),
                Some("key".into()),
                Some("secret".into()),
                None,
            )
            .unwrap_err(),
            MediaEnvError::MissingBucket
        );
    }

    #[test]
    fn local_media_env_does_not_require_r2() {
        let env = media_env(HostKind::Local, None, None, None, None).unwrap();
        assert_eq!(env, MediaEnv::NotRequired);
        assert_eq!(
            local_media_directory(None),
            std::path::PathBuf::from("var/ecclesia-media")
        );
        assert_eq!(
            local_media_directory(Some(" /tmp/photos ")),
            std::path::PathBuf::from("/tmp/photos")
        );
    }

    #[test]
    fn public_media_env_keeps_the_four_values() {
        let env = media_env(
            HostKind::Public,
            Some(" account ".into()),
            Some("key".into()),
            Some("secret".into()),
            Some("ecclesia-media".into()),
        )
        .unwrap();
        let MediaEnv::R2(config) = env else {
            panic!("public host without r2");
        };
        assert_eq!(config.account_id, "account");
        assert_eq!(config.bucket, "ecclesia-media");
        assert_eq!(config.access_key_id, "key");
        assert_eq!(config.secret_access_key, "secret");
    }
}

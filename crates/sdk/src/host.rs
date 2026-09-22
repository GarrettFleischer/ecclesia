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
        std::env::var("RESEND_API_KEY").ok().filter(|v| !v.is_empty()),
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
            let api_key =
                api_key.ok_or_else(|| anyhow::anyhow!("RESEND_API_KEY is required on a public host"))?;
            let from =
                from.ok_or_else(|| anyhow::anyhow!("RESEND_FROM is required on a public host"))?;
            let origin = public_url
                .ok_or_else(|| anyhow::anyhow!("ECCLESIA_PUBLIC_URL is required on a public host"))?;
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
}

//! Auth writes that ride beside a Domain Effect in one transaction.

#[derive(Debug, Clone, Default)]
pub struct StoryExtras {
    pub password_hash: Option<PasswordHashWrite>,
    pub session: Option<SessionWrite>,
    pub delete_sessions_user: Option<String>,
    pub delete_session_id: Option<String>,
    pub retire_magic_user: Option<String>,
    pub retire_reset_user: Option<String>,
    pub insert_magic: Option<TokenWrite>,
    pub insert_reset: Option<TokenWrite>,
    pub consume_magic_id: Option<String>,
    pub consume_reset_id: Option<String>,
    pub mail: Option<MailWrite>,
}

#[derive(Debug, Clone)]
pub struct PasswordHashWrite {
    pub user_id: String,
    pub hash: String,
}

#[derive(Debug, Clone)]
pub struct SessionWrite {
    pub id: String,
    pub user_id: String,
    pub csrf: String,
    pub created_at: String,
    pub last_seen_at: String,
    pub user_agent: String,
    pub ip: String,
}

#[derive(Debug, Clone)]
pub struct TokenWrite {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub expires_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct MailWrite {
    pub to: String,
    pub subject: String,
    pub text: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionRow {
    pub id: String,
    pub user_id: String,
    pub csrf: String,
    pub created_at: String,
    pub last_seen_at: String,
    pub user_agent: String,
    pub ip: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TokenRow {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub expires_at: String,
    pub consumed_at: Option<String>,
    pub created_at: String,
}

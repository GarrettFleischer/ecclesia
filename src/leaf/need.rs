use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedScope {
    Church,
    Neighboring,
    Body,
}

impl NeedScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Church => "church",
            Self::Neighboring => "neighboring",
            Self::Body => "body",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "church" => Some(Self::Church),
            "neighboring" => Some(Self::Neighboring),
            "body" => Some(Self::Body),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Church => "This church",
            Self::Neighboring => "Nearby churches",
            Self::Body => "Everyone",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct Need {
    pub id: String,
    pub church_id: String,
    pub author_id: String,
    pub title: String,
    pub body: String,
    pub gift_id: Option<String>,
    pub scope: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedStatus {
    Open,
    Closed,
}

impl NeedStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "open" => Some(Self::Open),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Closed => "Closed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationStatus {
    Pending,
    Accepted,
    Declined,
}

impl ApplicationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Declined => "declined",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "accepted" => Some(Self::Accepted),
            "declined" => Some(Self::Declined),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "Waiting",
            Self::Accepted => "Accepted",
            Self::Declined => "Declined",
        }
    }
}

impl Need {
    pub fn scope(&self) -> Option<NeedScope> {
        NeedScope::parse(&self.scope)
    }

    pub fn status(&self) -> Option<NeedStatus> {
        NeedStatus::parse(&self.status)
    }

    pub fn is_open(&self) -> bool {
        self.status() == Some(NeedStatus::Open)
    }

    pub fn sight(&self) -> NeedSight<'_> {
        NeedSight {
            church_id: &self.church_id,
            author_id: &self.author_id,
            scope: self.scope(),
            status: &self.status,
        }
    }
}

/// Borrowed fields a visibility or apply rule actually reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeedSight<'a> {
    pub church_id: &'a str,
    pub author_id: &'a str,
    pub scope: Option<NeedScope>,
    pub status: &'a str,
}

impl NeedSight<'_> {
    pub fn is_open(self) -> bool {
        NeedStatus::parse(self.status) == Some(NeedStatus::Open)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct NeedCard {
    pub id: String,
    pub church_id: String,
    pub church_name: String,
    pub church_city: String,
    pub church_region: String,
    pub author_id: String,
    pub author_name: String,
    pub title: String,
    pub body: String,
    pub gift_id: Option<String>,
    pub gift_name: Option<String>,
    pub scope: String,
    pub status: String,
    pub created_at: String,
}

impl NeedCard {
    pub fn scope(&self) -> Option<NeedScope> {
        NeedScope::parse(&self.scope)
    }

    pub fn status(&self) -> Option<NeedStatus> {
        NeedStatus::parse(&self.status)
    }

    pub fn is_open(&self) -> bool {
        self.status() == Some(NeedStatus::Open)
    }

    pub fn sight(&self) -> NeedSight<'_> {
        NeedSight {
            church_id: &self.church_id,
            author_id: &self.author_id,
            scope: self.scope(),
            status: &self.status,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct Application {
    pub id: String,
    pub need_id: String,
    pub user_id: String,
    pub message: String,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow, Serialize)]
pub struct ApplicationCard {
    pub id: String,
    pub need_id: String,
    pub user_id: String,
    pub user_name: String,
    pub message: String,
    pub status: String,
    pub created_at: String,
}

impl Application {
    pub fn status(&self) -> Option<ApplicationStatus> {
        ApplicationStatus::parse(&self.status)
    }
}

impl ApplicationCard {
    pub fn status(&self) -> Option<ApplicationStatus> {
        ApplicationStatus::parse(&self.status)
    }
}

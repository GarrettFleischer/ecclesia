//! Turn a church address into a map point.

use ecclesia_domain::{Place, coordinates};

#[derive(Clone, Copy)]
pub enum PlaceBook {
    Census,
    Fixture,
}

impl PlaceBook {
    pub async fn locate(self, address: &str) -> anyhow::Result<Option<Place>> {
        match self {
            Self::Census => census_place(address).await,
            Self::Fixture => Ok(fixture_place(address)),
        }
    }
}

async fn census_place(address: &str) -> anyhow::Result<Option<Place>> {
    let address = one_line(address);
    if address.is_empty() {
        return Ok(None);
    }
    let response = crate::chat::http_client(8)
        .get("https://geocoding.geo.census.gov/geocoder/locations/onelineaddress")
        .header(reqwest::header::USER_AGENT, "Ecclesia")
        .query(&[
            ("address", address.as_str()),
            ("benchmark", "Public_AR_Current"),
            ("format", "json"),
        ])
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!("census geocoder status {}", response.status());
    }
    let body = response.text().await?;
    Ok(place_from_census(&body))
}

pub fn place_from_census(body: &str) -> Option<Place> {
    let parsed: CensusBody = serde_json::from_str(body).ok()?;
    let point = &parsed.result.address_matches.first()?.coordinates;
    let (latitude, longitude) = coordinates(point.y, point.x).ok()?;
    Some(Place {
        latitude,
        longitude,
    })
}

fn fixture_place(address: &str) -> Option<Place> {
    let lower = address.to_ascii_lowercase();
    let (latitude, longitude) = if lower.contains("austin") {
        (30.2672, -97.7431)
    } else if lower.contains("waterloo") {
        (42.4928, -92.3426)
    } else if lower.contains("cedar falls") || lower.contains("main street") {
        (42.5349, -92.4453)
    } else {
        return None;
    };
    Some(Place {
        latitude,
        longitude,
    })
}

fn one_line(address: &str) -> String {
    let mut out = String::new();
    let mut spaced = false;
    for ch in address.chars() {
        if ch.is_whitespace() {
            spaced = !out.is_empty();
            continue;
        }
        if spaced {
            out.push(' ');
            spaced = false;
        }
        out.push(ch);
    }
    out
}

#[derive(serde::Deserialize)]
struct CensusBody {
    result: CensusResult,
}

#[derive(serde::Deserialize)]
struct CensusResult {
    #[serde(default, rename = "addressMatches")]
    address_matches: Vec<CensusMatch>,
}

#[derive(serde::Deserialize)]
struct CensusMatch {
    coordinates: CensusPoint,
}

#[derive(serde::Deserialize)]
struct CensusPoint {
    x: f64,
    y: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn census_match_uses_y_as_latitude() {
        let body = r#"{"result":{"addressMatches":[{"coordinates":{"x":-92.4453,"y":42.5349}}]}}"#;
        let place = place_from_census(body).expect("place");
        assert!((place.latitude - 42.5349).abs() < 0.0001);
        assert!((place.longitude - -92.4453).abs() < 0.0001);
    }

    #[test]
    fn census_miss_is_empty() {
        let body = r#"{"result":{"addressMatches":[]}}"#;
        assert!(place_from_census(body).is_none());
    }

    #[test]
    fn fixture_places_known_towns() {
        let austin = fixture_place("100 Congress Avenue, Austin, TX 78701").expect("austin");
        assert!((austin.latitude - 30.2672).abs() < 0.0001);
        let waterloo =
            fixture_place("200 Commercial Street, Waterloo, IA 50701").expect("waterloo");
        assert!((waterloo.longitude - -92.3426).abs() < 0.0001);
        assert!(fixture_place("somewhere else").is_none());
    }
}

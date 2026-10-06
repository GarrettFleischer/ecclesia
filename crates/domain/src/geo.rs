//! Distance between church buildings. The SDK copies `nearby_km` into SQL.

const EARTH_KM: f64 = 6371.0;

pub fn nearby_km() -> f64 {
    40.0
}

/// A point the browser shared. Domain does not read a device.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub latitude: f64,
    pub longitude: f64,
}

pub fn place_is_near(latitude: f64, longitude: f64, place: Place) -> bool {
    distance_km(latitude, longitude, place.latitude, place.longitude) <= nearby_km()
}

pub fn distance_km(a_lat: f64, a_lng: f64, b_lat: f64, b_lng: f64) -> f64 {
    let phi1 = a_lat.to_radians();
    let phi2 = b_lat.to_radians();
    let d_phi = (b_lat - a_lat).to_radians();
    let d_lambda = (b_lng - a_lng).to_radians();
    let haversine =
        (d_phi / 2.0).sin().powi(2) + phi1.cos() * phi2.cos() * (d_lambda / 2.0).sin().powi(2);
    let arc = 2.0 * haversine.sqrt().atan2((1.0 - haversine).sqrt());
    EARTH_KM * arc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearer_pair_is_shorter() {
        let cedar_falls = (42.5349, -92.4453);
        let waterloo = (42.4928, -92.3426);
        let austin = (30.2672, -97.7431);
        let near = distance_km(cedar_falls.0, cedar_falls.1, waterloo.0, waterloo.1);
        let far = distance_km(cedar_falls.0, cedar_falls.1, austin.0, austin.1);
        assert!(near < far);
        assert!(near <= nearby_km());
        assert!(far > nearby_km());
    }
}

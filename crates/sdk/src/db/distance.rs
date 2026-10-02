//! One haversine expression for SQL. Domain `distance_km` is what tests assert.

pub(crate) fn haversine_km_sql(lat_a: &str, lng_a: &str, lat_b: &str, lng_b: &str) -> String {
    let rise = format!(
        "sin(radians({lat_b} - {lat_a}) / 2.0) * sin(radians({lat_b} - {lat_a}) / 2.0) + cos(radians({lat_a})) * cos(radians({lat_b})) * sin(radians({lng_b} - {lng_a}) / 2.0) * sin(radians({lng_b} - {lng_a}) / 2.0)"
    );
    format!("(6371.0 * 2.0 * atan2(sqrt({rise}), sqrt(1.0 - ({rise}))))")
}

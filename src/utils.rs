pub fn vec_to_and_string<T: AsRef<str>>(items: Vec<T>, delimiter: &str) -> String {
    let len = items.len();

    match len {
        0 => String::new(),
        1 => items[0].as_ref().to_string(),
        _ => {
            let mut joined = items[..len - 1]
                .iter()
                .map(|s| s.as_ref()) // Convert to &str
                .collect::<Vec<&str>>()
                .join(", ");

            joined.push_str(&format!(" {} {}", delimiter, items[len - 1].as_ref()));
            joined
        }
    }
}

pub fn format_coordinate(value: f64, is_latitude: bool) -> String {
    let direction = match (is_latitude, value >= 0.0) {
        (true, true) => "N",
        (true, false) => "S",
        (false, true) => "E",
        (false, false) => "W",
    };

    let value = value.abs();
    let degrees = value.trunc() as i64;
    let minutes = ((value - degrees as f64) * 60.0).trunc() as i64;
    let seconds = ((value - degrees as f64 - minutes as f64 / 60.0) * 3600.0).round() as i64;

    format!("{}°{}'{}\"{}", degrees, minutes, seconds, direction)
}

pub fn format_coordinates(lat: &str, lon: &str, alt: Option<&str>) -> String {
    let lat_f: f64 = lat.parse().unwrap();
    let lon_f: f64 = lon.parse().unwrap();

    let coords = format!(
        "{}, {}",
        format_coordinate(lat_f, true),
        format_coordinate(lon_f, false)
    );

    if let Some(altitude) = alt {
        let alt_f: f64 = altitude.parse().unwrap();
        format!("{} ({}m)", coords, alt_f.round())
    } else {
        coords
    }
}

use chrono::NaiveDateTime;
use dashmap::DashMap;

pub fn vec_to_and_string<T: AsRef<str>>(items: &Vec<T>, delimiter: &str) -> String {
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

pub fn generate_text<T: AsRef<str>>(
    property_map: &DashMap<String, (String, String, String)>,
    sentences: &mut Vec<String>,
    questions: &mut Vec<String>,
    and_symbol: &str,
    prop_key: &str,
    prop_label: &str,
    prop_value: &Vec<T>,
) {
    let value = vec_to_and_string(prop_value, and_symbol);
    match property_map.get(prop_key) {
        Some(values) => {
            let (_, sentence, question) = &*values;
            let sentence = adjust_article(sentence, &value)
                .replacen("{}", prop_label, 1)
                .replacen("{}", &value, 1);
            let question = question.replace("{}", &prop_label) + &format!(" [{}]", value);
            if !sentence.is_empty() {
                sentences.push(sentence);
            }
            if !question.is_ascii() {
                questions.push(question);
            }
        }
        None => {}
    };
}

// Helper function to format dates nicely
pub fn format_date(date_str: &str, date_format: &str) -> String {
    let cleaned_date = date_str.trim_start_matches('+'); // Remove leading '+'

    match NaiveDateTime::parse_from_str(cleaned_date, "%Y-%m-%dT%H:%M:%SZ") {
        Ok(date) => date.format(date_format).to_string(),
        Err(_) => cleaned_date.replace("-00-00T00:00:00Z", ""), // In case only the year is published
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

/// Capitalize the first letter of a string to a lowercase version.
pub fn lowercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Adjust the English article "a" to "an" if the next word starts with a vowel.
pub fn adjust_article(first: &str, second: &str) -> String {
    if first.ends_with(" a {}.") && second.chars().next().map_or(false, |c| "aeiou".contains(c)) {
        let mut adjusted = first.to_string();
        adjusted.truncate(first.len() - 5); // Remove " a {}."
        adjusted.push_str(" an {}.");
        adjusted
    } else {
        first.to_string()
    }
}

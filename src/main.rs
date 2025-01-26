use chrono::NaiveDateTime;
use rayon::prelude::*;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use memmap2::{Mmap, MmapOptions};

mod batched_writer;
use batched_writer::BatchedWriter;
mod entity_resolver;
use entity_resolver::EntityResolver;
mod processing_error;
use processing_error::ProcessingError;
mod config;
use config::{get_configuration, Config};

#[derive(Debug, Deserialize)]
struct WikidataEntity {
    id: String,
    claims: Option<Map<String, Value>>,
    labels: Option<Map<String, Value>>,
    descriptions: Option<Map<String, Value>>,
    aliases: Option<Map<String, Value>>,
    // #[serde(default)]
    // sitelinks: Value,
}

fn get_entity_type_mappings() -> HashMap<&'static str, Vec<&'static str>> {
    HashMap::from([
        // human: https://www.wikidata.org/wiki/Q5
        ("person", vec!["Q5"]),
        // ("organization", vec!["Q43229"]),
        // ("scientific_organization", vec!["Q16519632"]),
        // ("research_institute", vec!["Q31855"]),
        // ("government_agency", vec!["Q327333"]),
        // ("event", vec!["Q1656682"]),
        // (
        //     "mood",
        //     vec![
        //         "Q331769",   // mood
        //         "Q41537118", // emotional state
        //         "Q3968640",  // mental state
        //         "Q16748867", // basic emotion
        //         "Q9415",     // emotions
        //         "Q9332",     // behavior
        //         "Q60539479", // positive emotion
        //         "Q60539481", // negative emotion
        //     ],
        // ),
    ])
}

fn get_default_properties() -> HashMap<&'static str, Vec<&'static str>> {
    // let organization_props = vec![
    //     "P31",   // Instance of
    //     "P17",   // Country
    //     "P112",  // Founder
    //     "P571",  // Inception date
    //     "P1813", // Short name
    //     "P18",   // Image
    //     "P154",  // Logo
    //     "P159",  // Headquarters locations
    //     "P856",  // Website
    //     "P749",  // Parent organisation
    //     "P1454", // Legal form
    //     "P3220", // KvK company ID
    //     "P452",  // industry
    //     "P101",  // field of work
    // ];
    HashMap::from([
        (
            // Person-related properties
            "person",
            vec![
                "P569", // Date of birth, https://www.wikidata.org/wiki/Property:P569
                "P570", // Date of death, https://www.wikidata.org/wiki/Property:P570
                "P27",  // Country of citizenship
                "P106", // Occupation
                // "P18",   // Image
                "P39",   // Position held
                "P1449", // Nickname
                "P101",  // field of work
            ],
        ),
        // ("organization", organization_props.clone()),
        // ("scientific_organization", organization_props.clone()),
        // ("research_institute", organization_props.clone()),
        // ("government_agency", organization_props.clone()),
        // (
        //     // Event-related properties
        //     "event",
        //     vec![
        //         "P585", // Point in time
        //         "P17",  // Country
        //         "P276", // Location
        //         "P31",  // Instance of
        //         "P18",  // Image
        //     ],
        // ),
        // (
        //     "mood",
        //     vec![
        //         "P31",   // Instance of
        //         "P1552", // Has characteristic
        //         "P1889", // Different from
        //         "P461",  // Opposite of
        //         "P460",  // Said to be the same as
        //         "P1382", // Partially coincident with
        //         "P18",   // Image
        //     ],
        // ),
    ])
}

// Extracted progress printing for reusability
fn print_progress(start_time: Instant, current_promille: u64, file_size: u64) {
    let elapsed = start_time.elapsed();
    let eta = if current_promille > 0 {
        let total_estimated_time = elapsed.as_secs_f64() / (current_promille as f64 / 1000.0);
        Duration::from_secs_f64(total_estimated_time - elapsed.as_secs_f64())
    } else {
        Duration::from_secs(0)
    };

    print!(
        "\rProcessing: {:.1}% | Elapsed: {:.0}s | ETA: {:.0}s         ",
        current_promille as f64 / 10.0,
        elapsed.as_secs(),
        eta.as_secs()
    );
    std::io::stdout().flush().ok();
}

// fn prefill_cache_old(
//     input_path: &String,
//     config: &Config,
// ) -> Result<EntityResolver, ProcessingError> {
//     // Create resolver with a specific cache file path
//     let resolver = EntityResolver::new(
//         PathBuf::from(format!(
//             "{}/{}/entity_cache.csv",
//             config.output_dir, config.lang,
//         )),
//         "https://www.wikidata.org/w/api.php".to_string(),
//         &config.lang,
//         &config.recreate_cache,
//     );

//     // Open input file and get total file size for progress tracking
//     let file = File::open(input_path).expect("JSON dump file not found");
//     let file_size = file.metadata()?.len();
//     let reader = BufReader::new(file);

//     // Progress tracking
//     let start_time = Instant::now();
//     let total_processed = AtomicU64::new(0);
//     let last_reported_promille = AtomicU64::new(0);

//     reader
//         .lines()
//         .par_bridge()
//         .try_for_each(|line_result| -> Result<(), ProcessingError> {
//             // Read line with thread-safe progress tracking
//             let line = match line_result {
//                 Ok(line) => line,
//                 Err(e) => return Err(ProcessingError::IoError(e)),
//             };

//             // Skip empty or array marker lines
//             if line.trim().is_empty() || line.starts_with('[') || line.starts_with(']') {
//                 return Ok(());
//             }

//             // Update progress using atomic operations
//             let line_len = line.len() as u64;
//             let current_total = total_processed.fetch_add(line_len, Ordering::Relaxed) + line_len; // it returns the previous value, so add line_len
//             let current_promille = ((current_total as f64 / file_size as f64) * 1000.0) as u64;

//             // Report progress with 0.1% granularity
//             let last_promille = last_reported_promille.load(Ordering::Relaxed);
//             if (current_promille - last_promille) >= 1 {
//                 // Use compare_exchange to ensure only one thread updates the progress
//                 if last_reported_promille
//                     .compare_exchange(
//                         last_promille,
//                         current_promille,
//                         Ordering::SeqCst,
//                         Ordering::Relaxed,
//                     )
//                     .is_ok()
//                 {
//                     let elapsed = start_time.elapsed();
//                     let eta = if current_promille > 0 {
//                         let total_estimated_time =
//                             elapsed.as_secs_f64() / (current_promille as f64 / 1000.0);
//                         Duration::from_secs_f64(total_estimated_time - elapsed.as_secs_f64())
//                     } else {
//                         Duration::from_secs(0)
//                     };

//                     print!(
//                         "\rProcessing: {:.1}% | Elapsed: {:.0}s | ETA: {:.0}s         ",
//                         current_promille as f64 / 10.0,
//                         elapsed.as_secs(),
//                         eta.as_secs()
//                     );
//                     std::io::stdout().flush()?;
//                 }
//             }

//             // Remove trailing comma if present
//             let json_str = line.trim_end_matches(',');

//             // Parse entity
//             let entity: WikidataEntity = match serde_json::from_str(json_str) {
//                 Ok(e) => e,
//                 Err(_) => return Ok(()),
//             };
//             // println!("{:?}", entity);
//             if let Some(labels) = entity.labels {
//                 if let Some(label_obj) = labels.get(&config.lang) {
//                     if let Some(label) = label_obj.get("value").and_then(|v| v.as_str()) {
//                         // println!("{}, {}", entity.id, &label);
//                         resolver.add_entry((entity.id.clone(), label.to_string()));
//                     }
//                 }
//             }

//             Ok(())
//         })?;

//     // Final flush of any remaining entries
//     resolver.save_cache_to_disk();

//     // Clear progress line
//     println!(
//         "\rProcessing: 100% | Completed in {:.0}s                 ",
//         start_time.elapsed().as_secs()
//     );

//     Ok(resolver)
// }

fn prefill_cache(
    input_path: &String,
    config: &Config,
) -> Result<DashMap<String, String>, ProcessingError> {
    let output_file = PathBuf::from(format!(
        "{}/{}/entity_cache.csv",
        config.output_dir, config.lang,
    ));
    println!("Output file: {:?}", output_file);

    let entity_map = DashMap::new();

    // Open input file and get total file size for progress tracking
    let file = File::open(input_path).expect("JSON dump file not found");
    let file_size = file.metadata()?.len();
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let reader = BufReader::new(mmap.as_ref());
    // let reader = BufReader::new(file);

    // Progress tracking
    let start_time = Instant::now();
    let total_processed = AtomicU64::new(0);
    let last_reported_promille = AtomicU64::new(0);

    // Parallel processing with better error handling
    reader
        .lines()
        .par_bridge()
        .try_for_each(|line_result| -> Result<(), ProcessingError> {
            let line: String = match line_result {
                Ok(line) => line,
                Err(e) => return Err(ProcessingError::IoError(e)),
            };

            if line.trim().is_empty() || line.starts_with(['[', ']']) {
                return Ok(());
            }

            let line_len = line.len() as u64;
            let current_total = total_processed.fetch_add(line_len, Ordering::Relaxed) + line_len;
            let current_promille = ((current_total as f64 / file_size as f64) * 1000.0) as u64;

            if (current_promille - last_reported_promille.load(Ordering::Relaxed)) >= 1 {
                last_reported_promille.store(current_promille, Ordering::Relaxed);
                print_progress(start_time, current_promille, file_size);
            }

            let json_str = line.trim_end_matches(',');
            let entity: WikidataEntity = match serde_json::from_str(json_str) {
                Ok(e) => e,
                Err(_) => return Ok(()),
            };

            if let Some(label) = entity
                .labels
                .and_then(|labels| labels.get(&config.lang).cloned())
                .and_then(|label_obj| label_obj.get("value")?.as_str().map(|s| s.to_string()))
            {
                // println!("{}, {}", entity.id, &label);
                entity_map.insert(entity.id, label);
            }

            Ok(())
        })?;

    // Clear progress line
    println!(
        "\rProcessing: 100% | Completed in {:.0}s                 ",
        start_time.elapsed().as_secs()
    );

    // Write resolver to CSV at the end
    let mut writer = csv::Writer::from_path(output_file)?;
    for entry in entity_map.iter() {
        writer.write_record(&[entry.key(), entry.value()])?;
    }
    writer.flush()?;

    Ok(entity_map)
}

fn process_wikidata(
    input_path: &String,
    config: &Config,
    resolver: EntityResolver,
) -> Result<(), ProcessingError> {
    let entity_mappings = get_entity_type_mappings();
    let default_properties = get_default_properties();

    // Initialize CSV writers
    let mut csv_writers: HashMap<String, csv::Writer<File>> = HashMap::new();
    for entity_type in &config.entity_types {
        let csv_path = format!("{}/{}/{}.csv", config.output_dir, config.lang, entity_type);
        csv_writers.insert(entity_type.clone(), csv::Writer::from_path(csv_path)?);
    }

    // Create a batched writer
    let batched_writer = BatchedWriter::new(csv_writers, 100);
    let batched_writer = Arc::new(Mutex::new(batched_writer));

    // Open input file and get total file size for progress tracking
    let file = File::open(input_path).expect("JSON dump file not found");
    let file_size = file.metadata()?.len();
    let reader = BufReader::new(file);

    // Progress tracking
    let start_time = Instant::now();
    let total_processed = AtomicU64::new(0);
    let last_reported_promille = AtomicU64::new(0);

    // Process file in parallel
    reader
        .lines()
        .par_bridge()
        .try_for_each(|line_result| -> Result<(), ProcessingError> {
            // Read line with thread-safe progress tracking
            let line = match line_result {
                Ok(line) => line,
                Err(e) => return Err(ProcessingError::IoError(e)),
            };

            // Skip empty or array marker lines
            if line.trim().is_empty() || line.starts_with('[') || line.starts_with(']') {
                return Ok(());
            }

            // Update progress using atomic operations
            let line_len = line.len() as u64;
            let current_total = total_processed.fetch_add(line_len, Ordering::Relaxed) + line_len; // it returns the previous value, so add line_len
            let current_promille = ((current_total as f64 / file_size as f64) * 1000.0) as u64;

            // Report progress with 0.1% granularity
            let last_promille = last_reported_promille.load(Ordering::Relaxed);
            if (current_promille - last_promille) >= 1 {
                // Use compare_exchange to ensure only one thread updates the progress
                if last_reported_promille
                    .compare_exchange(
                        last_promille,
                        current_promille,
                        Ordering::SeqCst,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    let elapsed = start_time.elapsed();
                    let eta = if current_promille > 0 {
                        let total_estimated_time =
                            elapsed.as_secs_f64() / (current_promille as f64 / 1000.0);
                        Duration::from_secs_f64(total_estimated_time - elapsed.as_secs_f64())
                    } else {
                        Duration::from_secs(0)
                    };

                    print!(
                        "\rProcessing: {:.1}% | Elapsed: {:.0}s | ETA: {:.0}s         ",
                        current_promille as f64 / 10.0,
                        elapsed.as_secs(),
                        eta.as_secs()
                    );
                    std::io::stdout().flush()?;
                }
            }

            // Remove trailing comma if present
            let json_str = line.trim_end_matches(',');

            // Parse entity
            let entity: WikidataEntity = match serde_json::from_str(json_str) {
                Ok(e) => e,
                Err(_) => return Ok(()),
            };
            // if let Some(title) = entity.sitelinks["enwiki"]["title"].as_str() {
            //     dbg!(title);
            // }

            // Process entity
            if let (Some(claims), Some(labels), Some(descriptions), Some(aliases)) = (
                entity.claims,
                entity.labels,
                entity.descriptions,
                entity.aliases,
            ) {
                if let Some(label_obj) = labels.get(&config.lang) {
                    if let Some(label) = label_obj.get("value").and_then(|v| v.as_str()) {
                        let description = descriptions
                            .get(&config.lang)
                            // .or(descriptions.get("en"))
                            .and_then(|obj| obj.get("value"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let aliases = aliases
                            .get(&config.lang)
                            .and_then(|value| value.as_array())
                            .and_then(|values| {
                                // dbg!(&values);
                                Some(
                                    values
                                        .iter()
                                        .map(|v| {
                                            v.get("value").and_then(|v| v.as_str()).unwrap_or("")
                                        })
                                        .filter(|alias| *alias != label)
                                        .collect::<Vec<&str>>(),
                                )
                            })
                            .unwrap_or(Vec::new());

                        for entity_type in &config.entity_types {
                            if let Some(instance_of) = entity_mappings.get(entity_type.as_str()) {
                                if claims.get("P31").and_then(|p31| p31.as_array()).map_or(
                                    false,
                                    |instances| {
                                        instances.iter().any(|i| {
                                            if let Some(instance) =
                                                i["mainsnak"]["datavalue"]["value"]["id"].as_str()
                                            {
                                                instance_of.contains(&instance)
                                            } else {
                                                false
                                            }
                                        })
                                    },
                                ) {
                                    let (used_names, kv_entry) = prepare_data_export(
                                        &resolver,
                                        entity_type,
                                        &entity.id,
                                        &claims,
                                        &default_properties,
                                        label,
                                        &aliases,
                                        description,
                                    );

                                    // Batch the writes
                                    let mut writer = batched_writer.lock().unwrap();
                                    write_entity_data(
                                        &mut writer,
                                        entity_type,
                                        used_names,
                                        kv_entry,
                                    )?;
                                }
                            }
                        }
                    }
                }
            }

            Ok(())
        })?;

    // Final flush of any remaining entries
    batched_writer.lock().unwrap().finalize()?;

    // Clear progress line
    println!(
        "\rProcessing: 100% | Completed in {:.0}s                 ",
        start_time.elapsed().as_secs()
    );

    Ok(())
}

/// Prepare the data for export
fn prepare_data_export(
    resolver: &EntityResolver,
    entity_type: &String,
    entity_id: &str,
    claims: &Map<String, Value>,
    default_properties: &HashMap<&str, Vec<&str>>,
    label: &str,
    aliases: &Vec<&str>,
    description: &str,
) -> (Vec<String>, Value) {
    let properties = &resolver.resolve_entity_ids(extract_properties(
        entity_type,
        &Value::Object(claims.clone()),
        default_properties,
    ));

    let mut used_names = HashSet::with_capacity(6);
    let mut ordered_names = vec![];
    used_names.insert(label);

    for key in ["P1813" /* Short name */, "P1449" /* Nickname */] {
        if let Some(alt_name_val) = properties.get(key) {
            if let Some(alt_name) = alt_name_val.as_str() {
                if used_names.insert(alt_name) {
                    ordered_names.push(alt_name.to_string());
                }
            }
        }
    }

    for alias in aliases {
        if used_names.insert(alias) {
            ordered_names.push(alias.to_string());
        }
    }

    let mut entity_data = serde_json::Map::new();
    entity_data.insert("label".to_string(), json!(label));

    // Conditionally add description if not empty
    if !description.is_empty() {
        entity_data.insert("descr".to_string(), json!(description));
    }

    // Conditionally add aliases if not empty
    if !aliases.is_empty() {
        entity_data.insert("alias".to_string(), json!(aliases));
    }

    // Always add properties
    if properties.len() > 0 {
        entity_data.insert("props".to_string(), json!(properties));
    }

    let kv_entry = json!({
        entity_id: entity_data
    });
    (ordered_names, kv_entry)
}

// Helper function to format dates nicely
fn format_date(date_str: &str) -> String {
    if let Ok(date) = NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%SZ") {
        date.format("%B %d, %Y").to_string()
    } else {
        date_str.to_string()
    }
}

fn write_entity_data(
    batched_writer: &mut BatchedWriter,
    entity_type: &str,
    used_names: Vec<String>,
    kv_entry: Value,
) -> Result<(), ProcessingError> {
    if let Value::Object(outer) = kv_entry {
        let (_, person_data) = match outer.iter().next() {
            Some(entry) => entry,
            None => return Ok(()),
        };

        if let Value::Object(data) = person_data {
            let name = data
                .get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Handle aliases
            for used_name in used_names {
                // println!("{}: {}", entity_type, used_name);
                batched_writer.add_csv_entry(
                    entity_type.to_string(),
                    format!("{} is also known as {}.", name, used_name),
                )?;
            }

            // Handle description
            if let Some(Value::String(description)) = data.get("descr") {
                batched_writer.add_csv_entry(
                    entity_type.to_string(),
                    format!("{} is a {}.", name, description.trim_matches('"')),
                )?;
            }

            // Handle properties
            if let Some(Value::Object(props)) = data.get("props") {
                if let Some(Value::String(value)) = props.get("P101") {
                    batched_writer.add_csv_entry(
                        entity_type.to_string(),
                        format!("{} works in the area of {}.", name, value),
                    )?;
                }

                if let Some(Value::String(value)) = props.get("P106") {
                    batched_writer.add_csv_entry(
                        entity_type.to_string(),
                        format!("{} is occupied as {}.", name, value),
                    )?;
                }

                if let Some(Value::String(value)) = props.get("P27") {
                    batched_writer.add_csv_entry(
                        entity_type.to_string(),
                        format!("{} is citizen of {}.", name, value),
                    )?;
                }

                if let Some(Value::String(value)) = props.get("P569") {
                    batched_writer.add_csv_entry(
                        entity_type.to_string(),
                        format!("{} was born on {}.", name, format_date(value)),
                    )?;
                }

                if let Some(Value::String(value)) = props.get("P570") {
                    batched_writer.add_csv_entry(
                        entity_type.to_string(),
                        format!("{} died on {}.", name, format_date(value)),
                    )?;
                }
            }

            batched_writer.add_kv_entry()?;
        }
    }

    Ok(())
}

fn extract_properties(
    entity_type: &str,
    claims: &Value,
    default_properties: &HashMap<&str, Vec<&str>>,
) -> Map<String, Value> {
    let mut properties = serde_json::Map::new();

    match default_properties.get(entity_type) {
        Some(all_properties) => {
            for prop in all_properties {
                if let Some(value) = claims
                    .get(prop)
                    .and_then(|p| p.as_array())
                    .and_then(|array| array.get(0))
                {
                    match *prop {
                        "P569" | "P570" | "P571" => {
                            // Simplify date fields (e.g., P569 = Date of Birth, P570 = Date of Death)
                            if let Some(date) = value
                                .get("mainsnak")
                                .and_then(|ms| ms.get("datavalue"))
                                .and_then(|dv| dv.get("value"))
                                .and_then(|v| v.get("time"))
                            {
                                // Strip precision and metadata, and format date
                                let simple_date =
                                    date.as_str().unwrap_or("").trim_start_matches('+');
                                properties.insert(
                                    prop.to_string(),
                                    Value::String(simple_date.to_string()),
                                );
                            }
                        }
                        "P17" | "P112" | "P27" | "P106" | "P39" | "P1454" | "P749" | "P101"
                        | "P452" | "P276" | "P31" | "P585" | "P1552" | "P1889" | "P461"
                        | "P460" | "P1382" => {
                            // Handle string or entity-id properties (e.g., country, occupation, position)
                            if let Some(id_value) = value
                                .get("mainsnak")
                                .and_then(|ms| ms.get("datavalue"))
                                .and_then(|dv| dv.get("value"))
                                .and_then(|v| v.get("id"))
                            {
                                properties.insert(prop.to_string(), id_value.clone());
                            }
                        }
                        "P159" => {
                            // Extract location address or entity
                            if let Some(location) = value
                                .get("mainsnak")
                                .and_then(|ms| ms.get("datavalue"))
                                .and_then(|dv| dv.get("value"))
                            {
                                properties.insert(prop.to_string(), location.clone());
                            }
                        }
                        "P1813" | "P1449" => {
                            // Extract short name or alias
                            if let Some(short_name) = value
                                .get("mainsnak")
                                .and_then(|ms| ms.get("datavalue"))
                                .and_then(|dv| dv.get("value"))
                                .and_then(|v| v.get("text"))
                            {
                                properties.insert(
                                    prop.to_string(),
                                    Value::String(short_name.as_str().unwrap_or("").to_string()),
                                );
                            }
                        }
                        "P856" | "P3220" => {
                            // Extract URLs (P856 = Official website, P3220 = Google Maps ID)
                            if let Some(url) = value
                                .get("mainsnak")
                                .and_then(|ms| ms.get("datavalue"))
                                .and_then(|dv| dv.get("value"))
                                .and_then(|v| v.as_str())
                            {
                                properties.insert(prop.to_string(), Value::String(url.to_string()));
                            }
                        }
                        _ => {
                            properties.insert(prop.to_string(), value.clone());
                        }
                    }
                }
            }
        }
        None => {}
    }
    properties
}

// fn main() -> Result<(), ProcessingError> {
fn main() -> Result<(), ProcessingError> {
    let (input_file, config) = get_configuration()?;

    let resolver = match prefill_cache(&input_file, &config) {
        Ok(r) => r,
        Err(e) => return Err(e),
    };
    // let resolver = match prefill_cache(&input_file, &config) {
    //     Ok(r) => r,
    //     Err(e) => return Err(e),
    // };
    // process_wikidata(&input_file, &config, resolver)
    Ok(())
}

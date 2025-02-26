use rayon::iter::{ParallelBridge, ParallelIterator};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use utils::{format_coordinates, format_date, generate_text, lowercase_first};

use dashmap::DashMap;
use memmap2::MmapOptions;

mod csv_writer_pool;
use csv_writer_pool::CsvWriterPool;

mod utils;

mod processing_error;
use processing_error::ProcessingError;
mod config;
use config::{get_configuration, Config};

#[derive(Deserialize, Debug)]
struct Sitelink {
    // title: String,               // The title of the page on the specific site
    // badges: Option<Vec<String>>, // Optional badges associated with the link
    // url: Option<String>,         // Optional URL of the page
}

#[derive(Debug, Deserialize)]
struct WikidataEntity {
    id: String,
    claims: Option<Map<String, Value>>,
    labels: Option<Map<String, Value>>,
    descriptions: Option<Map<String, Value>>,
    aliases: Option<Map<String, Value>>,
    // #[serde(default)]
    sitelinks: Option<HashMap<String, Sitelink>>,
}

// Extracted progress printing for reusability
fn print_progress(start_time: Instant, current_promille: u64) {
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

fn prefill_cache(
    input_path: &String,
    config: &Config,
) -> Result<DashMap<u32, String>, ProcessingError> {
    let output_file = PathBuf::from(format!(
        "{}/{}/entity_cache.csv",
        config.output_dir, config.lang,
    ));
    println!("Output file: {:?}", output_file);

    let entity_map = DashMap::new();

    if !config.recreate_cache && Path::new(&output_file).exists() {
        // TODO Remove
        // return Ok(entity_map);
        // Load cache from disk
        let mut reader = csv::Reader::from_path(output_file)?;
        for result in reader.records() {
            let record = result?;
            if !record[0].starts_with("Q") {
                continue;
            }
            match record[0].replace("Q", "").parse::<u32>() {
                Ok(key) => {
                    entity_map.insert(key, record[1].to_string());
                }
                Err(_) => {
                    println!("Failed to parse key: {:?}", record);
                    continue;
                }
            }
        }
        entity_map.shrink_to_fit();
        return Ok(entity_map);
    }

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
                print_progress(start_time, current_promille);
            }

            let json_str = line.trim_end_matches(',');
            let entity: WikidataEntity = match serde_json::from_str(json_str) {
                Ok(e) => e,
                Err(_) => return Ok(()),
            };
            if !entity.id.starts_with('Q') {
                // println!("{:?}", entity);
                return Ok(());
            }

            if let Some(label) = entity
                .labels
                .and_then(|labels| labels.get(&config.lang).cloned())
                .and_then(|label_obj| label_obj.get("value")?.as_str().map(|s| s.to_string()))
            {
                // println!("{}, {}", entity.id, &label);
                let key = entity.id.replace("Q", "").parse::<u32>().unwrap();
                entity_map.insert(key, label);
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
        writer.write_record(&[&format!("Q{}", entry.key()), entry.value()])?;
    }
    writer.flush()?;

    entity_map.shrink_to_fit();
    Ok(entity_map)
}

// WikiProperties record
#[derive(Debug, Deserialize)]
struct WikiProperties {
    key: String,
    value: String,
    sentence: String,
    question: String,
}

fn process_wikidata(
    input_path: &String,
    config: &Config,
    resolver: DashMap<u32, String>,
) -> Result<(), ProcessingError> {
    // Additional, manually added property keys
    const DESCRIPTIONS: &str = "descriptions";
    const PERSON_DESCRIPTIONS: &str = "person_descriptions";
    const ALIASES: &str = "aliases";
    const PERSON_ALIASES: &str = "person_aliases";
    // const MALE_PERSON: &str = "male_person";
    // const FEMALE_PERSON: &str = "female_person";
    const LIST: &str = "list";
    const DATE_FORMAT: &str = "date_format";
    // const MISSING_DATE: &str = "missing_date";

    let properties_file = PathBuf::from(format!("./data/wikidata-{}-properties.csv", config.lang,));
    println!(
        "Properties and input file: {:?}, {}",
        properties_file, input_path
    );

    let csv_writers = CsvWriterPool::new(&format!("{}/{}", config.output_dir, config.lang));

    let property_map = DashMap::new();

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .has_headers(true)
        .from_reader(File::open(properties_file)?);
    for result in reader.deserialize() {
        // println!("Record: {:?}", result);
        let record: WikiProperties = result?;
        property_map.insert(
            record.key.to_string(),
            (
                record.value.to_string(),    // value
                record.sentence.to_string(), // sentence
                record.question.to_string(), // question
            ),
        );
    }

    let and_symbol = property_map
        .get(LIST)
        .map_or("and".to_string(), |entry| entry.value().1.clone());
    let date_format = property_map
        .get(DATE_FORMAT)
        .map_or("%Y-%m-%d".to_string(), |entry| entry.value().1.clone());
    // let missing_date = property_map
    //     .get(MISSING_DATE)
    //     .map_or("missing date".to_string(), |entry| entry.value().1.clone());

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
                print_progress(start_time, current_promille);
            }

            let json_str = line.trim_end_matches(',');
            let entity: WikidataEntity = match serde_json::from_str(json_str) {
                Ok(e) => e,
                Err(_) => return Ok(()),
            };

            // Process entity

            if let (
                Some(claims),
                Some(labels),
                Some(descriptions),
                Some(aliases),
                // Some(sitelinks),
            ) = (
                entity.claims,
                entity.labels,
                entity.descriptions,
                entity.aliases,
                // entity.sitelinks,
            ) {
                if let Some(label_obj) = labels.get(&config.lang) {
                    if let Some(label) = label_obj.get("value").and_then(|v| v.as_str()) {
                        let mut sentences: Vec<String> = Vec::new();
                        let mut questions: Vec<String> = Vec::new();

                        let instance_of = claims.get("P31").and_then(|p31| p31.as_array()).map_or(
                            Vec::new(),
                            |instances| {
                                instances
                                    .iter()
                                    .filter_map(|i| {
                                        i["mainsnak"]["datavalue"]["value"]["numeric-id"]
                                            .as_number()
                                            .map(|instance| instance.as_u64().unwrap_or(0))
                                    })
                                    .collect()
                            },
                        );

                        // println!(
                        //     "{}: {}, desc: {}, instance_of {:?}, claims: {:?}\n\n",
                        //     entity.id, label, description, instance_of, claims,
                        // );

                        // Q5 Human, Q15632617 Fictional Human
                        let (male, female) = if instance_of.contains(&5)
                            || instance_of.contains(&15632617)
                        {
                            let is_famous =
                                entity.sitelinks.is_some() && !entity.sitelinks.unwrap().is_empty();
                            if !is_famous {
                                return Ok(()); // Skip non-famous humans.
                            }
                            //.contains_key("enwiki") {
                            // Famous human, having at least one wikipage in his/her name.
                            let gender = claims.get("P21").and_then(|p21| p21.as_array()).map_or(
                                Vec::new(),
                                |genders| {
                                    genders
                                        .iter()
                                        .filter_map(|i| {
                                            i["mainsnak"]["datavalue"]["value"]["id"]
                                                .as_str()
                                                .map(|gender| gender.to_string())
                                        })
                                        .collect()
                                },
                            );
                            let male = gender.contains(&"Q6581097".to_string());
                            let female = gender.contains(&"Q6581072".to_string());

                            // println!("Entity: {}", entity.id);
                            // println!("Label: {}", label);
                            // println!("Desc: {}\n\n", description);
                            // println!("Has English Wiki: {}", sitelinks.get("enwiki").);
                            // println!("{:?}", sitelinks);
                            // println!(
                            //     "{}: {}, instance_of {:?}, part_of {:?}, sitelinks: {:?}",
                            //     label, description, instance_of, part_of, sitelinks
                            // )
                            (Some(male), Some(female))
                        } else {
                            (None, None)
                        };
                        let is_human = male.is_some() || female.is_some();

                        let description = lowercase_first(descriptions
                            .get(&config.lang)
                            .and_then(|obj| obj.get("value"))
                            .and_then(|v| v.as_str())
                            .unwrap_or(""));

                        if !description.is_empty() {
                            let prop_key = if is_human { PERSON_DESCRIPTIONS } else { DESCRIPTIONS };
                            generate_text(&property_map, &mut sentences, &mut questions, &and_symbol, prop_key, label, &vec![&description]);
                        }

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
                        // println!("{}: {}", label, alias_str);
                        if !aliases.is_empty() {
                            let prop_key = if is_human { PERSON_ALIASES } else { ALIASES };
                            generate_text(&property_map, &mut sentences, &mut questions, &and_symbol, prop_key, label, &aliases);
                        }

                        // Process all claims
                        for (prop_key, value) in &claims {
                            let prop_value = value.as_array().map_or(Vec::new(), |instances| {
                                instances
                                    .iter()
                                    .filter_map(|i| {
                                        let media_type =
                                            i["mainsnak"]["datavalue"]["type"].as_str();

                                        match media_type.map(|instance| instance) {
                                            Some("wikibase-entityid") => {
                                                match i["mainsnak"]["datavalue"]["value"]
                                                    ["numeric-id"]
                                                    .as_number()
                                                    .map(|instance| instance.as_u64().unwrap_or(0))
                                                {
                                                    Some(id) => {
                                                        resolver.get(&(id as u32)).map(|r| r.clone())
                                                    }
                                                    // _ => Some(i["mainsnak"]["datavalue"]["value"]["numeric-id"].as_number().to_string()),
                                                    _ => None,
                                                }
                                            }
                                            Some("external-id") => i["mainsnak"]["datavalue"]
                                                ["value"]
                                                .as_str()
                                                .map(|instance| instance.to_string()),
                                            Some("globecoordinate") => {
                                                let lat = i["mainsnak"]["datavalue"]["value"]
                                                    ["latitude"]
                                                    .as_number()
                                                    .map(|instance| instance.to_string());
                                                let lon = i["mainsnak"]["datavalue"]["value"]
                                                    ["longitude"]
                                                    .as_number()
                                                    .map(|instance| instance.to_string());
                                                let alt = i["mainsnak"]["datavalue"]["value"]
                                                    ["altitude"]
                                                    .as_number()
                                                    .map(|instance| instance.to_string());
                                                // println!("{}: {:?}, {:?}, {:?}", label, i["mainsnak"]["datavalue"]["value"], lat, lon);
                                                if lat.is_some() && lon.is_some() {
                                                    Some(format_coordinates(
                                                        lat.unwrap().as_str(),
                                                        lon.unwrap().as_str(),
                                                        alt.as_deref(),
                                                    ))
                                                } else {
                                                    None
                                                }
                                            }
                                            Some("quantity") => {
                                                let amount = i["mainsnak"]["datavalue"]["value"]
                                                    ["amount"]
                                                    .as_str()
                                                    .map(|instance| instance.to_string());
                                                let unit = i["mainsnak"]["datavalue"]["value"]
                                                    ["unit"]
                                                    .as_str()
                                                    .map(|instance| instance.to_string());
                                                if let (Some(amount), Some(unit)) = (amount, unit) {
                                                    match unit.as_str() {
                                                        "1" => Some(amount),
                                                        _ if unit.starts_with(
                                                            "http://www.wikidata.org/entity/Q",
                                                        ) =>
                                                        {
                                                            match unit.replace(
                                                                "http://www.wikidata.org/entity/Q",
                                                                "",
                                                            ).parse::<u32>() {
                                                                Ok(id) => {
                                                                    let x = resolver.get(&id).map(|r| r.clone());
                                                                    let result = Some(format!(
                                                                        "{} {}",
                                                                        amount,
                                                                        if x.is_some() {
                                                                            x.unwrap()
                                                                        } else {
                                                                            unit
                                                                        }
                                                                    ));
                                                                    // println!("{}: {:?} - {:?}", label, id, result);
                                                                    result
                                                                }
                                                                _ => None
                                                            }
                                                        }
                                                        _ => None,
                                                    }
                                                } else {
                                                    None
                                                }
                                            }
                                            Some("monolingualtext") => i["mainsnak"]["datavalue"]
                                                ["value"]["text"]
                                                .as_str()
                                                .map(|instance| instance.to_string()),
                                            Some("time") => {
                                                let time = i["mainsnak"]["datavalue"]["value"]
                                                    ["time"]
                                                    .as_str()
                                                    .map(|instance| instance.to_string());
                                                if time.is_some() {
                                                    Some(format_date(
                                                        time.unwrap().as_str(),
                                                        &date_format,
                                                    ))
                                                } else {
                                                    None
                                                    // Some(missing_date.clone())
                                                }
                                            }
                                            Some("string") => i["mainsnak"]["datavalue"]["value"]
                                                .as_str()
                                                .map(|instance| instance.to_string()),
                                            Some("commonsMedia") =>  i["mainsnak"]["datavalue"]["value"]
                                                .as_str()
                                                .map(|instance| instance.to_string()),
                                            None => None,
                                            _ => i["mainsnak"]["datavalue"]["value"]
                                                .as_str()
                                                .map(|instance| instance.to_string()),
                                        }
                                    })
                                    .collect()
                            });
                            if !prop_value.is_empty() {
                                generate_text(&property_map, &mut sentences, &mut questions, &and_symbol, prop_key, label, &prop_value);
                            }
                        }

                        let category = instance_of
                            .first()
                            .map(|key| *key as u32)
                            .and_then(|key| resolver.get(&key).map(|r| r.value().clone())) // Ensure ownership
                            .unwrap_or_else(|| "misc".to_string()); // Ensure the fallback is owned
                        // println!("{}: {}", label, category);
                        // println!("{}: Sentences: {}", label, sentences.join("\n"));
                        // println!("{}: Questions: {}", label, questions.join("\n"));
                        // println!("{}: {:?}", category, instance_of);
                        if !sentences.is_empty() || !questions.is_empty() {
                            csv_writers.write(
                                &category,
                                &[label, &sentences.join("\n"), &questions.join("\n")],
                            );
                        }
                    }
                }
            }

            Ok(())
        })?;

    csv_writers.flush_all();

    // Clear progress line
    println!(
        "\rProcessing: 100% | Completed in {:.0}s                 ",
        start_time.elapsed().as_secs()
    );

    Ok(())
}

// fn main() -> Result<(), ProcessingError> {
fn main() -> Result<(), ProcessingError> {
    let (input_file, config) = get_configuration()?;

    let resolver = match prefill_cache(&input_file, &config) {
        Ok(r) => r,
        Err(e) => return Err(e),
    };
    process_wikidata(&input_file, &config, resolver)
    // Ok(())
}

use csv::{Writer, WriterBuilder};
use dashmap::DashMap;
use regex::Regex;
use std::fs::{create_dir_all, File};
use std::io::BufWriter;
use std::sync::{Arc, RwLock};

/// Thread-safe CSV writer manager
pub struct CsvWriterPool {
    writers: DashMap<String, Arc<RwLock<Writer<BufWriter<File>>>>>,
    output_dir: String,
}

fn sanitize_filename(category: &str) -> String {
    let re = Regex::new(r"[^a-z0-9]+").unwrap();
    let binding = category.to_lowercase();
    let sanitized = re.replace_all(&binding, "_");
    sanitized.trim_matches('_').to_string()
}

impl CsvWriterPool {
    /// Creates a new CSV writer pool with the given output directory
    pub fn new(output_dir: &str) -> Self {
        println!("Creating output directory: {}", output_dir);
        create_dir_all(output_dir).expect("Failed to create output directory");
        Self {
            writers: DashMap::new(),
            output_dir: output_dir.to_string(),
        }
    }

    /// Writes a record to the appropriate CSV file
    pub fn write(&self, category: &str, record: &[&str]) {
        let filename = sanitize_filename(category);
        let writer = self
            .writers
            .entry(filename.to_string())
            .or_insert_with(|| {
                let path = format!("{}/{}.csv", self.output_dir, filename);
                let file = File::create(&path).expect("Failed to create CSV file");

                // Use WriterBuilder to set semi-colon delimiter
                let mut writer = WriterBuilder::new()
                    .delimiter(b';') // Set semi-colon as delimiter
                    .quote_style(csv::QuoteStyle::Necessary)
                    .from_writer(BufWriter::new(file));
                writer
                    .write_record(&["label", "sentences", "questions"])
                    .expect("Failed to write header");
                Arc::new(RwLock::new(writer))
            })
            .clone();

        // Lock and write to the file
        let mut writer_lock = writer.write().unwrap();
        writer_lock
            .write_record(record)
            .expect("Failed to write record");
        // TODO Remove this line
        // println!("Writing record to category: {}", category);
        // writer_lock.flush().expect("Failed to flush CSV file");
    }

    /// Flushes all writers to disk
    pub fn flush_all(&self) {
        self.writers.iter().for_each(|entry| {
            let writer = entry.value();
            let mut writer_lock = writer.write().unwrap();
            writer_lock.flush().expect("Failed to flush CSV file");
        });
    }
}

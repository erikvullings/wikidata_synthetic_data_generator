use crate::processing_error;
use std::{collections::HashMap, fs::File};

use processing_error::ProcessingError;

// Batched writer struct to handle buffered writes
pub struct BatchedWriter {
    csv_writers: HashMap<String, Vec<String>>,
    kv_entries: usize,
    batch_size: usize,
    total_csv_writers: HashMap<String, csv::Writer<File>>,
}

impl BatchedWriter {
    pub fn new(csv_writers: HashMap<String, csv::Writer<File>>, batch_size: usize) -> Self {
        BatchedWriter {
            csv_writers: HashMap::new(),
            total_csv_writers: csv_writers,
            kv_entries: 0,
            batch_size,
        }
    }

    pub fn add_csv_entry(
        &mut self,
        entity_type: String,
        record: String,
    ) -> Result<(), ProcessingError> {
        self.csv_writers
            .entry(entity_type)
            .or_insert_with(Vec::new)
            .push(record);

        // Flush if batch is full
        if self.kv_entries >= self.batch_size {
            self.flush()?;
        }

        Ok(())
    }

    pub fn add_kv_entry(&mut self) -> Result<(), ProcessingError> {
        self.kv_entries += 1;

        // Flush if batch is full
        if self.kv_entries >= self.batch_size {
            self.flush()?;
        }

        Ok(())
    }

    pub fn flush(&mut self) -> Result<(), ProcessingError> {
        // Flush CSV entries
        for (entity_type, entries) in &self.csv_writers {
            if let Some(writer) = self.total_csv_writers.get_mut(entity_type) {
                for label in entries {
                    writer.write_record(&[label])?;
                }
            }
        }
        self.csv_writers.clear();
        self.kv_entries = 0;

        Ok(())
    }

    // Ensure any remaining entries are written on drop
    pub fn finalize(&mut self) -> Result<(), ProcessingError> {
        self.flush()?;

        // Close and flush all CSV writers
        for writer in self.total_csv_writers.values_mut() {
            writer.flush()?;
        }

        Ok(())
    }
}

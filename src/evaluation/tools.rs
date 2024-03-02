
use std::fs::remove_file;
use std::{collections::HashMap, path::PathBuf};
use slow5::EnumField;
use slow5::{FileReader, RecordExt, Record};
use glob::glob;
use serde::{Serialize, Deserialize};

use cipher::simulation::config::MemberRole;
use cipher::simulation::simulator::Slow5EndReason;
use cipher::simulation::simulator::ReadRecord;
use crate::evaluation::error::EvaluationError;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CipherTimeSeriesRecord {
    pub uuid: String,
    pub start_time_seconds: f64,
    pub end_time_seconds: f64,
    pub read_sequenced_seconds: f64,
    pub read_completed: bool,
    pub output_signal_length: u64,
    pub simulated_signal_length: u64,
    pub channel_number: String,
    pub read_number: i32,
    pub end_reason: Slow5EndReason,
    pub ref_id: String,
    pub ref_start: u64,
    pub ref_end: u64,
    pub ref_strand: String,
    pub read_length: u64,
    pub member_id: String,
    pub member_role: MemberRole,
    pub member_target: bool,
    pub member_abundance: f32,
    pub member_taxid: String,
    pub member_scientific_name: String,
    pub member_accession: String
}
impl CipherTimeSeriesRecord {
    pub fn from(read_record: &ReadRecord, slow5_record: &Record) -> Result<Self, EvaluationError> {

        let signal_len = slow5_record.len_signal();
        let sampling_rate = slow5_record.sampling_rate();
        let start_time_seconds = slow5_record.get_aux_field::<u64>("start_time")? as f64 / sampling_rate;
        let end_time_seconds = start_time_seconds + (signal_len as f64 / sampling_rate);
        let read_sequenced_seconds =  end_time_seconds - start_time_seconds;
        let channel_number = slow5_record.get_aux_field::<&str>("channel_number")?.to_string();
        let read_number = slow5_record.get_aux_field::<i32>("read_number")?;
        let end_reason = Slow5EndReason::from_value(slow5_record.get_aux_field::<EnumField>("end_reason")?.0 as u8);

        let rr = read_record.clone();

        Ok(
            Self {
                uuid: rr.uuid,
                start_time_seconds,
                end_time_seconds,
                read_sequenced_seconds,
                read_completed: signal_len == read_record.signal_length,
                output_signal_length: signal_len,
                simulated_signal_length: read_record.signal_length,
                channel_number, 
                read_number,
                end_reason,
                ref_id: rr.ref_id,
                ref_start: rr.ref_start,
                ref_end: rr.ref_end,
                ref_strand: rr.ref_strand,
                read_length: rr.read_length,
                member_id: rr.member_id,
                member_role: rr.member_role,
                member_target: rr.member_target,
                member_abundance: rr.member_abundance,
                member_taxid: rr.member_taxid,
                member_scientific_name: rr.member_scientific_name,
                member_accession: rr.member_accession

            }
        )

    }
}

#[derive(Debug, Clone)]
pub struct EvaluationTools {

}

impl EvaluationTools {
    pub fn new() -> Self {
        Self {}
    }
    /// Create a timeseries from the output of a simulated community adaptive sequencing run
    /// for downstream plotting and other evaluation analysis using the `streamfish-utils` package
    pub fn cipher_timeseries(&self, directory: &PathBuf, metadata: &PathBuf, output: &PathBuf) -> Result<(), EvaluationError> {

        let pattern = directory.join("*.blow5").to_string_lossy().into_owned();

        let read_records: HashMap<String, ReadRecord> = HashMap::from_iter(
            get_cipher_read_records(&metadata)?.into_iter().map(|r| (r.uuid.clone(), r))
        );

        let mut records: Vec<CipherTimeSeriesRecord> = Vec::new();
        for entry in glob(&pattern)? {
            let path = entry?;
            let index_path = path.with_extension("blow5.idx");

            let mut not_found = 0;
            let mut reader = FileReader::open(&path)?;
            while let Some(rec) = reader.records().next() {
                let slow5_record = rec?;

                let record_uuid = &String::from_utf8_lossy(slow5_record.read_id()).into_owned();
                log::info!("{}", record_uuid);
                let result = read_records.get(record_uuid);

                if let None = result {
                    not_found += 1;
                    continue;
                }

                records.push(CipherTimeSeriesRecord::from(&result.unwrap(), &slow5_record)?);  // safe
                
            }
            if not_found > 0 {
                log::warn!("Could not find {} reads from simulation in output file: {}", not_found, path.display())
            }
            if index_path.exists() {
                // Remove index file after reading, some issues seem to arise from this...?
                remove_file(&index_path)?;
            }
        }
        let mut writer = csv::WriterBuilder::new().has_headers(true).delimiter(b'\t').from_path(output)?;
        for record in records {
            writer.serialize(record)?;
        }

        Ok(())
    }
}

fn get_cipher_read_records(path: &PathBuf) -> Result<Vec<ReadRecord>, EvaluationError> {
    let mut reader =  csv::ReaderBuilder::new().has_headers(true).delimiter(b'\t').from_path(path)?;
    let mut records = Vec::new();
    for result in reader.deserialize() {
        let record: ReadRecord = result?;
        records.push(record);
    }
    Ok(records)
}
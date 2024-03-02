use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkflowLauncherError {
    /// Indicates failure input/output file
    #[error("failed to read file")]
    FileIO(#[from] std::io::Error),
    /// Indicates failure obtain path name
    #[error("failed to parse base name of: {0}")]
    PathBaseName(String),
    /// Indicates failure to validate inputs
    #[error("failed input validation")]
    InputValidationFailed,
    /// Indicates failure to detect fastq sub-directory
    #[error("failed to detect fastq sub-directory")]
    FastqDirectoryNotFound,
}

/// Input validation and workflow launcher for production
pub struct WorkflowLauncher {
    pub base_path: PathBuf,
    pub input_path: PathBuf,
    pub run_id: String,
    pub launch_id: uuid::Uuid
}

impl WorkflowLauncher {
    pub fn new(base_path: PathBuf, input_path: PathBuf) -> Result<Self, WorkflowLauncherError> {
        
        // Input path directory is the run identifier
        let path_string = input_path.display().to_string();

        let run_id = match input_path.file_name() {
            Some(name) => match name.to_os_string().into_string() {
                Ok(run_id) => run_id, 
                Err(_) => return Err(WorkflowLauncherError::PathBaseName(path_string))
            },
            None => return Err(WorkflowLauncherError::PathBaseName(path_string))
        };

        // Launch identifier - used in the execution directory
        let launch_id = uuid::Uuid::new_v4();

        Ok(Self { base_path, input_path, run_id,  launch_id })
    }
    /// Validates presence and integrity of input sample sheet and read files 
    pub fn validate_inputs(&self) -> Result<(), WorkflowLauncherError> {

        let _run_id = self.run_id.clone();

        // Conduct all checks first
        let mut validation = InputValidation::new();

        // Does the sample sheet {run_id}.csv exist?
        let _sample_sheet_path = self.input_path.join(self.run_id.clone()).with_extension("csv");

        validation.sample_sheet_detected.checked = true;
        

        if validation.pass() {
            Ok(())
        } else {
            Err(WorkflowLauncherError::InputValidationFailed)
        }
    }
}


pub struct InputValidationChecks {
    pub pass: bool,
    pub checked: bool,
    pub error_message: String
}
impl Default for InputValidationChecks {
    fn default() -> Self {
        Self { pass: false, checked: false, error_message: String::from("") }
    }
}

pub struct InputValidation {
    pub sample_sheet_detected: InputValidationChecks,
    pub fastq_subdir_detected: InputValidationChecks,
    pub sample_sheet_parsed: InputValidationChecks,
    pub sample_sheet_not_empty: InputValidationChecks,
}
impl InputValidation {
    pub fn new() -> Self {
        Self { 
            sample_sheet_detected: Default::default(), 
            fastq_subdir_detected:  Default::default(),
            sample_sheet_parsed:  Default::default(),
            sample_sheet_not_empty:  Default::default()
        }
    }
    pub fn pass(&self) -> bool {
        self.sample_sheet_detected.pass &&
        self.fastq_subdir_detected.pass &&
        self.sample_sheet_parsed.pass && 
        self.sample_sheet_not_empty.pass
    }
}

use thiserror::Error;


#[derive(Error, Debug)]
pub enum EvaluationError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    /// Represents all other cases of `needletail::errors::ParseError`
    #[error(transparent)]
    NeedletailParseError(#[from] needletail::errors::ParseError),
    /// Represents all other cases of `slow5::Error`
    #[error(transparent)]
    Slow5Error(#[from] slow5::Slow5Error),
    /// Represents all cases of `csv::Error`
    #[error("Error in CSV")]
    CsvError(#[from] csv::Error),
    /// Represents all cases of `glob::PatternError`
    #[error("Error in pattern glob")]
    GlobPatternError(#[from] glob::PatternError),
    /// Represents all cases of `glob::PatternError`
    #[error("Error in path glob")]
    GlobError(#[from] glob::GlobError),
}

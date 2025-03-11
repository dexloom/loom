use eyre::ErrReport;
use revm::database::DBErrorMarker;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

#[derive(Clone, Default, Debug)]
pub enum LoomDBError {
    #[default]
    Nonimportant,
    TransportError,
    NoDB,
    DatabaseError(String),
}

impl DBErrorMarker for LoomDBError {}

impl Display for LoomDBError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for LoomDBError {}

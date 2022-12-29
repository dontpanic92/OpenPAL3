use std::{
    error::Error,
    fmt::{Display, Formatter},
};

#[derive(Debug)]
pub enum EntityError {
    EntityLoadingError,
    EntityAnimationNotFound,
}

impl Display for EntityError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl Error for EntityError {}

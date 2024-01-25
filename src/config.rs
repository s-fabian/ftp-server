use std::{collections::HashMap, fs, path::Path};

use crate::{
    user::{User, UserMap},
    BoxedStdError,
};

pub fn load(path: impl AsRef<Path>) -> Result<UserMap, BoxedStdError> {
    let content = fs::read_to_string(path)?;

    let res: Vec<User> = serde_yaml::from_str(&content)?;

    Ok(HashMap::from_iter(
        res.into_iter().map(|u| (u.name_cloned(), u)),
    ))
}

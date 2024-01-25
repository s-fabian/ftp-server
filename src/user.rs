use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    path::PathBuf,
};

use libunftp::auth::UserDetail;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VirtualDir {
    // pub name: String,
    pub path: PathBuf,
    pub read_only: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    access: HashMap<String, VirtualDir>,
    name: String,
    password: String,
}

impl User {
    pub fn get_access(&self, name: &String) -> Option<&VirtualDir> {
        self.access.get(name)
    }

    pub fn accesses(&self) -> Vec<(String, PathBuf)> {
        self.access
            .iter()
            .map(|(k, v)| (k.clone(), v.path.clone()))
            .collect()
    }

    pub fn password_ok(&self, password: &str) -> bool {
        self.password.trim() == password.trim()
    }

    pub fn name_ref(&self) -> &str { &self.name }

    pub fn name_cloned(&self) -> String { self.name.clone() }
}

pub type UserMap = HashMap<String, User>;

impl Display for User {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Account of {}", self.name)
    }
}

impl UserDetail for User {}

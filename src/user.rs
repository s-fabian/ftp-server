use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    path::PathBuf,
    sync::Arc,
};

use libunftp::auth::UserDetail;

#[derive(Debug, Clone)]
pub struct VirtualDir {
    pub name: String,
    pub path: PathBuf,
    pub read_only: bool,
}

impl VirtualDir {
    pub fn builder() -> VirtualDirBuilder {
        VirtualDirBuilder {
            name: None,
            path: None,
            read_only: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct User {
    access: HashMap<String, Arc<VirtualDir>>,
    name: String,
    password: String,
}

impl User {
    pub fn builder() -> UserBuilder {
        UserBuilder {
            access: Vec::new(),
            name: None,
            password: None,
        }
    }

    pub fn get_access(&self, name: &String) -> Option<Arc<VirtualDir>> {
        self.access.get(name).map(Arc::clone)
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
}

pub type UserMap = HashMap<String, User>;

impl Display for User {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Account of {}", self.name)
    }
}

pub struct UserBuilder {
    access: Vec<VirtualDir>,
    name: Option<String>,
    password: Option<String>,
}

impl UserBuilder {
    pub fn access(mut self, access: VirtualDir) -> Self {
        self.access.push(access);
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    pub fn build(self) -> User {
        User {
            access: HashMap::from_iter(
                self.access
                    .into_iter()
                    .map(|a| (a.name.clone(), Arc::new(a))),
            ),
            name: self.name.expect("No name provided!"),
            password: self.password.expect("No password provided!"),
        }
    }
}

pub struct VirtualDirBuilder {
    name: Option<String>,
    path: Option<PathBuf>,
    read_only: bool,
}

impl VirtualDirBuilder {
    pub fn build(self) -> VirtualDir {
        VirtualDir {
            name: self.name.expect("No name provided!"),
            path: self.path.expect("No path provided!"),
            read_only: self.read_only,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }
}

pub struct UsersBuilder {
    users: Vec<User>,
}

impl UsersBuilder {
    pub fn new() -> Self { UsersBuilder { users: Vec::new() } }

    pub fn with_user(mut self, user: User) -> Self {
        self.users.push(user);
        self
    }

    pub fn build(self) -> UserMap {
        HashMap::from_iter(self.users.into_iter().map(|u| (u.name.clone(), u)))
    }
}

impl UserDetail for User {}

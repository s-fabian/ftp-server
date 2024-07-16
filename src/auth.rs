use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use libunftp::auth::{AuthenticationError, Authenticator as LibAuthenticator};
use serde::Deserialize;
use tokio::time::sleep;

use crate::user::{User, UserMap};

#[derive(Deserialize, Clone, Debug)]
pub struct ClientCertCredential {
    // pub allowed_cn: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Authenticator {
    pub users: Arc<UserMap>,
}

#[async_trait]
impl LibAuthenticator<User> for Authenticator {
    // #[tracing_attributes::instrument]
    async fn authenticate(
        &self,
        username: &str,
        creds: &libunftp::auth::Credentials,
    ) -> Result<User, AuthenticationError> {
        let res = if let Some(user) = self.users.get(username) {
            match &creds.password {
                Some(ref given_password) =>
                    if !user.password_ok(given_password) {
                        Err(AuthenticationError::BadPassword)
                    } else {
                        Ok(user.clone())
                    },
                None => Err(AuthenticationError::BadPassword),
            }
        } else {
            Err(AuthenticationError::BadUser)
        };

        if res.is_err() {
            sleep(Duration::from_millis(1500)).await;
        }

        res
    }

    fn name(&self) -> &str { std::any::type_name::<Self>() }
}

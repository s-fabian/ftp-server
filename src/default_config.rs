use crate::user::{User, UserMap, UsersBuilder, VirtualDir};

pub fn default_config() -> UserMap {
    UsersBuilder::new()
        .with_user(
            User::builder()
                .name("fabian")
                .password("password")
                .access(
                    VirtualDir::builder()
                        .name("easy-snacks-read")
                        .path("/Users/fabian/Pictures")
                        .read_only()
                        .build(),
                )
                .access(
                    VirtualDir::builder()
                        .name("easy-snacks-write")
                        .path("/Users/fabian/Pictures")
                        .build(),
                )
                .build(),
        )
        .build()
}

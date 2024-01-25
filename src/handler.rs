use std::{
    fmt::Debug,
    path::{Component, Path, PathBuf},
    time::SystemTime,
};

use async_trait::async_trait;
use cfg_if::cfg_if;
use libunftp::storage::{Error, ErrorKind, Fileinfo, Metadata, Result, StorageBackend};
use tokio::io::AsyncSeekExt;

use crate::user::User;

cfg_if! {
    if #[cfg(target_os = "linux")] {
        use std::os::linux::fs::MetadataExt;
    } else if #[cfg(target_os = "unix")] {
        use std::os::unix::fs::MetadataExt;
    }
}

#[derive(Debug)]
pub struct Filesystem;

enum ResolveRes {
    Read(PathBuf, PathBuf),
    Write(PathBuf, PathBuf),
    Error(Error),
    Root(Vec<(String, PathBuf)>),
}

fn clean(clean: impl AsRef<Path>) -> Option<PathBuf> {
    let mut path = PathBuf::new();

    for component in clean.as_ref().components() {
        match component {
            Component::Prefix(..) => unimplemented!(),
            Component::RootDir => {
                path.push(Component::RootDir);
            },
            Component::CurDir => {},
            Component::ParentDir =>
                if !path.pop() {
                    return None;
                },
            Component::Normal(c) => {
                path.push(c);
            },
        }
    }

    if path.components().next().is_none() {
        path.push(Component::CurDir);
    }

    Some(path)
}

impl ResolveRes {
    fn read_ok(self) -> Result<PathBuf> {
        match self {
            ResolveRes::Read(r, _) => Ok(r),
            ResolveRes::Write(w, _) => Ok(w),
            ResolveRes::Error(e) => Err(e),
            ResolveRes::Root(_) => Err(Error::new(
                ErrorKind::PermissionDenied,
                String::from("Error: root not ok"),
            )),
        }
    }

    fn write_ok(self) -> Result<PathBuf> {
        match self {
            ResolveRes::Read(..) => Err(Error::new(
                ErrorKind::PermissionDenied,
                String::from("Error: permission denied"),
            )),
            ResolveRes::Write(w, _) => Ok(w),
            ResolveRes::Error(e) => Err(e),
            ResolveRes::Root(_) => Err(Error::new(
                ErrorKind::PermissionDenied,
                String::from("Error: root not ok"),
            )),
        }
    }
}

#[derive(Debug)]
pub struct Meta {
    inner: std::fs::Metadata,
}

fn canonicalize<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    use path_abs::PathAbs;
    let p = PathAbs::new(path)
        .map_err(|_| Error::from(ErrorKind::FileNameNotAllowedError))?;
    Ok(p.as_path().to_path_buf())
}

impl Filesystem {
    async fn full_path(&self, user: &User, path: impl AsRef<Path>) -> ResolveRes {
        let path = path.as_ref();
        let Some(path) = clean(path) else {
            return ResolveRes::Error(Error::new(
                ErrorKind::PermissionDenied,
                String::from("Error: no parent parent parent"),
            ));
        };
        let path = path.strip_prefix("/").unwrap_or(&path);

        let components: Vec<_> = path.components().collect();

        if components.is_empty() {
            return ResolveRes::Root(user.accesses());
        }

        let Some(Component::Normal(first_dir)) = components.first() else {
            return ResolveRes::Error(Error::new(
                ErrorKind::PermissionDenied,
                String::from("Error: invalid path"),
            ));
        };
        let virtual_path = first_dir.to_string_lossy().into_owned();

        let Some(target_path) = user.get_access(&virtual_path) else {
            return if virtual_path.is_empty() {
                ResolveRes::Root(user.accesses())
            } else {
                ResolveRes::Error(Error::new(
                    ErrorKind::PermissionDenied,
                    String::from("Error: invalid path"),
                ))
            };
        };
        let real_path = target_path
            .path
            .as_path()
            .join(path.strip_prefix(first_dir).unwrap_or(path));

        let full_path = match tokio::task::spawn_blocking(move || canonicalize(real_path))
            .await
            .map_err(|e| Error::new(ErrorKind::LocalError, e))
        {
            Err(e) => return ResolveRes::Error(e),
            Ok(Err(e)) => return ResolveRes::Error(e),
            Ok(Ok(p)) => p,
        };

        if full_path.starts_with(&target_path.path) {
            if full_path.ends_with(&virtual_path) || target_path.read_only {
                ResolveRes::Read(full_path, target_path.path.clone())
            } else {
                ResolveRes::Write(full_path, target_path.path.clone())
            }
        } else {
            ResolveRes::Error(Error::from(ErrorKind::PermanentFileNotAvailable))
        }
    }
}

#[async_trait]
impl StorageBackend<User> for Filesystem {
    type Metadata = Meta;

    fn supported_features(&self) -> u32 {
        libunftp::storage::FEATURE_RESTART | libunftp::storage::FEATURE_SITEMD5
    }

    #[tracing_attributes::instrument]
    async fn metadata<P: AsRef<Path> + Send + Debug>(
        &self,
        user: &User,
        path: P,
    ) -> Result<Self::Metadata> {
        let full_path = self.full_path(user, path).await.read_ok()?;

        let fs_meta = tokio::fs::symlink_metadata(full_path)
            .await
            .map_err(|_| Error::from(ErrorKind::PermanentFileNotAvailable))?;
        Ok(Meta { inner: fs_meta })
    }

    #[allow(clippy::type_complexity)]
    #[tracing_attributes::instrument]
    async fn list<P>(
        &self,
        user: &User,
        path: P,
    ) -> Result<Vec<Fileinfo<PathBuf, Self::Metadata>>>
    where
        P: AsRef<Path> + Send + Debug,
        <Self as StorageBackend<User>>::Metadata: Metadata,
    {
        let (full_path, prefix) = match self.full_path(user, path).await {
            ResolveRes::Read(full_path, prefix) => (full_path, prefix),
            ResolveRes::Write(full_path, prefix) => (full_path, prefix),
            ResolveRes::Error(e) => return Err(e),
            ResolveRes::Root(paths) => {
                let mut fis: Vec<Fileinfo<PathBuf, Self::Metadata>> = Vec::new();
                for (name, real) in paths {
                    let metadata = tokio::fs::symlink_metadata(real.as_path()).await?;
                    let meta: Self::Metadata = Meta { inner: metadata };
                    fis.push(Fileinfo {
                        path: PathBuf::from(name),
                        metadata: meta,
                    })
                }

                return Ok(fis);
            },
        };

        let mut rd: tokio::fs::ReadDir = tokio::fs::read_dir(full_path).await?;

        let mut fis: Vec<Fileinfo<PathBuf, Self::Metadata>> = vec![];
        while let Ok(Some(dir_entry)) = rd.next_entry().await {
            let prefix = prefix.clone();
            let path = dir_entry.path();
            let relpath = path.strip_prefix(prefix).unwrap();
            let relpath: PathBuf = PathBuf::from(relpath);
            let metadata = tokio::fs::symlink_metadata(dir_entry.path()).await?;
            let meta: Self::Metadata = Meta { inner: metadata };
            fis.push(Fileinfo {
                path: relpath,
                metadata: meta,
            })
        }

        Ok(fis)
    }

    //#[tracing_attributes::instrument]
    async fn get<P: AsRef<Path> + Send + Debug>(
        &self,
        user: &User,
        path: P,
        start_pos: u64,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Sync + Unpin>> {
        use tokio::io::AsyncSeekExt;

        let full_path = self.full_path(user, path).await.read_ok()?;
        let mut file = tokio::fs::File::open(full_path).await?;
        if start_pos > 0 {
            file.seek(std::io::SeekFrom::Start(start_pos)).await?;
        }

        Ok(Box::new(tokio::io::BufReader::with_capacity(4096, file))
            as Box<dyn tokio::io::AsyncRead + Send + Sync + Unpin>)
    }

    async fn put<
        P: AsRef<Path> + Send,
        R: tokio::io::AsyncRead + Send + Sync + 'static + Unpin,
    >(
        &self,
        user: &User,
        bytes: R,
        path: P,
        start_pos: u64,
    ) -> Result<u64> {
        let path = path.as_ref();
        let full_path = self.full_path(user, path).await.write_ok()?;

        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(full_path)
            .await?;
        file.set_len(start_pos).await?;
        file.seek(std::io::SeekFrom::Start(start_pos)).await?;

        let mut reader = tokio::io::BufReader::with_capacity(4096, bytes);
        let mut writer = tokio::io::BufWriter::with_capacity(4096, file);

        let bytes_copied = tokio::io::copy(&mut reader, &mut writer).await?;
        Ok(bytes_copied)
    }

    #[tracing_attributes::instrument]
    async fn del<P: AsRef<Path> + Send + Debug>(
        &self,
        user: &User,
        path: P,
    ) -> Result<()> {
        let full_path = self.full_path(user, path).await.write_ok()?;
        tokio::fs::remove_file(full_path)
            .await
            .map_err(|error: std::io::Error| error.into())
    }

    #[tracing_attributes::instrument]
    async fn mkd<P: AsRef<Path> + Send + Debug>(
        &self,
        user: &User,
        path: P,
    ) -> Result<()> {
        tokio::fs::create_dir(self.full_path(user, path).await.write_ok()?)
            .await
            .map_err(|error: std::io::Error| error.into())
    }

    #[tracing_attributes::instrument]
    async fn rename<P: AsRef<Path> + Send + Debug>(
        &self,
        user: &User,
        from: P,
        to: P,
    ) -> Result<()> {
        let from = self.full_path(user, from).await.write_ok()?;
        let to = self.full_path(user, to).await.write_ok()?;

        let from_rename = from.clone();

        let r = tokio::fs::symlink_metadata(from).await;
        match r {
            Ok(metadata) =>
                if metadata.is_file() || metadata.is_dir() {
                    let r = tokio::fs::rename(from_rename, to).await;
                    match r {
                        Ok(_) => Ok(()),
                        Err(e) =>
                            Err(Error::new(ErrorKind::PermanentFileNotAvailable, e)),
                    }
                } else {
                    Err(Error::from(ErrorKind::PermanentFileNotAvailable))
                },
            Err(e) => Err(Error::new(ErrorKind::PermanentFileNotAvailable, e)),
        }
    }

    #[tracing_attributes::instrument]
    async fn rmd<P: AsRef<Path> + Send + Debug>(
        &self,
        user: &User,
        path: P,
    ) -> Result<()> {
        let full_path = self.full_path(user, path).await.write_ok()?;
        tokio::fs::remove_dir(full_path)
            .await
            .map_err(|error: std::io::Error| error.into())
    }

    #[tracing_attributes::instrument]
    async fn cwd<P: AsRef<Path> + Send + Debug>(
        &self,
        user: &User,
        path: P,
    ) -> Result<()> {
        let full_path = self.full_path(user, path).await.read_ok()?;
        tokio::fs::read_dir(full_path)
            .await
            .map_err(|error: std::io::Error| error.into())
            .map(|_| ())
    }
}

impl Metadata for Meta {
    fn len(&self) -> u64 { self.inner.len() }

    fn is_dir(&self) -> bool { self.inner.is_dir() }

    fn is_file(&self) -> bool { self.inner.is_file() }

    fn is_symlink(&self) -> bool { self.inner.file_type().is_symlink() }

    fn modified(&self) -> Result<SystemTime> {
        self.inner.modified().map_err(|e| e.into())
    }

    fn gid(&self) -> u32 {
        cfg_if! {
            if #[cfg(target_os = "linux")] {
                self.inner.st_gid()
            }
            else if #[cfg(target_os = "unix")] {
                self.inner.gid()
            } else {
                0
            }
        }
    }

    fn uid(&self) -> u32 {
        cfg_if! {
            if #[cfg(target_os = "linux")] {
                self.inner.st_uid()
            }
            else if #[cfg(target_os = "unix")] {
                self.inner.uid()
            } else {
                0
            }
        }
    }

    fn links(&self) -> u64 {
        cfg_if! {
            if #[cfg(target_os = "linux")] {
                self.inner.st_nlink()
            }
            else if #[cfg(target_os = "unix")] {
                self.inner.nlink()
            } else {
                1
            }
        }
    }
}

use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use tower_lsp::lsp_types::Url;
use typst::syntax::PackageSpec;

use crate::ext::PathExt;
use crate::workspace::fs::local::LocalFs;
use crate::workspace::package::{FullFileId, Package, PackageId};

use super::repo::{RepoProvider, RepoResult};
use super::ExternalPackageProvider;

// TODO: cache packages so we don't need to do IO to check if a package is provided
#[derive(Debug)]
pub struct LocalProvider {
    root: PathBuf,
}

impl ExternalPackageProvider for LocalProvider {
    fn package(&self, spec: &PackageSpec) -> Option<Package> {
        let path = self.fs_path(spec);
        let manifest = path.join("typst.toml");

        if manifest.is_file() {
            let uri = LocalFs::path_to_uri(path).expect("should be absolute");
            Some(Package::new(uri))
        } else {
            None
        }
    }

    fn full_id(&self, uri: &Url) -> Option<FullFileId> {
        let path = LocalFs::uri_to_path(uri).ok()?;
        let relative_path = path.strip_prefix(&self.root).ok()?;
        let (spec, package_path) = Self::split_spec(relative_path)?;

        let package_id = PackageId::new_external(spec);
        let full_id = FullFileId::new(package_id, package_path.to_owned());

        Some(full_id)
    }
}

impl LocalProvider {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root: root_dir }
    }

    #[tracing::instrument]
    pub async fn download(&self, spec: &PackageSpec, repo: &RepoProvider) -> RepoResult<Package> {
        let path = self.fs_path(spec);
        repo.download_to(spec, &path).await?;
        Ok(Package::new(
            LocalFs::path_to_uri(path).expect("should be absolute"),
        ))
    }

    fn fs_path(&self, spec: &PackageSpec) -> PathBuf {
        let subdir = format!("{}/{}/{}/", spec.namespace, spec.name, spec.version);
        self.root.join(subdir)
    }

    /// Parse a spec from a path to a package directory, relative to the packaging root, returning
    /// the spec and package path.
    ///
    /// For example, given `preview/test/0.1.0/subdir/example.typ`, the spec `@preview/test:0.1.0`
    /// and path `subdir/example.typ` will be returned.
    fn split_spec(path: &Path) -> Option<(PackageSpec, PathBuf)> {
        let mut components = path.components();

        let mut components_str = (&mut components)
            .filter(|component| matches!(component, Component::Normal(_)))
            .map(|component| component.as_os_str());

        let namespace = components_str.next()?.to_str()?;
        let name = components_str.next()?.to_str()?;
        let version = components_str.next()?.to_str()?;
        let spec_str = format!("@{namespace}/{name}:{version}");

        let spec = PackageSpec::from_str(&spec_str).ok()?;

        let package_path = components.as_path().push_front(Path::root());

        Some((spec, package_path))
    }
}

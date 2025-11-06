//! Shared testing utilities placeholder.

use anyhow::Result;
use inkgen_core::{PackageId, PackageService};

/// Utility to create a temporary directory for generator tests.
pub fn temp_output_dir(prefix: &str) -> Result<tempfile::TempDir> {
    let dir = tempfile::Builder::new().prefix(prefix).tempdir()?;
    Ok(dir)
}

/// Trait extension that asserts a package is described.
pub trait PackageServiceExt: PackageService {
    /// Assert the service can respond for the given package id.
    fn assert_has_package(&self, id: &PackageId) -> Result<()> {
        let maybe = self.describe(id)?;
        if maybe.is_none() {
            anyhow::bail!("package {} missing from service", id.name);
        }
        Ok(())
    }
}

impl<T> PackageServiceExt for T where T: PackageService {}

#[cfg(test)]
mod tests {
    use super::*;
    use inkgen_core::{PackageMetadata, PackageService};

    struct StubService;

    impl PackageService for StubService {
        fn describe(&self, id: &PackageId) -> anyhow::Result<Option<PackageMetadata>> {
            let metadata = PackageMetadata::default();
            if id.name == "hl7.fhir.r4.core" {
                Ok(Some(metadata))
            } else {
                Ok(None)
            }
        }
    }

    #[test]
    fn temp_output_dir_is_created() {
        let dir = temp_output_dir("inkgen-test").expect("dir");
        assert!(dir.path().exists());
    }

    #[test]
    fn package_service_ext_asserts_presence() {
        let service = StubService;
        let package = PackageId::new("hl7.fhir.r4.core", None);
        service.assert_has_package(&package).expect("found");
    }
}

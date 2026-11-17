use std::fmt;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
#[cfg(feature = "resolver")]
use octofhir_canonical_manager::config::PackageSpec;
use serde::{Deserialize, Serialize};

/// Identifier for a FHIR package (name + version).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackageId {
    /// Package name (e.g. `hl7.fhir.r4.core`).
    pub name: String,
    /// Package version (e.g. `4.0.1`).
    pub version: String,
}

impl PackageId {
    /// Creates an identifier from a package name and version.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }

    /// Converts to a resolver `PackageSpec` with the given resolution priority.
    #[cfg(feature = "resolver")]
    pub fn as_spec(&self, priority: u32) -> PackageSpec {
        PackageSpec {
            name: self.name.clone(),
            version: self.version.clone(),
            priority,
            url: None,
        }
    }

    /// Formats the identifier as `<name>@<version>`.
    pub fn as_str(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.name, self.version)
    }
}

impl std::str::FromStr for PackageId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut segments = value.split('@');
        let name = segments
            .next()
            .ok_or_else(|| "missing package name".to_string())?
            .trim();
        let version = segments
            .next()
            .ok_or_else(|| "missing package version".to_string())?
            .trim();

        if name.is_empty() || version.is_empty() {
            return Err("package identifiers must be in the form <name>@<version>".to_string());
        }

        Ok(Self::new(name, version))
    }
}

/// Source of a package (remote registry or local override).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageSource {
    /// Fetched from a remote FHIR package registry.
    Registry,
    /// Loaded from a local directory instead of the registry.
    Local {
        /// Filesystem path of the local package directory.
        path: PathBuf,
        /// Resolution priority of the local package.
        priority: i32,
    },
}

impl PackageSource {
    /// Creates a local-directory source with the given resolution priority.
    pub fn local(path: impl AsRef<Path>, priority: i32) -> Self {
        Self::Local {
            path: path.as_ref().to_path_buf(),
            priority,
        }
    }
}

/// Request describing a package that should be available in the cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageRequest {
    /// Package to make available in the cache.
    pub id: PackageId,
    /// Resolution priority used when the same canonical appears in several packages.
    pub priority: u32,
    /// Where the package contents come from.
    pub source: PackageSource,
}

impl PackageRequest {
    /// Requests a package from the remote registry with default priority.
    pub fn registry(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: PackageId::new(name, version),
            priority: 1,
            source: PackageSource::Registry,
        }
    }

    /// Requests a package from a local directory with default request priority.
    pub fn local(
        name: impl Into<String>,
        version: impl Into<String>,
        path: impl AsRef<Path>,
        priority: i32,
    ) -> Self {
        Self {
            id: PackageId::new(name, version),
            priority: 1,
            source: PackageSource::local(path, priority),
        }
    }

    /// Builds a descriptor for the installed package from its resource count and
    /// inventory.
    pub fn descriptor(
        &self,
        resource_count: usize,
        inventory: PackageInventory,
    ) -> PackageDescriptor {
        PackageDescriptor {
            id: self.id.clone(),
            source: self.source.clone(),
            priority: self.priority,
            resource_count,
            inventory,
        }
    }
}

/// Basic descriptor for an installed package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDescriptor {
    /// Identifier of the installed package.
    pub id: PackageId,
    /// Where the package was loaded from.
    pub source: PackageSource,
    /// Resolution priority carried over from the request.
    pub priority: u32,
    /// Number of FHIR resources found in the package.
    pub resource_count: usize,
    /// Artifacts discovered in the package, grouped by kind.
    pub inventory: PackageInventory,
}

impl PackageDescriptor {
    /// StructureDefinition summaries from the package inventory.
    pub fn structures(&self) -> &[StructureSummary] {
        &self.inventory.structures
    }
}

/// Kind of artifact exposed by a package.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtifactKind {
    /// A StructureDefinition artifact (types, resources, profiles).
    StructureDefinition,
    /// A ValueSet artifact.
    ValueSet,
    /// A CodeSystem artifact.
    CodeSystem,
    /// A CapabilityStatement artifact.
    CapabilityStatement,
    /// Any other resource type, kept verbatim.
    Other(String),
}

impl ArtifactKind {
    /// Maps a FHIR `resourceType` string to a kind, falling back to `Other`.
    pub fn from_resource_type(resource_type: &str) -> Self {
        match resource_type {
            "StructureDefinition" => Self::StructureDefinition,
            "ValueSet" => Self::ValueSet,
            "CodeSystem" => Self::CodeSystem,
            "CapabilityStatement" => Self::CapabilityStatement,
            other => Self::Other(other.to_string()),
        }
    }
}

/// Lightweight description of a FHIR artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactDescriptor {
    /// Canonical URL of the artifact, if declared.
    pub canonical_url: Option<String>,
    /// Computer-friendly artifact name, if declared.
    pub name: Option<String>,
    /// FHIR `resourceType` of the artifact.
    pub resource_type: String,
    /// Business version of the artifact, if declared.
    pub version: Option<String>,
    /// Publication status (e.g. `draft`, `active`).
    pub status: Option<String>,
    /// Package the artifact belongs to.
    pub package: PackageId,
}

impl ArtifactDescriptor {
    /// Classifies the artifact by its `resource_type`.
    pub fn kind(&self) -> ArtifactKind {
        ArtifactKind::from_resource_type(&self.resource_type)
    }
}

/// Classification for StructureDefinitions in inventory listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructureKind {
    /// A base FHIR resource definition (e.g. `Patient`).
    BaseResource,
    /// A complex datatype definition (e.g. `HumanName`).
    ComplexType,
    /// A primitive datatype definition (e.g. `string`).
    PrimitiveType,
    /// A logical model.
    Logical,
    /// A profile constraining another definition.
    Profile,
}

/// Summary describing a StructureDefinition artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureSummary {
    /// Canonical URL of the StructureDefinition.
    pub canonical_url: String,
    /// Computer-friendly name, if declared.
    pub name: Option<String>,
    /// FHIR type code the definition describes (`StructureDefinition.type`).
    pub type_code: Option<String>,
    /// Human-friendly title, if declared.
    pub title: Option<String>,
    /// Business version, if declared.
    pub version: Option<String>,
    /// Publication status, if declared.
    pub status: Option<String>,
    /// Package providing the definition.
    pub package: PackageId,
    /// Inventory classification of the definition.
    pub kind: StructureKind,
}

/// Inventory of known artifacts grouped by semantic buckets.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackageInventory {
    /// StructureDefinition summaries.
    pub structures: Vec<StructureSummary>,
    /// ValueSet artifacts.
    pub value_sets: Vec<ArtifactDescriptor>,
    /// CodeSystem artifacts.
    pub code_systems: Vec<ArtifactDescriptor>,
    /// CapabilityStatement artifacts.
    pub capability_statements: Vec<ArtifactDescriptor>,
    /// Remaining artifacts grouped by resource type, in insertion order.
    pub others: IndexMap<String, Vec<ArtifactDescriptor>>,
}

impl PackageInventory {
    /// True when no bucket contains any artifact.
    pub fn is_empty(&self) -> bool {
        self.structures.is_empty()
            && self.value_sets.is_empty()
            && self.code_systems.is_empty()
            && self.capability_statements.is_empty()
            && self.others.values().all(|list| list.is_empty())
    }

    /// Total number of artifacts across all buckets.
    pub fn total_artifacts(&self) -> usize {
        self.structures.len()
            + self.value_sets.len()
            + self.code_systems.len()
            + self.capability_statements.len()
            + self.others.values().map(Vec::len).sum::<usize>()
    }

    /// Appends a StructureDefinition summary to the inventory.
    pub fn push_structure(&mut self, summary: StructureSummary) {
        self.structures.push(summary);
    }

    /// Routes an artifact into the bucket matching its kind. StructureDefinitions
    /// are expected to be pushed as [`StructureSummary`] via [`Self::push_structure`].
    pub fn push_artifact(&mut self, descriptor: ArtifactDescriptor) {
        match descriptor.kind() {
            ArtifactKind::ValueSet => self.value_sets.push(descriptor),
            ArtifactKind::CodeSystem => self.code_systems.push(descriptor),
            ArtifactKind::CapabilityStatement => self.capability_statements.push(descriptor),
            ArtifactKind::Other(kind) => {
                self.others.entry(kind).or_default().push(descriptor);
            }
            ArtifactKind::StructureDefinition => {
                // StructureDefinitions should be converted to StructureSummary before pushing.
                self.others
                    .entry("StructureDefinition".to_string())
                    .or_default()
                    .push(descriptor);
            }
        }
    }

    /// Sorts every bucket for deterministic output: structures by type code then
    /// canonical URL, other artifacts by canonical URL then name.
    pub fn sort(&mut self) {
        self.structures.sort_by(|a, b| {
            let key_a = (
                a.type_code.clone().unwrap_or_default(),
                a.canonical_url.clone(),
            );
            let key_b = (
                b.type_code.clone().unwrap_or_default(),
                b.canonical_url.clone(),
            );
            key_a.cmp(&key_b)
        });

        let sorter = |items: &mut Vec<ArtifactDescriptor>| {
            items.sort_by(|a, b| {
                let key_a = (
                    a.canonical_url.clone().unwrap_or_default(),
                    a.name.clone().unwrap_or_default(),
                );
                let key_b = (
                    b.canonical_url.clone().unwrap_or_default(),
                    b.name.clone().unwrap_or_default(),
                );
                key_a.cmp(&key_b)
            });
        };

        // `structures` already sorted above
        sorter(&mut self.value_sets);
        sorter(&mut self.code_systems);
        sorter(&mut self.capability_statements);
        for list in self.others.values_mut() {
            sorter(list);
        }
    }
}

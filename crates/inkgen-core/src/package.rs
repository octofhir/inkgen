use std::fmt;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use octofhir_canonical_manager::config::PackageSpec;
use serde::{Deserialize, Serialize};

/// Identifier for a FHIR package (name + version).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackageId {
    pub name: String,
    pub version: String,
}

impl PackageId {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }

    pub fn as_spec(&self, priority: u32) -> PackageSpec {
        PackageSpec {
            name: self.name.clone(),
            version: self.version.clone(),
            priority,
            url: None,
        }
    }

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
    Registry,
    Local { path: PathBuf, priority: i32 },
}

impl PackageSource {
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
    pub id: PackageId,
    pub priority: u32,
    pub source: PackageSource,
}

impl PackageRequest {
    pub fn registry(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: PackageId::new(name, version),
            priority: 1,
            source: PackageSource::Registry,
        }
    }

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
    pub id: PackageId,
    pub source: PackageSource,
    pub priority: u32,
    pub resource_count: usize,
    pub inventory: PackageInventory,
}

impl PackageDescriptor {
    pub fn structures(&self) -> &[StructureSummary] {
        &self.inventory.structures
    }
}

/// Kind of artifact exposed by a package.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtifactKind {
    StructureDefinition,
    ValueSet,
    CodeSystem,
    CapabilityStatement,
    Other(String),
}

impl ArtifactKind {
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
    pub canonical_url: Option<String>,
    pub name: Option<String>,
    pub resource_type: String,
    pub version: Option<String>,
    pub status: Option<String>,
    pub package: PackageId,
}

impl ArtifactDescriptor {
    pub fn kind(&self) -> ArtifactKind {
        ArtifactKind::from_resource_type(&self.resource_type)
    }
}

/// Classification for StructureDefinitions in inventory listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructureKind {
    BaseResource,
    ComplexType,
    PrimitiveType,
    Logical,
    Profile,
}

/// Summary describing a StructureDefinition artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureSummary {
    pub canonical_url: String,
    pub name: Option<String>,
    pub type_code: Option<String>,
    pub title: Option<String>,
    pub version: Option<String>,
    pub status: Option<String>,
    pub package: PackageId,
    pub kind: StructureKind,
}

/// Inventory of known artifacts grouped by semantic buckets.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackageInventory {
    pub structures: Vec<StructureSummary>,
    pub value_sets: Vec<ArtifactDescriptor>,
    pub code_systems: Vec<ArtifactDescriptor>,
    pub capability_statements: Vec<ArtifactDescriptor>,
    pub others: IndexMap<String, Vec<ArtifactDescriptor>>,
}

impl PackageInventory {
    pub fn is_empty(&self) -> bool {
        self.structures.is_empty()
            && self.value_sets.is_empty()
            && self.code_systems.is_empty()
            && self.capability_statements.is_empty()
            && self.others.values().all(|list| list.is_empty())
    }

    pub fn total_artifacts(&self) -> usize {
        self.structures.len()
            + self.value_sets.len()
            + self.code_systems.len()
            + self.capability_statements.len()
            + self.others.values().map(Vec::len).sum::<usize>()
    }

    pub fn push_structure(&mut self, summary: StructureSummary) {
        self.structures.push(summary);
    }

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

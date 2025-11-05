//! Element tree definitions for IR

use serde::{Deserialize, Serialize};
use indexmap::IndexMap;

/// Hierarchical representation of FHIR elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementTree {
    pub root: ElementNode,
    pub elements: IndexMap<String, ElementNode>, // Using IndexMap for deterministic ordering
}

/// Individual element node in the tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementNode {
    pub path: String,
    pub definition: ElementDefinition,
    pub children: Vec<String>,
    pub slicing: Option<SlicingInfo>,
}

/// FHIR ElementDefinition representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementDefinition {
    pub min: u32,
    pub max: String, // Can be "*" for unbounded
    pub types: Vec<ElementType>,
    pub must_support: bool,
    pub short: Option<String>,
    pub definition: Option<String>,
    pub comment: Option<String>,
}

/// Type information for an element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementType {
    pub code: String,
    pub profile: Option<String>,
    pub target_profile: Option<String>,
}

/// Slicing information for complex elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlicingInfo {
    pub discriminator: Vec<Discriminator>,
    pub description: Option<String>,
    pub ordered: bool,
    pub rules: SlicingRules,
}

/// Discriminator for slicing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discriminator {
    pub type_: DiscriminatorType,
    pub path: String,
}

/// Type of discriminator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscriminatorType {
    Value,
    Exists,
    Pattern,
    Type,
    Profile,
}

/// Rules for slicing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SlicingRules {
    Closed,
    Open,
    OpenAtEnd,
}

impl ElementTree {
    /// Create a new ElementTree with a root element
    pub fn new(root: ElementNode) -> Self {
        let mut elements = IndexMap::new();
        elements.insert(root.path.clone(), root.clone());

        Self { root, elements }
    }

    /// Add an element to the tree
    pub fn add_element(&mut self, element: ElementNode) {
        self.elements.insert(element.path.clone(), element);
    }

    /// Get an element by path
    pub fn get_element(&self, path: &str) -> Option<&ElementNode> {
        self.elements.get(path)
    }

    /// Get all child elements of a given path
    pub fn get_children(&self, path: &str) -> Vec<&ElementNode> {
        if let Some(parent) = self.elements.get(path) {
            parent
                .children
                .iter()
                .filter_map(|child_path| self.elements.get(child_path))
                .collect()
        } else {
            Vec::new()
        }
    }
}

impl ElementNode {
    /// Create a new ElementNode
    pub fn new(path: String, definition: ElementDefinition) -> Self {
        Self {
            path,
            definition,
            children: Vec::new(),
            slicing: None,
        }
    }

    /// Add a child element path
    pub fn add_child(&mut self, child_path: String) {
        if !self.children.contains(&child_path) {
            self.children.push(child_path);
        }
    }

    /// Check if this element is required (min > 0)
    pub fn is_required(&self) -> bool {
        self.definition.min > 0
    }

    /// Check if this element is repeating (max != "1")
    pub fn is_repeating(&self) -> bool {
        self.definition.max != "1"
    }
}
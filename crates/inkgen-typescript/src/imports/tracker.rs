/// Context in which a type is used
///
/// This enum tracks where and how a type is referenced, allowing for intelligent
/// import optimization and tree-shaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UsageContext {
    /// Type used in an interface or type alias field
    InterfaceField,
    /// Type referenced in a Zod schema
    ZodSchema,
    /// Type used in runtime code (class methods, builders, etc.)
    RuntimeCode,
    /// Type used in a type alias definition
    TypeAlias,
    /// Type used in an extends/implements clause
    Extends,
}

/// Tracks which types are used and in what contexts
///
/// This enables tree-shaking optimizations by identifying unused imports
/// and separating type-only imports from value imports.
#[derive(Debug, Default)]
pub struct UsageTracker {
    /// Map of type name to the contexts in which it's used
    usage_map: std::collections::HashMap<String, std::collections::HashSet<UsageContext>>,
}

impl UsageTracker {
    /// Creates a new empty usage tracker
    pub fn new() -> Self {
        Self {
            usage_map: std::collections::HashMap::new(),
        }
    }

    /// Records that a type is used in a specific context
    ///
    /// # Arguments
    /// * `type_name` - The name of the type being used
    /// * `context` - The context in which the type is used
    pub fn track_usage(&mut self, type_name: String, context: UsageContext) {
        self.usage_map.entry(type_name).or_default().insert(context);
    }

    /// Checks if a type is used in any context
    pub fn is_used(&self, type_name: &str) -> bool {
        self.usage_map.contains_key(type_name)
    }

    /// Gets all contexts in which a type is used
    pub fn get_contexts(
        &self,
        type_name: &str,
    ) -> Option<&std::collections::HashSet<UsageContext>> {
        self.usage_map.get(type_name)
    }

    /// Checks if a type is used only in type-only contexts
    ///
    /// Returns true if the type is used only in contexts that don't require
    /// runtime values (interface fields, type aliases, etc.)
    pub fn is_type_only(&self, type_name: &str) -> bool {
        if let Some(contexts) = self.get_contexts(type_name) {
            contexts.iter().all(|ctx| {
                matches!(
                    ctx,
                    UsageContext::InterfaceField | UsageContext::TypeAlias | UsageContext::Extends
                )
            })
        } else {
            false
        }
    }

    /// Checks if a type is used in runtime code
    ///
    /// Returns true if the type is used in contexts that require runtime values
    /// (class methods, builders, concrete runtime usage, etc.)
    pub fn needs_value_import(&self, type_name: &str) -> bool {
        if let Some(contexts) = self.get_contexts(type_name) {
            contexts
                .iter()
                .any(|ctx| matches!(ctx, UsageContext::RuntimeCode))
        } else {
            false
        }
    }

    /// Returns all tracked type names
    pub fn all_types(&self) -> Vec<&str> {
        self.usage_map.keys().map(|s| s.as_str()).collect()
    }

    /// Returns the total number of tracked types
    pub fn len(&self) -> usize {
        self.usage_map.len()
    }

    /// Checks if the tracker is empty
    pub fn is_empty(&self) -> bool {
        self.usage_map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_track_usage() {
        let mut tracker = UsageTracker::new();

        tracker.track_usage("Patient".to_string(), UsageContext::InterfaceField);
        tracker.track_usage("Patient".to_string(), UsageContext::ZodSchema);

        assert!(tracker.is_used("Patient"));
        assert!(!tracker.is_used("Observation"));
        assert_eq!(tracker.len(), 1);
    }

    #[test]
    fn test_type_only_usage() {
        let mut tracker = UsageTracker::new();

        tracker.track_usage("Patient".to_string(), UsageContext::InterfaceField);
        tracker.track_usage("Patient".to_string(), UsageContext::TypeAlias);

        assert!(tracker.is_type_only("Patient"));
        assert!(!tracker.needs_value_import("Patient"));
    }

    #[test]
    fn test_value_import_needed() {
        let mut tracker = UsageTracker::new();

        tracker.track_usage("Patient".to_string(), UsageContext::InterfaceField);
        tracker.track_usage("Patient".to_string(), UsageContext::RuntimeCode);

        assert!(!tracker.is_type_only("Patient"));
        assert!(tracker.needs_value_import("Patient"));
    }

    #[test]
    fn test_multiple_types() {
        let mut tracker = UsageTracker::new();

        tracker.track_usage("Patient".to_string(), UsageContext::InterfaceField);
        tracker.track_usage("Observation".to_string(), UsageContext::RuntimeCode);
        tracker.track_usage("Condition".to_string(), UsageContext::TypeAlias);

        assert_eq!(tracker.len(), 3);
        assert!(tracker.is_type_only("Condition"));
        assert!(tracker.needs_value_import("Observation"));
    }

    #[test]
    fn test_get_contexts() {
        let mut tracker = UsageTracker::new();

        tracker.track_usage("Patient".to_string(), UsageContext::InterfaceField);
        tracker.track_usage("Patient".to_string(), UsageContext::ZodSchema);

        let contexts = tracker.get_contexts("Patient").unwrap();
        assert_eq!(contexts.len(), 2);
        assert!(contexts.contains(&UsageContext::InterfaceField));
        assert!(contexts.contains(&UsageContext::ZodSchema));
    }
}

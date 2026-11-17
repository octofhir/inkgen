//! Pydantic v2 model generation from a resolved [`PackageIr`].
//!
//! Emits an importable `fhir` package: one module per resource/complex type,
//! composite classes for backbone elements, `Literal` aliases for enumerated
//! ValueSet bindings, and a shared `_runtime` module with validated primitive
//! aliases. Cross-module cycles are resolved by `model_rebuild()` calls in
//! `__init__.py` after every class is imported.

use indexmap::{IndexMap, IndexSet};
use inkgen_core::ir::{
    Derivation, ElementDefinition, ElementMax, ResourceDefinition, ResourceKind,
    expand_choice_elements,
};
use inkgen_core::{BackendError, GenerationOutput, PackageIr, extract_codes_from_valueset};
use serde::Serialize;
use tera::{Context, Tera};

use crate::invariants::{self, FieldIndex, InvariantCheck};
use crate::naming::{
    class_name, doc_line, is_valid_package_name, pascal_case, py_string_escape, safe_attr,
    safe_module, snake_case, upper_first,
};
use crate::primitives;

const MODULE_TEMPLATE: &str = include_str!("templates/module.py.tera");
const INIT_TEMPLATE: &str = include_str!("templates/init.py.tera");
const ENUMS_TEMPLATE: &str = include_str!("templates/enums.py.tera");

/// Generate the Python package for the whole IR, under `package_name`.
pub(crate) fn generate(
    ir: &PackageIr,
    package_name: &str,
) -> Result<GenerationOutput, BackendError> {
    if !is_valid_package_name(package_name) {
        return Err(BackendError::Message(format!(
            "invalid Python package name {package_name:?}: must be a Python identifier \
             (letters, digits, underscores; not starting with a digit or a keyword)"
        )));
    }
    let mut generator = Generator::new(ir);
    generator.collect_types();
    generator.collect_backbones();
    generator.collect_enums();

    let mut tera = Tera::default();
    tera.add_raw_templates([
        ("module.py.tera", MODULE_TEMPLATE),
        ("init.py.tera", INIT_TEMPLATE),
        ("enums.py.tera", ENUMS_TEMPLATE),
    ])
    .map_err(|e| BackendError::Message(format!("template registration failed: {e}")))?;

    let mut out = GenerationOutput::new();
    out.add_file(
        format!("{package_name}/_runtime.py"),
        primitives::RUNTIME_PY,
    );

    let mut module_imports: Vec<ModuleImport> = Vec::new();
    let mut all_classes: Vec<String> = Vec::new();
    for def in ir.types() {
        if !generator.is_emitted(def) {
            continue;
        }
        let info = generator.type_info(def);
        let module = generator.render_module(def, &info.class, &info.module, &tera)?;
        out.add_file(format!("{package_name}/{}.py", info.module), module);
        let mut classes = generator.module_classes(&info.class);
        classes.sort();
        all_classes.extend(classes.iter().cloned());
        module_imports.push(ModuleImport {
            module: info.module.clone(),
            classes,
        });
    }

    if !generator.enums.is_empty() {
        let enums_ctx = EnumsContext {
            package_id: ir.id.clone(),
            enums: generator
                .enums
                .values()
                .map(|e| EnumContext {
                    name: e.name.clone(),
                    codes_literal: e
                        .codes
                        .iter()
                        .map(|c| format!("\"{}\"", py_string_escape(c)))
                        .collect::<Vec<_>>()
                        .join(", "),
                })
                .collect(),
        };
        let rendered = tera
            .render(
                "enums.py.tera",
                &Context::from_serialize(&enums_ctx).unwrap(),
            )
            .map_err(|e| BackendError::Message(format!("enums template failed: {e}")))?;
        out.add_file(format!("{package_name}/enums.py"), rendered);
    }

    let mut all_names = all_classes.clone();
    all_names.extend(generator.enums.values().map(|e| e.name.clone()));
    all_names.sort();
    all_classes.sort();
    let init_ctx = InitContext {
        package_id: ir.id.clone(),
        enum_imports: generator.enums.values().map(|e| e.name.clone()).collect(),
        module_imports,
        all_names,
        models: all_classes,
    };
    let rendered = tera
        .render("init.py.tera", &Context::from_serialize(&init_ctx).unwrap())
        .map_err(|e| BackendError::Message(format!("init template failed: {e}")))?;
    out.add_file(format!("{package_name}/__init__.py"), rendered);

    Ok(out)
}

/// Where a generated class lives.
#[derive(Debug, Clone)]
struct TypeInfo {
    class: String,
    module: String,
}

/// An enumerated ValueSet binding lowered to a `Literal` alias.
#[derive(Debug, Clone)]
struct EnumInfo {
    name: String,
    codes: Vec<String>,
}

/// Per-module emission state: classes plus the imports they need.
#[derive(Debug, Default)]
struct ModuleBuilder {
    classes: Vec<ClassContext>,
    runtime_imports: IndexSet<String>,
    enum_imports: IndexSet<String>,
    model_imports: IndexMap<String, IndexSet<String>>,
    needs_any: bool,
    needs_literal: bool,
    needs_field: bool,
    needs_validator: bool,
}

impl ModuleBuilder {
    /// Reference a class from another module (resolved via `TYPE_CHECKING`
    /// import + `model_rebuild()` in `__init__.py`).
    fn reference(&mut self, info: &TypeInfo, current_module: &str) -> String {
        if info.module != current_module {
            self.model_imports
                .entry(info.module.clone())
                .or_default()
                .insert(info.class.clone());
        }
        info.class.clone()
    }

    /// Reference the `Element` datatype for a primitive companion, falling
    /// back to `Any` when the package does not define it.
    fn element_reference(&mut self, generator: &Generator<'_>, current_module: &str) -> String {
        match generator.types.get("Element") {
            Some(info) => self.reference(info, current_module),
            None => {
                self.needs_any = true;
                "Any".to_string()
            }
        }
    }
}

struct Generator<'a> {
    ir: &'a PackageIr,
    /// Type code / name / id -> generated class location.
    types: IndexMap<String, TypeInfo>,
    /// Structures emitted as classes, keyed by canonical URL.
    emitted: IndexSet<String>,
    /// ValueSet canonical URL -> `Literal` alias.
    enums: IndexMap<String, EnumInfo>,
    /// Backbone element path -> composite class name.
    backbones: IndexMap<String, String>,
    /// Names already taken in the package namespace (classes + enums).
    used_names: IndexSet<String>,
    /// Module stem -> owning class, for stem collision avoidance.
    stems: IndexMap<String, String>,
    /// Classes emitted per main class name, in emission order.
    module_class_lists: IndexMap<String, Vec<String>>,
}

impl<'a> Generator<'a> {
    fn new(ir: &'a PackageIr) -> Self {
        Self {
            ir,
            types: IndexMap::new(),
            emitted: IndexSet::new(),
            enums: IndexMap::new(),
            backbones: IndexMap::new(),
            used_names: IndexSet::new(),
            stems: IndexMap::new(),
            module_class_lists: IndexMap::new(),
        }
    }

    fn is_emitted(&self, def: &ResourceDefinition) -> bool {
        self.emitted.contains(&def.url)
    }

    fn type_info(&self, def: &ResourceDefinition) -> TypeInfo {
        let key = def.fhir_type.clone().unwrap_or_else(|| def.id.clone());
        self.types
            .get(&key)
            .cloned()
            .expect("emitted type registered")
    }

    /// Allocate a package-unique identifier.
    fn unique_name(&mut self, base: &str) -> String {
        let mut name = base.to_string();
        let mut suffix = 2;
        while self.used_names.contains(&name) {
            name = format!("{base}{suffix}");
            suffix += 1;
        }
        self.used_names.insert(name.clone());
        name
    }

    fn collect_types(&mut self) {
        for def in self.ir.types() {
            if def.kind == ResourceKind::PrimitiveType {
                continue;
            }
            if def.lineage.derivation == Some(Derivation::Constraint) {
                continue;
            }
            let base = class_name(def.name.as_deref().unwrap_or(&def.id));
            let class = self.unique_name(&base);
            let stem_base = safe_module(&snake_case(&class));
            let mut module = stem_base.clone();
            let mut suffix = 2;
            while self.stems.contains_key(&module) {
                module = format!("{stem_base}_{suffix}");
                suffix += 1;
            }
            self.stems.insert(module.clone(), class.clone());
            let info = TypeInfo { class, module };
            for key in [
                def.fhir_type.as_deref(),
                def.name.as_deref(),
                Some(def.id.as_str()),
            ]
            .into_iter()
            .flatten()
            {
                self.types
                    .entry(key.to_string())
                    .or_insert_with(|| info.clone());
            }
            self.emitted.insert(def.url.clone());
        }
    }

    fn collect_backbones(&mut self) {
        for def in self.ir.types() {
            if !self.is_emitted(def) {
                continue;
            }
            let root = self.type_info(def).class;
            let elements = top_level_elements(def);
            self.walk_backbones(&elements, &root);
        }
    }

    fn walk_backbones(&mut self, elements: &[ElementDefinition], root: &str) {
        for element in elements {
            if element.children.is_empty() {
                continue;
            }
            let composite = composite_name(root, &element.path);
            let name = self.unique_name(&composite);
            self.backbones.insert(element.path.clone(), name);
            self.walk_backbones(&element.children, root);
        }
    }

    fn collect_enums(&mut self) {
        for def in self.ir.types() {
            if !self.is_emitted(def) {
                continue;
            }
            let elements = top_level_elements(def);
            self.walk_enums(&elements);
        }
    }

    fn walk_enums(&mut self, elements: &[ElementDefinition]) {
        for element in elements {
            if let Some(binding) = &element.binding
                && let inkgen_core::ir::BindingLowering::Enumerated {
                    value_set_url,
                    type_name,
                } = self.ir.lower_binding(binding)
                && !self.enums.contains_key(&value_set_url)
                && let Some(vs) = self.ir.value_sets.get(&value_set_url)
                && let Ok(resolved) = extract_codes_from_valueset(vs, None, &self.ir.value_sets)
            {
                let mut seen = IndexSet::new();
                let codes: Vec<String> = resolved
                    .codes
                    .into_iter()
                    .filter(|c| seen.insert(c.clone()))
                    .collect();
                if !codes.is_empty() {
                    let name = self.unique_name(&pascal_case(&type_name));
                    self.enums
                        .insert(value_set_url.clone(), EnumInfo { name, codes });
                }
            }
            self.walk_enums(&element.children);
        }
    }

    fn module_classes(&self, main_class: &str) -> Vec<String> {
        self.module_class_lists
            .get(main_class)
            .cloned()
            .unwrap_or_default()
    }

    fn render_module(
        &mut self,
        def: &ResourceDefinition,
        class: &str,
        module: &str,
        tera: &Tera,
    ) -> Result<String, BackendError> {
        let mut builder = ModuleBuilder::default();
        let elements = top_level_elements(def);

        // Backbone classes first (post-order), then the main class.
        self.emit_backbone_classes(&elements, class, module, &mut builder);

        let mut fields = FieldBatch::default();
        self.build_fields(&elements, module, &mut builder, &mut fields);

        let mut checks = choice_checks(&fields);
        checks.extend(invariants::lower_xor_exists_checks(
            &def.invariants,
            &fields.index,
        ));

        let resource_type = match def.kind {
            ResourceKind::Resource | ResourceKind::Logical => {
                Some(def.fhir_type.clone().unwrap_or_else(|| def.id.clone()))
            }
            _ => None,
        };
        let abstract_base = matches!(class, "Resource" | "DomainResource");
        if resource_type.is_some() {
            builder.needs_field = true;
            if !abstract_base {
                builder.needs_literal = true;
            }
        }
        if !checks.is_empty() {
            builder.needs_validator = true;
        }

        let field_lines = fields.lines;
        let doc = def
            .title
            .as_deref()
            .or(def.description.as_deref())
            .map(doc_line);
        let empty =
            doc.is_none() && resource_type.is_none() && field_lines.is_empty() && checks.is_empty();
        let main = ClassContext {
            name: class.to_string(),
            doc,
            resource_type: resource_type.map(|rt| ResourceTypeContext {
                value: rt,
                literal: !abstract_base,
            }),
            fields: field_lines,
            checks: checks
                .into_iter()
                .map(|c| CheckContext {
                    condition: c.condition,
                    message: c.message,
                })
                .collect(),
            empty,
        };
        builder.classes.push(main);
        self.module_class_lists.insert(
            class.to_string(),
            builder.classes.iter().map(|c| c.name.clone()).collect(),
        );

        let mut typing_imports = Vec::new();
        if builder.needs_any {
            typing_imports.push("Any");
        }
        if builder.needs_literal {
            typing_imports.push("Literal");
        }
        if builder.needs_validator {
            typing_imports.push("Self");
        }
        if !builder.model_imports.is_empty() {
            typing_imports.push("TYPE_CHECKING");
        }
        let mut pydantic_imports = Vec::new();
        if builder.needs_field {
            pydantic_imports.push("Field");
        }
        if builder.needs_validator {
            pydantic_imports.push("model_validator");
        }
        let mut runtime_imports = vec!["FhirModel".to_string()];
        let mut aliases: Vec<String> = builder.runtime_imports.iter().cloned().collect();
        aliases.sort();
        runtime_imports.extend(aliases);
        let mut enum_imports: Vec<String> = builder.enum_imports.iter().cloned().collect();
        enum_imports.sort();
        let model_imports: Vec<ModuleImport> = builder
            .model_imports
            .iter()
            .map(|(m, classes)| {
                let mut classes: Vec<String> = classes.iter().cloned().collect();
                classes.sort();
                ModuleImport {
                    module: m.clone(),
                    classes,
                }
            })
            .collect();

        let ctx = ModuleContext {
            package_id: self.ir.id.clone(),
            typing_imports,
            pydantic_imports,
            runtime_imports,
            enum_imports,
            model_imports,
            classes: builder.classes,
        };
        tera.render("module.py.tera", &Context::from_serialize(&ctx).unwrap())
            .map_err(|e| BackendError::Message(format!("module template failed for {class}: {e}")))
    }

    fn emit_backbone_classes(
        &mut self,
        elements: &[ElementDefinition],
        root: &str,
        module: &str,
        builder: &mut ModuleBuilder,
    ) {
        for element in elements {
            if element.children.is_empty() {
                continue;
            }
            self.emit_backbone_classes(&element.children, root, module, builder);
            let name = self
                .backbones
                .get(&element.path)
                .cloned()
                .unwrap_or_else(|| composite_name(root, &element.path));
            let mut fields = FieldBatch::default();
            self.build_fields(&element.children, module, builder, &mut fields);
            let checks = choice_checks(&fields);
            if !checks.is_empty() {
                builder.needs_validator = true;
            }
            let doc = element
                .short
                .as_deref()
                .or(element.definition.as_deref())
                .map(doc_line);
            let empty = doc.is_none() && fields.lines.is_empty() && checks.is_empty();
            builder.classes.push(ClassContext {
                name,
                doc,
                resource_type: None,
                fields: fields.lines,
                checks: checks
                    .into_iter()
                    .map(|c| CheckContext {
                        condition: c.condition,
                        message: c.message,
                    })
                    .collect(),
                empty,
            });
        }
    }

    /// Render field lines (plus primitive companions) for one class body and
    /// record the wire-name index used by invariant lowering.
    fn build_fields(
        &mut self,
        elements: &[ElementDefinition],
        module: &str,
        builder: &mut ModuleBuilder,
        batch: &mut FieldBatch,
    ) {
        // Choice groups keyed by the original `[x]` element id.
        let mut groups: IndexMap<String, ChoiceGroup> = IndexMap::new();
        for element in elements {
            if element.is_choice() && !element.types.is_empty() {
                let wire = last_segment(&element.path);
                groups.insert(
                    element.id.clone(),
                    ChoiceGroup {
                        id: element.id.clone(),
                        base: wire.trim_end_matches("[x]").to_string(),
                        min: element.cardinality.min,
                        variants: Vec::new(),
                    },
                );
            }
        }

        let expanded = expand_choice_elements(elements.to_vec());
        for element in &expanded {
            let wire = last_segment(&element.path).to_string();
            let attr = safe_attr(&snake_case(&wire));
            let inner = self.annotation(element, module, builder);
            let is_list = matches!(element.cardinality.max, ElementMax::Unbounded)
                || matches!(element.cardinality.max, ElementMax::Finite(n) if n > 1);
            let required = element.cardinality.min >= 1;

            let base = if is_list {
                format!("list[{inner}]")
            } else {
                inner.clone()
            };
            let annotation = if required {
                base
            } else {
                format!("{base} | None")
            };
            let alias = (attr != wire).then(|| format!("alias=\"{}\"", py_string_escape(&wire)));
            let rhs = match (is_list, required) {
                (true, true) => {
                    builder.needs_field = true;
                    let mut args = vec![format!("min_length={}", element.cardinality.min)];
                    args.extend(alias);
                    Some(format!("Field({})", args.join(", ")))
                }
                (true, false) => optional_rhs(alias, builder),
                (false, true) => alias.map(|a| {
                    builder.needs_field = true;
                    format!("Field({a})")
                }),
                (false, false) => optional_rhs(alias, builder),
            };
            let line = match rhs {
                Some(rhs) => format!("{attr}: {annotation} = {rhs}"),
                None => format!("{attr}: {annotation}"),
            };
            batch.lines.push(line);

            // Primitive-extension companion (`_x` on the wire). Pydantic
            // reserves leading-underscore attributes, so the field is named
            // `<attr>_ext` with an explicit alias.
            if element.types.len() == 1 && primitives::is_primitive(&element.types[0].code) {
                let element_ty = builder.element_reference(self, module);
                let comp_annotation = if is_list {
                    format!("list[{element_ty} | None] | None")
                } else {
                    format!("{element_ty} | None")
                };
                let comp_wire = format!("_{wire}");
                builder.needs_field = true;
                batch.lines.push(format!(
                    "{}_ext: {comp_annotation} = Field(default=None, alias=\"{}\")",
                    attr,
                    py_string_escape(&comp_wire)
                ));
            }

            // Record the field for choice/invariant lowering.
            let presence = if is_list {
                format!("bool(self.{attr})")
            } else {
                format!("self.{attr} is not None")
            };
            if let Some(group) = groups.get_mut(&element.id) {
                group.variants.push((wire.clone(), attr.clone(), presence));
            } else if is_list {
                batch.index.arrays.insert(wire, attr);
            } else {
                batch.index.scalars.insert(wire, attr);
            }
        }

        for group in groups.into_values() {
            batch.index.choices.insert(
                group.base.clone(),
                group
                    .variants
                    .iter()
                    .map(|(_, attr, _)| attr.clone())
                    .collect(),
            );
            batch.groups.push(group);
        }
    }

    /// Map one element to its inner Python annotation (before cardinality
    /// wrapping), registering any imports it needs.
    fn annotation(
        &mut self,
        element: &ElementDefinition,
        module: &str,
        builder: &mut ModuleBuilder,
    ) -> String {
        if let Some(reference) = &element.content_reference {
            let path = reference.trim_start_matches('#');
            if let Some(name) = self.backbones.get(path) {
                return name.clone();
            }
            builder.needs_any = true;
            return "Any".to_string();
        }
        if !element.children.is_empty()
            && let Some(name) = self.backbones.get(&element.path)
        {
            return name.clone();
        }
        if element.types.len() == 1 {
            return self.single_type(element, &element.types[0].code, module, builder);
        }
        if element.types.len() > 1 {
            let mut parts: Vec<String> = Vec::new();
            for ty in &element.types {
                let part = self.single_type(element, &ty.code, module, builder);
                if !parts.contains(&part) {
                    parts.push(part);
                }
            }
            return parts.join(" | ");
        }
        builder.needs_any = true;
        "Any".to_string()
    }

    fn single_type(
        &mut self,
        element: &ElementDefinition,
        code: &str,
        module: &str,
        builder: &mut ModuleBuilder,
    ) -> String {
        if let Some(binding) = &element.binding
            && let inkgen_core::ir::BindingLowering::Enumerated { value_set_url, .. } =
                self.ir.lower_binding(binding)
            && let Some(info) = self.enums.get(&value_set_url)
        {
            builder.enum_imports.insert(info.name.clone());
            return info.name.clone();
        }
        if let Some(mapping) = primitives::primitive(code) {
            if let Some(alias) = mapping.runtime_alias {
                builder.runtime_imports.insert(alias.to_string());
            }
            return mapping.annotation.to_string();
        }
        if let Some(info) = self.types.get(code) {
            return builder.reference(info, module);
        }
        builder.needs_any = true;
        "Any".to_string()
    }
}

/// Render the RHS for an optional field: `Field(default=None, alias=...)`
/// when an alias is needed, plain `None` otherwise.
fn optional_rhs(alias: Option<String>, builder: &mut ModuleBuilder) -> Option<String> {
    match alias {
        Some(a) => {
            builder.needs_field = true;
            Some(format!("Field(default=None, {a})"))
        }
        None => Some("None".to_string()),
    }
}

/// Fields rendered for one class plus the lookup tables for constraints.
#[derive(Debug, Default)]
struct FieldBatch {
    lines: Vec<String>,
    index: FieldIndex,
    groups: Vec<ChoiceGroup>,
}

/// One choice (`[x]`) group with its emitted variants.
#[derive(Debug, Clone)]
struct ChoiceGroup {
    id: String,
    base: String,
    min: u32,
    /// (wire name, attribute, presence expression) per variant.
    variants: Vec<(String, String, String)>,
}

/// At-most-one / exactly-one checks for every choice group of a class.
fn choice_checks(batch: &FieldBatch) -> Vec<InvariantCheck> {
    let mut checks = Vec::new();
    for group in &batch.groups {
        if group.variants.is_empty() {
            continue;
        }
        let wires = group
            .variants
            .iter()
            .map(|(wire, _, _)| wire.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let presence = group
            .variants
            .iter()
            .map(|(_, _, expr)| expr.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if group.variants.len() > 1 {
            checks.push(InvariantCheck {
                condition: format!("sum(({presence})) > 1"),
                message: format!(
                    "\"{}\"",
                    py_string_escape(&format!(
                        "{}: at most one of {wires} may be present",
                        group.id
                    ))
                ),
            });
        }
        if group.min >= 1 {
            let condition = if group.variants.len() == 1 {
                format!("not ({presence})")
            } else {
                format!("sum(({presence})) == 0")
            };
            checks.push(InvariantCheck {
                condition,
                message: format!(
                    "\"{}\"",
                    py_string_escape(&format!(
                        "{}: exactly one of {wires} must be present",
                        group.id
                    ))
                ),
            });
        }
    }
    checks
}

/// Composite backbone class name: root class + upper-cased path segments
/// (`Patient.contact` -> `PatientContact`).
fn composite_name(root: &str, path: &str) -> String {
    let mut name = root.to_string();
    for segment in path.split('.').skip(1) {
        name.push_str(&upper_first(&segment.replace("[x]", "")));
    }
    name
}

fn last_segment(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or(path)
}

/// Top-level field elements of a structure: the root element's children when
/// the IR carries a tree, otherwise the flat two-segment paths.
fn top_level_elements(def: &ResourceDefinition) -> Vec<ElementDefinition> {
    let root = def
        .elements
        .iter()
        .find(|elem| elem.path == def.id)
        .or_else(|| {
            def.name
                .as_ref()
                .and_then(|name| def.elements.iter().find(|elem| elem.path == *name))
        })
        .or_else(|| def.elements.iter().find(|elem| !elem.path.contains('.')));

    if let Some(root) = root
        && !root.children.is_empty()
    {
        let mut elements = Vec::new();
        for child in &root.children {
            if child.is_choice() && !child.children.is_empty() {
                elements.extend(child.children.iter().cloned());
            } else {
                elements.push(child.clone());
            }
        }
        elements.sort_by(|a, b| a.path.cmp(&b.path));
        return elements;
    }

    let mut elements: Vec<ElementDefinition> = def
        .elements
        .iter()
        .filter(|elem| elem.path.split('.').count() == 2)
        .cloned()
        .collect();
    elements.sort_by(|a, b| a.path.cmp(&b.path));
    elements
}

// Template contexts.

#[derive(Debug, Serialize)]
struct ModuleContext {
    package_id: String,
    typing_imports: Vec<&'static str>,
    pydantic_imports: Vec<&'static str>,
    runtime_imports: Vec<String>,
    enum_imports: Vec<String>,
    model_imports: Vec<ModuleImport>,
    classes: Vec<ClassContext>,
}

#[derive(Debug, Serialize)]
struct ModuleImport {
    module: String,
    classes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ClassContext {
    name: String,
    doc: Option<String>,
    resource_type: Option<ResourceTypeContext>,
    fields: Vec<String>,
    checks: Vec<CheckContext>,
    empty: bool,
}

#[derive(Debug, Serialize)]
struct ResourceTypeContext {
    value: String,
    literal: bool,
}

#[derive(Debug, Serialize)]
struct CheckContext {
    condition: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct InitContext {
    package_id: String,
    enum_imports: Vec<String>,
    module_imports: Vec<ModuleImport>,
    all_names: Vec<String>,
    models: Vec<String>,
}

#[derive(Debug, Serialize)]
struct EnumsContext {
    package_id: String,
    enums: Vec<EnumContext>,
}

#[derive(Debug, Serialize)]
struct EnumContext {
    name: String,
    codes_literal: String,
}

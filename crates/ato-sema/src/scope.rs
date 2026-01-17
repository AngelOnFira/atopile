//! Scope management for name resolution.
//!
//! Scopes track name bindings during semantic analysis. They form a tree
//! structure where child scopes can look up names in their parent scopes.

use ato_ir::{FieldId, ModuleId};
use ato_lexer::Span;
use std::collections::HashMap;

/// A binding in a scope.
#[derive(Debug, Clone)]
pub enum Binding {
    /// A module binding (module, component, interface).
    Module(ModuleId),
    /// A field binding (parameter, pin, signal, instance).
    Field(FieldId),
    /// An imported name (not yet resolved to a module).
    Import {
        /// The original qualified name from the import.
        name: String,
        /// The file path (if `from "path"` was used).
        from_path: Option<String>,
    },
    /// A loop variable binding.
    LoopVariable {
        /// The instance field being iterated.
        source: FieldId,
        /// Current index in the iteration (set during lowering).
        index: Option<u32>,
    },
}

impl Binding {
    /// Get the module ID if this is a module binding.
    pub fn as_module(&self) -> Option<ModuleId> {
        match self {
            Binding::Module(id) => Some(*id),
            _ => None,
        }
    }

    /// Get the field ID if this is a field binding.
    pub fn as_field(&self) -> Option<FieldId> {
        match self {
            Binding::Field(id) => Some(*id),
            _ => None,
        }
    }
}

/// A scope that tracks name bindings.
#[derive(Debug, Clone)]
pub struct Scope {
    /// The parent scope (if any).
    parent: Option<Box<Scope>>,

    /// Name bindings in this scope.
    bindings: HashMap<String, (Binding, Option<Span>)>,

    /// The current module context (for resolving `self`).
    current_module: Option<ModuleId>,
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

impl Scope {
    /// Create a new empty scope.
    pub fn new() -> Self {
        Self {
            parent: None,
            bindings: HashMap::new(),
            current_module: None,
        }
    }

    /// Create a child scope.
    pub fn child(&self) -> Self {
        Self {
            parent: Some(Box::new(self.clone())),
            bindings: HashMap::new(),
            current_module: self.current_module,
        }
    }

    /// Create a child scope with a new current module.
    pub fn child_with_module(&self, module_id: ModuleId) -> Self {
        Self {
            parent: Some(Box::new(self.clone())),
            bindings: HashMap::new(),
            current_module: Some(module_id),
        }
    }

    /// Set the current module context.
    pub fn set_current_module(&mut self, module_id: ModuleId) {
        self.current_module = Some(module_id);
    }

    /// Get the current module context.
    pub fn current_module(&self) -> Option<ModuleId> {
        self.current_module
    }

    /// Define a name in this scope.
    pub fn define(&mut self, name: impl Into<String>, binding: Binding, span: Option<Span>) {
        self.bindings.insert(name.into(), (binding, span));
    }

    /// Define a module in this scope.
    pub fn define_module(&mut self, name: impl Into<String>, module_id: ModuleId, span: Option<Span>) {
        self.define(name, Binding::Module(module_id), span);
    }

    /// Define a field in this scope.
    pub fn define_field(&mut self, name: impl Into<String>, field_id: FieldId, span: Option<Span>) {
        self.define(name, Binding::Field(field_id), span);
    }

    /// Define an import in this scope.
    pub fn define_import(
        &mut self,
        name: impl Into<String>,
        qualified_name: impl Into<String>,
        from_path: Option<String>,
        span: Option<Span>,
    ) {
        self.define(
            name,
            Binding::Import {
                name: qualified_name.into(),
                from_path,
            },
            span,
        );
    }

    /// Look up a name in this scope or parent scopes.
    pub fn lookup(&self, name: &str) -> Option<&Binding> {
        if let Some((binding, _)) = self.bindings.get(name) {
            return Some(binding);
        }

        if let Some(parent) = &self.parent {
            return parent.lookup(name);
        }

        None
    }

    /// Look up a name and return its span too.
    pub fn lookup_with_span(&self, name: &str) -> Option<(&Binding, Option<Span>)> {
        if let Some((binding, span)) = self.bindings.get(name) {
            return Some((binding, *span));
        }

        if let Some(parent) = &self.parent {
            return parent.lookup_with_span(name);
        }

        None
    }

    /// Check if a name is defined in this scope (not checking parents).
    pub fn is_defined_locally(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    /// Check if a name is defined anywhere in the scope chain.
    pub fn is_defined(&self, name: &str) -> bool {
        self.lookup(name).is_some()
    }

    /// Get all local bindings (not from parent scopes).
    pub fn local_bindings(&self) -> impl Iterator<Item = (&str, &Binding)> {
        self.bindings.iter().map(|(k, (v, _))| (k.as_str(), v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ato_ir::{Design, ModuleKind};

    #[test]
    fn test_scope_define_and_lookup() {
        let mut design = Design::new();
        let module_id = design.create_module("Foo", ModuleKind::Module);

        let mut scope = Scope::new();
        scope.define_module("Foo", module_id, None);

        let binding = scope.lookup("Foo").unwrap();
        assert!(matches!(binding, Binding::Module(_)));

        assert!(scope.lookup("Bar").is_none());
    }

    #[test]
    fn test_scope_inheritance() {
        let mut design = Design::new();
        let parent_id = design.create_module("Parent", ModuleKind::Module);
        let child_id = design.create_module("Child", ModuleKind::Module);

        let mut parent_scope = Scope::new();
        parent_scope.define_module("Parent", parent_id, None);

        let mut child_scope = parent_scope.child();
        child_scope.define_module("Child", child_id, None);

        // Child can see its own binding
        assert!(child_scope.lookup("Child").is_some());

        // Child can see parent binding
        assert!(child_scope.lookup("Parent").is_some());

        // Parent cannot see child binding
        assert!(parent_scope.lookup("Child").is_none());
    }

    #[test]
    fn test_scope_shadowing() {
        let mut design = Design::new();
        let foo0 = design.create_module("Foo0", ModuleKind::Module);
        let foo1 = design.create_module("Foo1", ModuleKind::Module);

        let mut parent = Scope::new();
        parent.define_module("Foo", foo0, None);

        let mut child = parent.child();
        child.define_module("Foo", foo1, None);

        // Child sees its own binding
        if let Some(Binding::Module(id)) = child.lookup("Foo") {
            assert_eq!(id.index(), foo1.index());
        } else {
            panic!("Expected module binding");
        }
    }

    #[test]
    fn test_scope_current_module() {
        let mut design = Design::new();
        let mod1 = design.create_module("Mod1", ModuleKind::Module);
        let mod2 = design.create_module("Mod2", ModuleKind::Module);

        let mut scope = Scope::new();
        assert!(scope.current_module().is_none());

        scope.set_current_module(mod1);
        assert_eq!(scope.current_module(), Some(mod1));

        let child = scope.child();
        assert_eq!(child.current_module(), Some(mod1));

        let child2 = scope.child_with_module(mod2);
        assert_eq!(child2.current_module(), Some(mod2));
    }
}

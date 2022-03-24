use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

use serde::Serialize;

use crate::cidl::ast::Method;

#[derive(Serialize, Debug)]
pub struct SymbolTable(pub HashMap<String, RefCell<Symbol>>);

impl SymbolTable {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub enum SymbolType {
    Interface,
    Class,
}

#[derive(Serialize, Debug, Clone)]
pub struct Ancestor {
    pub name: String,
    pub offset: u32,
}

#[derive(Serialize, Debug)]
pub struct Symbol {
    pub name: String,
    pub symbol_type: SymbolType,
    pub parents: Vec<String>,
    pub ancestors: Vec<Ancestor>,
    pub size: u32,
    pub uuid: String,
    pub methods: Vec<Method>,

    pub internal: Internal,
}

impl Symbol {
    pub fn new(
        name: String,
        symbol_type: SymbolType,
        parents: Vec<String>,
        methods: Vec<Method>,
        attributes: &HashMap<String, String>,
    ) -> Self {
        if parents.len() > 1 && symbol_type == SymbolType::Interface {
            panic!("Interface cannot have multiple parents in COM: {}", name);
        }

        Self {
            name,
            symbol_type,
            parents,
            ancestors: vec![],
            size: 0,
            methods,
            uuid: attributes
                .get("uuid")
                .unwrap_or(&"".to_string())
                .to_string(),
            internal: Internal::new(),
        }
    }

    pub fn analyze_ancestors(&mut self, symbols: &SymbolTable) {
        if self.internal.ancestor_analysis == AnalyzeState::Completed {
            return;
        } else if self.internal.ancestor_analysis == AnalyzeState::Analyzing {
            panic!(
                "Cannot analyze ancestors: circular inheritance detected: {}",
                self.name
            );
        }

        self.internal.ancestor_analysis = AnalyzeState::Analyzing;

        let mut ancestors_set = HashSet::new();
        let mut offset = 0;

        let mut try_push_ancestor = |name: &String, offset| {
            if ancestors_set.contains(name) {
                return;
            }

            ancestors_set.insert(name.clone());
            self.ancestors.push(Ancestor {
                name: name.clone(),
                offset,
            });
        };

        for parent in &self.parents {
            let iface = symbols.0.get(parent).unwrap();
            let mut iface_ref = iface.borrow_mut();
            if iface_ref.symbol_type == SymbolType::Interface {
                if iface_ref.internal.ancestor_analysis != AnalyzeState::Completed {
                    iface_ref.analyze_ancestors(symbols);
                }

                try_push_ancestor(&iface_ref.name, offset);

                for a in &iface_ref.ancestors {
                    try_push_ancestor(&a.name, a.offset + offset);
                }
            }

            offset += 1;
        }

        self.internal.ancestor_analysis = AnalyzeState::Completed;
    }
}

#[derive(Serialize, Debug)]
pub struct Internal {
    pub ancestor_analysis: AnalyzeState,
}

impl Internal {
    pub fn new() -> Self {
        Self {
            ancestor_analysis: AnalyzeState::NotStart,
        }
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub enum AnalyzeState {
    NotStart,
    Analyzing,
    Completed,
}

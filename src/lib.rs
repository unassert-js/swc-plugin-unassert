use std::collections::HashSet;
use swc_core::common::util::take::Take;
use swc_core::ecma::atoms::JsWord;
use swc_core::ecma::ast::{
    Id,
    Program,
    ImportDecl,
    ImportSpecifier,
    ImportDefaultSpecifier,
    ImportStarAsSpecifier,
    ImportNamedSpecifier,
    ModuleItem,
    ModuleDecl,
    CallExpr,
    Callee,
    Expr,
    Stmt,
    ExprStmt,
    MemberExpr,
};
use swc_core::ecma::visit::{
    as_folder,
    FoldWith,
    VisitMut, VisitMutWith
};
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};

pub struct TransformVisitor {
    target_variables: HashSet<Id>,
    target_modules: HashSet<JsWord>,
}

impl Default for TransformVisitor {
    fn default() -> Self {
        Self {
            target_variables: HashSet::new(),
            target_modules: [
                "node:assert",
                "node:assert/strict",
                "assert",
                "assert/strict",
            ].iter().map(|s| JsWord::from(*s)).collect(),
        }
    }
}

impl VisitMut for TransformVisitor {
    fn visit_mut_import_decl(&mut self, n: &mut ImportDecl) {
        if self.target_modules.contains(&n.src.value) {
            for s in &mut n.specifiers {
                match s {
                    ImportSpecifier::Default(ImportDefaultSpecifier { local, .. }) => {
                        self.target_variables.insert(local.to_id());
                    },
                    ImportSpecifier::Namespace(ImportStarAsSpecifier { local, .. }) => {
                        self.target_variables.insert(local.to_id());
                    },
                    ImportSpecifier::Named(ImportNamedSpecifier { local, .. }) => {
                        self.target_variables.insert(local.to_id());
                    },
                }
            }
            n.take();
        } else {
            n.visit_mut_children_with(self);
        }
    }

    fn visit_mut_module_items(&mut self, n: &mut Vec<ModuleItem>) {
        n.visit_mut_children_with(self);
        n.retain(|s| {
            match s {
                ModuleItem::ModuleDecl(ModuleDecl::Import(decl)) => *decl != ImportDecl::dummy(),
                _ => true,
            }
        });
    }

    fn visit_mut_call_expr(&mut self, n: &mut CallExpr) {
        if self.target_variables.is_empty() {
            n.visit_mut_children_with(self);
            return;
        }
        let to_be_removed = match n.callee {
            Callee::Expr(ref expr) => {
                match expr.as_ref() {
                    Expr::Member(MemberExpr{ obj, .. }) => {
                        match obj.as_ref() {
                            Expr::Ident(ref obj_ident) => self.target_variables.contains(&obj_ident.to_id()),
                            _ => false
                        }
                    },
                    Expr::Ident(ref ident) => self.target_variables.contains(&ident.to_id()),
                    _ => false
                }
            },
            _ => false
        };
        if to_be_removed {
            n.take();
        } else {
            n.visit_mut_children_with(self);
        }
    }

    fn visit_mut_stmt(&mut self, n: &mut Stmt) {
        n.visit_mut_children_with(self);
        if let Stmt::Expr(ExprStmt{ expr, ..}) = n {
            if let Expr::Call(call_expr) = expr.as_ref() {
                if *call_expr == CallExpr::dummy() {
                    n.take();
                }
            }
        }
    }
}

/// An example plugin function with macro support.
/// `plugin_transform` macro interop pointers into deserialized structs, as well
/// as returning ptr back to host.
///
/// It is possible to opt out from macro by writing transform fn manually
/// if plugin need to handle low-level ptr directly via
/// `__transform_plugin_process_impl(
///     ast_ptr: *const u8, ast_ptr_len: i32,
///     unresolved_mark: u32, should_enable_comments_proxy: i32) ->
///     i32 /*  0 for success, fail otherwise.
///             Note this is only for internal pointer interop result,
///             not actual transform result */`
///
/// This requires manual handling of serialization / deserialization from ptrs.
/// Refer swc_plugin_macro to see how does it work internally.
#[plugin_transform]
pub fn process_transform(program: Program, _metadata: TransformPluginProgramMetadata) -> Program {
    program.fold_with(&mut as_folder(TransformVisitor::default()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use swc_ecma_transforms_testing::test_fixture;
    use swc_core::ecma::transforms::testing::FixtureTestConfig;
    use swc_core::ecma::visit::as_folder;
    use swc_ecma_parser::{EsConfig, Syntax};
    use super::TransformVisitor;

    #[testing::fixture("tests/fixtures/*/input.mjs")]
    fn test_with_fixtures(input: PathBuf) {
        let output = input.with_file_name("expected.mjs");
        test_fixture(
            Syntax::Es(EsConfig::default()),
            &|_t| {
                as_folder(TransformVisitor::default())
            },
            &input,
            &output,
            FixtureTestConfig {
                allow_error: true,
                ..Default::default()
            },
        );
    }
}

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate syntax;
#[macro_use] extern crate rustc;
extern crate rustc_errors;
extern crate getopts;

use rustc::hir::*;
use rustc::hir::def::Def;
use syntax::ast::NodeId;
use syntax::codemap::Span;

#[allow(unused_imports)]
use rustc_driver::{driver,CompilerCalls,Compilation};
use rustc::session::Session;
use rustc::ty;

const ERROR_MSG: &'static str = "Nice try! Anonymous lifetimes may not be used for this assignment";

struct NoElision;

impl<'a> CompilerCalls<'a> for NoElision {
    fn build_controller(
        &mut self,
        _: &Session,
        _: &getopts::Matches
    ) -> driver::CompileController<'a> {
        let mut control = driver::CompileController::basic();
        //control.after_analysis.stop = Compilation::Stop;
        control.after_analysis.callback = Box::new(|state| {
            if !state.session.has_errors() {
                let mut visitor = ElisionVisitor { session: state.session, tcx: state.tcx.unwrap() };
                let hir = state.hir_crate.unwrap();
                hir.visit_all_items(&mut visitor);
            }
        });
        control
    }
}

struct ElisionVisitor<'v, 't: 'v> {
    tcx: ty::TyCtxt<'v, 't, 't>,
    session: &'v Session,
}

impl<'v, 't> intravisit::Visitor<'v> for ElisionVisitor<'v, 't> {
    fn visit_fn(&mut self, fk: intravisit::FnKind<'v>, fd: &'v FnDecl, _: &'v Block, _: Span, _: NodeId) {
        use rustc::hir::intravisit::FnKind::*;
        let mut visitor = RefVisitor::new(self.session, self.tcx);

        let generics = match fk {
            ItemFn(_, generics, _, _, _, _, _) => Some(generics),
            Method(_, sig, _, _) => Some(&sig.generics),
            Closure(_) => None,
        };

        generics.map(|generics| {

            // extract lifetimes in input argument types
            for arg in &fd.inputs {
                visitor.visit_ty(&arg.ty);
            }
            // extract lifetimes in output type
            if let Return(ref ty) = fd.output {
                visitor.visit_ty(ty);
            }

            visitor.visit_where_clause(&generics.where_clause);
        });
    }
}

fn main() {
    let args : Vec<_> = std::env::args().collect();
    let mut analyzer = NoElision;
    rustc_driver::run_compiler(&args, &mut analyzer);
}


// The below is taken (and modified) from clippy

/// A visitor usable for `rustc_front::visit::walk_ty()`.
struct RefVisitor<'v, 't: 'v> {
    session: &'v Session,
    tcx: ty::TyCtxt<'v, 't, 't>,
}

impl<'v, 't> RefVisitor<'v, 't> {
    fn new(session: &'v Session, tcx: ty::TyCtxt<'v, 't, 't>) -> RefVisitor<'v, 't> {
        RefVisitor {
            session: session,
            tcx: tcx,
        }
    }

    fn visit_where_clause(&mut self, where_clause: &'v WhereClause) {
        for predicate in &where_clause.predicates {
            match *predicate {
                WherePredicate::RegionPredicate(_) => {
                    // There can be no unnamed lifetimes in a region predicate
                }
                WherePredicate::BoundPredicate(ref pred) => {
                    // walk the type F, it may not contain LT refs
                    intravisit::walk_ty(self, &pred.bounded_ty);
                    for bound in pred.bounds.iter() {
                        intravisit::walk_ty_param_bound(self, bound);
                    }
                }
                WherePredicate::EqPredicate(ref pred) => {
                    intravisit::walk_ty(self, &pred.ty);
                }
            }
        }
    }

    fn visit_path_(&mut self, path: &Path, ty: &Ty) {
        let last_path_segment = path.segments.last().map(|s| &s.parameters);
        if let Some(&AngleBracketedParameters(ref params)) = last_path_segment {
            // One might ask, is it possible to have some named lifetimes, and some unnamed?
            // Nope, not in paths.
            if params.lifetimes.is_empty() {
                if let Some(def) = self.tcx.def_map.borrow().get(&ty.id).map(|r| r.full_def()) {
                    match def {
                        Def::TyAlias(def_id) |
                        Def::Struct(def_id) => {
                            let type_scheme = self.tcx.lookup_item_type(def_id);
                            if type_scheme.generics.regions.as_slice().len() > 0 {
                                self.session.span_err(path.span, ERROR_MSG);
                            }
                        }
                        Def::Trait(def_id) => {
                            let trait_def = self.tcx.trait_defs.borrow()[&def_id];
                            if trait_def.generics.regions.as_slice().len() > 0 {
                                self.session.span_err(path.span, ERROR_MSG);
                            }
                        }
                        _ => (),
                    }
                }
            }
        }
    }
}

impl<'v, 't> intravisit::Visitor<'v> for RefVisitor<'v, 't> {

    fn visit_ty(&mut self, ty: &'v Ty) {
        match ty.node {
            TyRptr(None, _) => {
                self.session.span_err(ty.span, ERROR_MSG);
            }
            TyPath(_, ref path) => {
                self.visit_path_(path, ty);
            }
            _ => (),
        }
        intravisit::walk_ty(self, ty);
    }
}


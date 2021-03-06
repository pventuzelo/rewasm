use std::collections::{HashMap, HashSet};

use crate::ssa::{Cond, Expr, Stmt, Var};

pub fn apply(code: &mut Vec<Stmt>, expr_map: &mut HashMap<u32, Expr>) {
    let mut count = HashMap::<u32, u32>::new();
    for_each_mapped_expr(code, &mut |index| *count.entry(index).or_default() += 1);

    let mut sidefx_conds = count
        .into_iter()
        .filter_map(|(i, count)| if count > 1 { Some(i) } else { None })
        .collect();

    hoist_conds(code, &mut sidefx_conds, expr_map);
}

fn for_each_mapped_expr(code: &[Stmt], f: &mut impl FnMut(u32)) {
    for stmt in code {
        use Stmt::*;
        match stmt {
            While(cond, body, _) => {
                for_each_mapped_expr_cond(cond, f);
                for_each_mapped_expr(body, f);
            }
            If(cond, body) => {
                for_each_mapped_expr_cond(cond, f);
                for_each_mapped_expr(body, f);
            }
            IfElse(cond, true_body, false_body) => {
                for_each_mapped_expr_cond(cond, f);
                for_each_mapped_expr(true_body, f);
                for_each_mapped_expr(false_body, f);
            }
            Seq(body) => {
                for_each_mapped_expr(body, f);
            }
            _ => (),
        }
    }
}

fn for_each_mapped_expr_cond(cond: &Cond, f: &mut impl FnMut(u32)) {
    use Cond::*;
    match cond {
        True | False => (),
        Not(cond) => for_each_mapped_expr_cond(cond, f),
        And(a, b) | Or(a, b) => {
            for_each_mapped_expr_cond(a, f);
            for_each_mapped_expr_cond(b, f);
        }
        Cmp(a, _, b) => {
            if let crate::ssa::cond::MappedExpr::Mapped(index) = a {
                f(*index);
            }
            if let crate::ssa::cond::MappedExpr::Mapped(index) = b {
                f(*index);
            }
        }
        Expr(expr) => {
            if let crate::ssa::cond::MappedExpr::Mapped(index) = expr {
                f(*index);
            }
        }
    }
}

fn hoist_conds(code: &mut Vec<Stmt>, todo: &mut HashSet<u32>, expr_map: &mut HashMap<u32, Expr>) {
    let mut hoisted = Vec::new();
    let mut i = 0;

    while let Some(stmt) = code.get_mut(i) {
        match stmt {
            Stmt::While(cond, ref mut body, _) => {
                for_each_mapped_expr_cond(cond, &mut |index| {
                    if todo.contains(&index) {
                        hoisted.push(index);
                        todo.remove(&index);
                    }
                });
                hoist_conds(body, todo, expr_map);
            }
            Stmt::If(cond, ref mut body) => {
                for_each_mapped_expr_cond(cond, &mut |index| {
                    if todo.contains(&index) {
                        hoisted.push(index);
                        todo.remove(&index);
                    }
                });
                hoist_conds(body, todo, expr_map);
            }
            Stmt::IfElse(cond, ref mut true_body, ref mut false_body) => {
                for_each_mapped_expr_cond(cond, &mut |index| {
                    if todo.contains(&index) {
                        hoisted.push(index);
                        todo.remove(&index);
                    }
                });
                hoist_conds(true_body, todo, expr_map);
                hoist_conds(false_body, todo, expr_map);
            }
            Stmt::Seq(ref mut body) => {
                hoist_conds(body, todo, expr_map);
            }
            _ => (),
        }
        if !hoisted.is_empty() {
            let mut stmts = Vec::new();
            for index in &hoisted {
                // TODO: find a proper way to get an unused index
                let var = Var::new(1337_0000, *index);
                let expr = expr_map.remove(index).unwrap();
                stmts.push(Stmt::SetLocal(var, expr));
                expr_map.insert(*index, Expr::GetLocal(var));
            }
            code.insert(i, Stmt::Seq(stmts));
            hoisted.clear();
            i += 1;
        }
        i += 1;
    }
}

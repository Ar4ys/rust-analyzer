use hir::{db::ExpandDatabase, diagnostics::RemoveUnnecessaryElse, HirFileIdExt};
use ide_db::{assists::Assist, source_change::SourceChange};
use itertools::Itertools;
use syntax::{
    ast::{self, edit::IndentLevel},
    AstNode, SyntaxToken, TextRange,
};
use text_edit::TextEdit;

use crate::{
    adjusted_display_range, fix, Diagnostic, DiagnosticCode, DiagnosticsContext, Severity,
};

// Diagnostic: remove-unnecessary-else
//
// This diagnostic is triggered when there is an `else` block for an `if` expression whose
// then branch diverges (e.g. ends with a `return`, `continue`, `break` e.t.c).
pub(crate) fn remove_unnecessary_else(
    ctx: &DiagnosticsContext<'_>,
    d: &RemoveUnnecessaryElse,
) -> Diagnostic {
    let display_range = adjusted_display_range(ctx, d.if_expr, &|if_expr| {
        if_expr.else_token().as_ref().map(SyntaxToken::text_range)
    });
    Diagnostic::new(
        DiagnosticCode::Ra("remove-unnecessary-else", Severity::WeakWarning),
        "remove unnecessary else block",
        display_range,
    )
    .with_fixes(fixes(ctx, d))
}

fn fixes(ctx: &DiagnosticsContext<'_>, d: &RemoveUnnecessaryElse) -> Option<Vec<Assist>> {
    let root = ctx.sema.db.parse_or_expand(d.if_expr.file_id);
    let if_expr = d.if_expr.value.to_node(&root);
    let if_expr = ctx.sema.original_ast_node(if_expr.clone())?;

    let indent = IndentLevel::from_node(if_expr.syntax());
    let replacement = match if_expr.else_branch()? {
        ast::ElseBranch::Block(ref block) => {
            block.statements().map(|stmt| format!("\n{indent}{stmt}")).join("")
        }
        ast::ElseBranch::IfExpr(ref nested_if_expr) => {
            format!("\n{indent}{nested_if_expr}")
        }
    };
    let range = TextRange::new(
        if_expr.then_branch()?.syntax().text_range().end(),
        if_expr.syntax().text_range().end(),
    );

    let edit = TextEdit::replace(range, replacement);
    let source_change =
        SourceChange::from_text_edit(d.if_expr.file_id.original_file(ctx.sema.db), edit);

    Some(vec![fix(
        "remove_unnecessary_else",
        "Remove unnecessary else block",
        source_change,
        range,
    )])
}

#[cfg(test)]
mod tests {
    use crate::tests::{check_diagnostics, check_fix};

    #[test]
    fn remove_unnecessary_else_for_return() {
        check_diagnostics(
            r#"
fn test() {
    if foo {
        return bar;
    } else {
    //^^^^ 💡 weak: remove unnecessary else block
        do_something_else();
    }
}
"#,
        );
        check_fix(
            r#"
fn test() {
    if foo {
        return bar;
    } else$0 {
        do_something_else();
    }
}
"#,
            r#"
fn test() {
    if foo {
        return bar;
    }
    do_something_else();
}
"#,
        );
    }

    #[test]
    fn remove_unnecessary_else_for_return2() {
        check_diagnostics(
            r#"
fn test() {
    if foo {
        return bar;
    } else if qux {
    //^^^^ 💡 weak: remove unnecessary else block
        do_something_else();
    } else {
        do_something_else2();
    }
}
"#,
        );
        check_fix(
            r#"
fn test() {
    if foo {
        return bar;
    } else$0 if qux {
        do_something_else();
    } else {
        do_something_else2();
    }
}
"#,
            r#"
fn test() {
    if foo {
        return bar;
    }
    if qux {
        do_something_else();
    } else {
        do_something_else2();
    }
}
"#,
        );
    }

    #[test]
    fn remove_unnecessary_else_for_break() {
        check_diagnostics(
            r#"
fn test() {
    loop {
        if foo {
            break;
        } else {
        //^^^^ 💡 weak: remove unnecessary else block
            do_something_else();
        }
    }
}
"#,
        );
        check_fix(
            r#"
fn test() {
    loop {
        if foo {
            break;
        } else$0 {
            do_something_else();
        }
    }
}
"#,
            r#"
fn test() {
    loop {
        if foo {
            break;
        }
        do_something_else();
    }
}
"#,
        );
    }

    #[test]
    fn remove_unnecessary_else_for_continue() {
        check_diagnostics(
            r#"
fn test() {
    loop {
        if foo {
            continue;
        } else {
        //^^^^ 💡 weak: remove unnecessary else block
            do_something_else();
        }
    }
}
"#,
        );
        check_fix(
            r#"
fn test() {
    loop {
        if foo {
            continue;
        } else$0 {
            do_something_else();
        }
    }
}
"#,
            r#"
fn test() {
    loop {
        if foo {
            continue;
        }
        do_something_else();
    }
}
"#,
        );
    }

    #[test]
    fn remove_unnecessary_else_for_never() {
        check_diagnostics(
            r#"
fn test() {
    if foo {
        never();
    } else {
    //^^^^ 💡 weak: remove unnecessary else block
        do_something_else();
    }
}

fn never() -> ! {
    loop {}
}
"#,
        );
        check_fix(
            r#"
fn test() {
    if foo {
        never();
    } else$0 {
        do_something_else();
    }
}

fn never() -> ! {
    loop {}
}
"#,
            r#"
fn test() {
    if foo {
        never();
    }
    do_something_else();
}

fn never() -> ! {
    loop {}
}
"#,
        );
    }

    #[test]
    fn no_diagnostic_if_no_else_branch() {
        check_diagnostics(
            r#"
fn test() {
    if foo {
        return bar;
    }

    do_something_else();
}
"#,
        );
    }

    #[test]
    fn no_diagnostic_if_no_divergence() {
        check_diagnostics(
            r#"
fn test() {
    if foo {
        do_something();
    } else {
        do_something_else();
    }
}
"#,
        );
    }

    #[test]
    fn no_diagnostic_if_no_divergence_in_else_branch() {
        check_diagnostics(
            r#"
fn test() {
    if foo {
        do_something();
    } else {
        return bar;
    }
}
"#,
        );
    }
}

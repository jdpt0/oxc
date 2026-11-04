//! [`JsRoot::Program`](crate::JsRoot::Program): statements, a `<script>`'s, a Markdown code block's,
//! an event handler attribute's (`onclick="a(); b()"`, Vue `@click="a(); b()"`).

use oxc_ast::ast::*;
use oxc_span::GetSpan;

use crate::{
    ProgramLayout, ProgramSemicolons,
    ast_nodes::AstNode,
    formatter::{
        JsFormatter, TopLevelSemicolons,
        prelude::*,
        trivia::{FormatTrailingComments, format_dangling_comments},
    },
    options::Semicolons,
    utils::{is_dropped_statement, statement_span},
    write,
};

use super::super::{block_statement::is_empty_block, program::FormatProgramBody};

/// Writes `program`'s statements, ending as `semicolons` says, laid out as `layout` says.
pub(super) fn write_program<'a>(
    program: &AstNode<'_, 'a, Program<'a>>,
    semicolons: ProgramSemicolons,
    layout: ProgramLayout,
    f: &mut JsFormatter<'_, 'a>,
) {
    let top_level = top_level_semicolons(program, semicolons, f.options().semicolons);
    f.context_mut().set_top_level_semicolons(top_level);
    // The top level's `;` that `Separated` drops the ASI guards for are written by `FormatSeparatedStatements`, whatever the layout
    let statements = format_with(|f| {
        if top_level == TopLevelSemicolons::Separated {
            write!(f, FormatSeparatedStatements(program));
        } else {
            write!(f, FormatProgramBody(program));
        }
    });
    match layout {
        ProgramLayout::Bare => write!(f, statements),
        ProgramLayout::Indented => {
            let is_empty = program.hashbang.is_none()
                && is_empty_block(&program.body)
                && program.directives.is_empty();
            // Nothing to print (`onclick=" ; "`) leaves the value empty
            if is_empty && program.comments.is_empty() {
                return;
            }
            write!(
                f,
                soft_block_indent(&format_with(|f| {
                    if is_empty {
                        // Comments only (`onclick="/* c */"`) print as the program's own, without a space before them
                        write!(f, format_dangling_comments(program.span));
                    } else {
                        write!(f, statements);
                    }
                }))
            );
        }
    }
}

/// How `program`'s top-level statements end under `semicolons` (and `semi`).
fn top_level_semicolons(
    program: &Program<'_>,
    semicolons: ProgramSemicolons,
    semi: Semicolons,
) -> TopLevelSemicolons {
    let lone_expression = match program.body.as_slice() {
        [Statement::ExpressionStatement(statement)] if program.directives.is_empty() => {
            Some(statement)
        }
        _ => None,
    };
    match semicolons {
        ProgramSemicolons::AsOptions => TopLevelSemicolons::AsOptions,
        ProgramSemicolons::LoneJsxOmitted | ProgramSemicolons::LoneExpressionOmitted => {
            lone_expression
                .filter(|statement| {
                    semicolons == ProgramSemicolons::LoneExpressionOmitted
                        || statement.expression.is_jsx()
                })
                .map_or(TopLevelSemicolons::AsOptions, |statement| {
                    TopLevelSemicolons::Omitted(statement.span)
                })
        }
        ProgramSemicolons::MarksStatements => marking_statements(program, semi),
    }
}

/// [`ProgramSemicolons::MarksStatements`]: under `semi: false`, the program writes the top level's `;`
/// (`FormatSeparatedStatements`), which the host reads the code as statements by:
/// between the statements, and after a lone one, as the source has it.
fn marking_statements(program: &Program<'_>, semi: Semicolons) -> TopLevelSemicolons {
    let separated = if semi == Semicolons::AsNeeded {
        TopLevelSemicolons::Separated
    } else {
        TopLevelSemicolons::AsOptions
    };
    let mut statements = program.body.iter().filter(|stmt| !is_dropped_statement(stmt));
    match (statements.next(), statements.next()) {
        // A lone expression without its `;` is one the host reads as an expression (`a, b`), and keeps it so
        (Some(Statement::ExpressionStatement(statement)), None)
            if program.directives.is_empty()
                && !program.source_text[..statement.span.end as usize].ends_with(';') =>
        {
            TopLevelSemicolons::Omitted(statement.span)
        }
        (Some(_), _) => separated,
        (None, _) => TopLevelSemicolons::AsOptions,
    }
}

/// The statements under [`TopLevelSemicolons::Separated`]: a `;` after each but the last,
/// or after a lone one, a guard for the host as the ASI guard is for JS. Nested statements follow `semi`.
///
/// A statement ending where its `;` goes prints it as its terminator (`FormatContentWithSemicolon`),
/// its same-line comments behind it; another (`if (a) {}`) is followed by it.
struct FormatSeparatedStatements<'a, 'b>(&'b AstNode<'b, 'a, Program<'a>>);

impl<'a> Format<'a, JsFormatContext<'a>> for FormatSeparatedStatements<'a, '_> {
    fn fmt(&self, f: &mut JsFormatter<'_, 'a>) {
        let program = self.0;
        write!(f, [program.hashbang(), program.directives()]);
        let body = program.body();
        let statements = || body.iter().filter(|stmt| !is_dropped_statement(stmt.as_ref()));
        let last = statements().count() - 1;
        let mut join = f.join_nodes_with_hardline();
        for (i, statement) in statements().enumerate() {
            let separated = i < last || last == 0;
            let entry = format_with(|f| {
                if !separated {
                    return write!(f, statement);
                }
                let end = statement.as_ref().span().end;
                f.context_mut().set_separator_end(end);
                write!(f, statement);
                // No statement ending there printed it
                if f.context_mut().take_separator_at(end) {
                    write!(f, ";");
                }
            });
            join.entry(statement_span(statement.as_ref()), &entry);
        }
        drop(join);
        write!(f, FormatTrailingComments::Comments(f.context().comments().unprinted_comments()));
    }
}

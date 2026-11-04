//! [`JsRoot::Expression`](crate::JsRoot::Expression): an expression in its host's delimiters (Vue `{{ a }}`, `:a="b"`).

use std::ops::Range;

use oxc_ast::ast::*;
use oxc_diagnostics::OxcDiagnostic;
use oxc_span::GetSpan;
use oxc_syntax::{identifier::is_white_space, line_terminator::is_line_terminator};

use crate::{
    ExpressionLayout, ExpressionRoot,
    ast_nodes::{AstNode, AstNodes},
    formatter::{JsFormatter, TailwindContextEntry, prelude::*},
    utils::expression::ExpressionLeftSide,
    write,
};

use super::shape_error;

/// The code is the one expression of `program`, parsed in `(` and `)`.
pub(super) fn check(
    program: &Program<'_>,
    source: Range<usize>,
    root: ExpressionRoot,
    inlines_code: bool,
) -> Result<(), OxcDiagnostic> {
    // An expression that ends inside the source closes the wrapper's paren (`a) + (b`)
    let [Statement::ExpressionStatement(statement)] = program.body.as_slice() else {
        return Err(shape_error());
    };
    let span = statement.expression.span();
    if !program.directives.is_empty()
        || (span.start as usize) < source.start
        || span.end as usize > source.end
    {
        return Err(shape_error());
    }
    // Inlined as written, a top-level `a, b` reads as more than one value (Vue's `toDisplayString(a, b)` shows `a`):
    // the parens it prints in would change it, so it is left as written
    if inlines_code && matches!(statement.expression, Expression::SequenceExpression(_)) {
        return Err(OxcDiagnostic::error(
            "The code is a sequence expression; it is preserved as-is",
        ));
    }
    // Read as statements, it does not compile; the parens it prints in would make it
    if root.reads_as_statements
        && starts_declaration_as_statements(program, &statement.expression, source)
    {
        return Err(OxcDiagnostic::error(
            "The code starts a declaration or a block where the host reads statements; it is preserved as-is",
        ));
    }
    Ok(())
}

/// The host reads the code as statements, holding a `;` ([`ExpressionRoot::reads_as_statements`]),
/// and its leftmost node starts a declaration or a block there (`function`, `class`, `{`, `let [`),
/// which the expression is not (`function () { a(); }`).
fn starts_declaration_as_statements(
    program: &Program<'_>,
    expression: &Expression<'_>,
    source: Range<usize>,
) -> bool {
    if !program.source_text[source.clone()].contains(';') {
        return false;
    }
    // The position past whitespace and comments
    let skip_trivia = |mut position: usize| {
        loop {
            let rest = &program.source_text[position..source.end];
            position += rest.len()
                - rest.trim_start_matches(|c| is_white_space(c) || is_line_terminator(c)).len();
            match program.comments.iter().find(|comment| comment.span.start as usize == position) {
                Some(comment) => position = comment.span.end as usize,
                None => return position,
            }
        }
    };
    let leftmost = ExpressionLeftSide::leftmost(expression);
    // In parens in the source (`(function () {})()`), it starts no statement
    if skip_trivia(source.start) != leftmost.span().start as usize {
        return false;
    }
    match leftmost {
        Expression::FunctionExpression(_)
        | Expression::ClassExpression(_)
        | Expression::ObjectExpression(_) => true,
        Expression::AssignmentExpression(assignment) => {
            matches!(assignment.left, AssignmentTarget::ObjectAssignmentTarget(_))
        }
        Expression::Identifier(identifier) if identifier.name == "let" => {
            program.source_text[skip_trivia(identifier.span.end as usize)..].starts_with('[')
        }
        _ => false,
    }
}

/// Writes `expression`, the root of `program`, in the layout `root` names.
pub(super) fn write_expression<'a>(
    program: &AstNode<'_, 'a, Program<'a>>,
    expression: &'a Expression<'a>,
    root: ExpressionRoot,
    ends_in_line_comment: bool,
    f: &mut JsFormatter<'_, 'a>,
) {
    f.context_mut().set_expression_root(root);
    // Under the `Program`, not the `ExpressionStatement`: no statement rule applies to it
    let expression = AstNode::new(expression, AstNodes::Program(program));
    let tailwind = f
        .options()
        .sort_tailwindcss
        .as_ref()
        .filter(|_| root.tailwind_classes)
        .map(|options| TailwindContextEntry::new(options.preserve_whitespace));
    if let Some(context) = tailwind {
        f.context_mut().push_tailwind_context(context);
    }
    match root.layout {
        ExpressionLayout::Bare => write!(f, expression),
        _ if ends_in_line_comment => write!(f, soft_block_indent(&expression)),
        ExpressionLayout::Group => write!(f, group(&expression)),
        ExpressionLayout::Hug { literals } => {
            let hugs = match expression.as_ref() {
                Expression::ObjectExpression(_) | Expression::ArrayExpression(_) => true,
                Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => literals,
                _ => false,
            };
            if hugs {
                write!(f, group(&expression));
            } else {
                write!(f, soft_block_indent(&expression));
            }
        }
    }
    if tailwind.is_some() {
        f.context_mut().pop_tailwind_context();
    }
}

/// The object at `span`, in an embedded expression its host ends at the first `}}` ([`ExpressionRoot::ends_at_double_brace`]),
/// prints `}}` without bracket spacing, right before another's `}`: it prints in parens (`{a: ({b: 1})}`, `{...({a: 1})}`).
///
/// Prettier parens a property's value only (`apps/oxfmt/DIVERGENCES.md#js-in-vue-object-before-double-brace`).
///
/// NOTE: Only an object is covered: a `}` other nodes end in can still print `}}`
/// (`${{}}`, `{a() {}}`, `{a: () => {}}`). Never seen in real code, so left as is.
pub fn object_prints_double_brace(
    span: Span,
    parent: &AstNodes<'_, '_>,
    f: &JsFormatter<'_, '_>,
) -> bool {
    f.context().expression_root().is_some_and(|root| root.ends_at_double_brace)
        && !f.options().bracket_spacing.value()
        && is_followed_by_right_bracket(span, parent)
}

/// The node at `span` prints right before an object's `}`.
fn is_followed_by_right_bracket(span: Span, parent: &AstNodes<'_, '_>) -> bool {
    let ends_parent = match parent {
        AstNodes::ObjectProperty(property) if property.value.span() == span => {
            return matches!(property.parent(), AstNodes::ObjectExpression(object)
                if object.properties.last().is_some_and(|last| last.span() == property.span));
        }
        // Another argument (a conditional, a logical, an assignment) prints in parens there
        AstNodes::SpreadElement(spread)
            if matches!(
                spread.argument,
                Expression::ObjectExpression(_) | Expression::UnaryExpression(_)
            ) =>
        {
            return matches!(spread.parent(), AstNodes::ObjectExpression(object)
                if object.properties.last().is_some_and(|last| last.span() == spread.span));
        }
        AstNodes::BinaryExpression(binary) => binary.right.span() == span,
        AstNodes::LogicalExpression(logical) => logical.right.span() == span,
        AstNodes::ConditionalExpression(conditional) => conditional.alternate.span() == span,
        AstNodes::UnaryExpression(_) => true,
        _ => false,
    };
    ends_parent && is_followed_by_right_bracket(parent.span(), parent.parent())
}

use crate::ast::*;

pub type Error = String;
pub type Result<T> = std::result::Result<T, Error>;

pub trait Modifier = FnMut(Node) -> Node;

pub fn modify<F: Modifier>(node: Node, mut modifier: F) -> Result<Node> {
    match node {
        Node::Program(it) => Ok(modify_program(it, &mut modifier)?.into()),
        Node::Statement(it) => Ok(modify_statement(it, &mut modifier)?.into()),
        Node::Expression(it) => Ok(modify_expression(it, &mut modifier)?.into()),
    }
}

fn modify_program<F: Modifier>(prog: Program, modifier: &mut F) -> Result<Program> {
    let mut statements = Vec::new();
    for stmt in prog.statements {
        let stmt = modify_statement(stmt, modifier)?;
        statements.push(stmt);
    }
    let prog = Program { statements };
    Ok(prog)
}

// statements

fn modify_statement<F: Modifier>(stmt: Statement, modifier: &mut F) -> Result<Statement> {
    match stmt {
        Statement::Let {
            identifier,
            expression,
        } => {
            let expression = modify_expression(expression, modifier)?;
            Ok(Statement::Let {
                identifier,
                expression,
            })
        }
        Statement::Return(expr) => Ok(Statement::Return(modify_expression(expr, modifier)?)),
        Statement::Expression(expr) => {
            Ok(Statement::Expression(modify_expression(expr, modifier)?))
        }
        Statement::Block(block) => {
            let block = modify_block_statement(block, modifier)?;
            Ok(block.into())
        }
    }
}

fn modify_block_statement<F: Modifier>(
    block: BlockStatement,
    modifier: &mut F,
) -> Result<BlockStatement> {
    let mut statements = Vec::new();
    for stmt in block.statements {
        let stmt = modify_statement(stmt, modifier)?;
        statements.push(stmt);
    }
    let block = BlockStatement { statements };
    Ok(block.into())
}

// expressions

fn modify_expression<F: Modifier>(expr: Expression, modifier: &mut F) -> Result<Expression> {
    match expr {
        Expression::Array(ary) => {
            let mut elements = Vec::new();
            for e in ary {
                elements.push(modify_expression(e, modifier)?);
            }
            Ok(Expression::Array(elements))
        }
        Expression::Hash(map) => {
            let mut new_map = BTreeMap::new();
            for (k, v) in map {
                let k = modify_expression(k, modifier)?;
                let v = modify_expression(v, modifier)?;
                new_map.insert(k, v);
            }
            Ok(Expression::Hash(new_map))
        }
        Expression::Prefix { operator, right } => {
            let right = modify_expression(*right, modifier)?;
            Ok(Expression::Prefix {
                operator,
                right: right.into(),
            })
        }
        Expression::Infix {
            left,
            operator,
            right,
        } => {
            let left = modify_expression(*left, modifier)?;
            let right = modify_expression(*right, modifier)?;
            Ok(Expression::Infix {
                left: left.into(),
                operator,
                right: right.into(),
            })
        }
        Expression::If {
            condition,
            consequence,
            alternative,
        } => {
            let condition = modify_expression(*condition, modifier)?;
            let consequence = modify_block_statement(consequence, modifier)?;
            let alternative = if let Some(alt) = alternative {
                Some(modify_block_statement(alt, modifier)?)
            } else {
                alternative
            };
            Ok(Expression::If {
                condition: condition.into(),
                consequence,
                alternative,
            })
        }
        Expression::Function(f) => {
            let body = modify_block_statement(f.body, modifier)?;
            let f = FunctionExpression {
                params: f.params,
                body,
            };
            Ok(Expression::Function(f))
        }
        Expression::Index { left, index } => {
            let left = modify_expression(*left, modifier)?;
            let index = modify_expression(*index, modifier)?;
            Ok(Expression::Index {
                left: left.into(),
                index: index.into(),
            })
        }
        other => Ok(modifier(other.into()).expression()?),
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::modify::modify;
    use crate::ast::*;
    use std::collections::BTreeMap;

    #[test]
    fn modify_integer_expression() -> Result<(), Box<dyn std::error::Error>> {
        let node = Node::from(one());
        let expected = two().into();
        let res = modify(node, turn_one_into_two)?;
        assert_eq!(res, expected);
        Ok(())
    }

    #[test]
    fn modify_array_expression() -> Result<(), Box<dyn std::error::Error>> {
        let tests = vec![(
            Expression::Array(vec![one(), one()]),
            Expression::Array(vec![two(), two()]),
        )];
        for (expr, expected) in tests {
            let node = Node::from(expr);
            let res = modify(node, turn_one_into_two)?;
            assert_eq!(res, expected.into());
        }
        Ok(())
    }

    #[test]
    fn modify_hash_expression() -> Result<(), Box<dyn std::error::Error>> {
        let tests = vec![(
            Expression::Hash(vec![(one(), one())].into_iter().collect::<BTreeMap<_, _>>()),
            Expression::Hash(vec![(two(), two())].into_iter().collect::<BTreeMap<_, _>>()),
        )];
        for (expr, expected) in tests {
            let node = Node::from(expr);
            let res = modify(node, turn_one_into_two)?;
            assert_eq!(res, expected.into());
        }
        Ok(())
    }

    #[test]
    fn modify_prefix_expression() -> Result<(), Box<dyn std::error::Error>> {
        let tests = vec![(
            Expression::Prefix {
                operator: PrefixOperator::Minus,
                right: one().into(),
            },
            Expression::Prefix {
                operator: PrefixOperator::Minus,
                right: two().into(),
            },
        )];
        for (expr, expected) in tests {
            let node = Node::from(expr);
            let res = modify(node, turn_one_into_two)?;
            assert_eq!(res, expected.into());
        }
        Ok(())
    }

    #[test]
    fn modify_infix_expression() -> Result<(), Box<dyn std::error::Error>> {
        let tests = vec![
            (
                Expression::Infix {
                    left: one().into(),
                    operator: InfixOperator::Add,
                    right: two().into(),
                },
                Expression::Infix {
                    left: two().into(),
                    operator: InfixOperator::Add,
                    right: two().into(),
                },
            ),
            (
                Expression::Infix {
                    left: two().into(),
                    operator: InfixOperator::Add,
                    right: one().into(),
                },
                Expression::Infix {
                    left: two().into(),
                    operator: InfixOperator::Add,
                    right: two().into(),
                },
            ),
        ];
        for (expr, expected) in tests {
            let node = Node::from(expr);
            let res = modify(node, turn_one_into_two)?;
            assert_eq!(res, expected.into());
        }
        Ok(())
    }

    #[test]
    fn modify_if_expression() -> Result<(), Box<dyn std::error::Error>> {
        let tests = vec![(
            Expression::If {
                condition: one().into(),
                consequence: BlockStatement {
                    statements: vec![Statement::Expression(one())],
                },
                alternative: Some(BlockStatement {
                    statements: vec![Statement::Expression(one())],
                }),
            },
            Expression::If {
                condition: two().into(),
                consequence: BlockStatement {
                    statements: vec![Statement::Expression(two())],
                },
                alternative: Some(BlockStatement {
                    statements: vec![Statement::Expression(two())],
                }),
            },
        )];
        for (expr, expected) in tests {
            let node = Node::from(expr);
            let res = modify(node, turn_one_into_two)?;
            assert_eq!(res, expected.into());
        }
        Ok(())
    }

    #[test]
    fn modify_function_expression() -> Result<(), Box<dyn std::error::Error>> {
        let tests = vec![(
            Expression::Function(FunctionExpression {
                params: Vec::new(),
                body: BlockStatement {
                    statements: vec![Statement::Expression(one())],
                },
            }),
            Expression::Function(FunctionExpression {
                params: Vec::new(),
                body: BlockStatement {
                    statements: vec![Statement::Expression(two())],
                },
            }),
        )];
        for (expr, expected) in tests {
            let node = Node::from(expr);
            let res = modify(node, turn_one_into_two)?;
            assert_eq!(res, expected.into());
        }
        Ok(())
    }

    #[test]
    fn modify_index_expression() -> Result<(), Box<dyn std::error::Error>> {
        let tests = vec![(
            Expression::Index {
                left: one().into(),
                index: one().into(),
            },
            Expression::Index {
                left: two().into(),
                index: two().into(),
            },
        )];
        for (expr, expected) in tests {
            let node = Node::from(expr);
            let res = modify(node, turn_one_into_two)?;
            assert_eq!(res, expected.into());
        }
        Ok(())
    }

    #[test]
    fn modify_program_statement() -> Result<(), Box<dyn std::error::Error>> {
        let node = Program {
            statements: vec![Statement::Expression(one())],
        }
        .into();
        let expected = Program {
            statements: vec![Statement::Expression(two())],
        }
        .into();
        let res = modify(node, turn_one_into_two)?;
        assert_eq!(res, expected);
        Ok(())
    }

    #[test]
    fn modify_let_statement() -> Result<(), Box<dyn std::error::Error>> {
        let tests = vec![(
            Statement::Let {
                identifier: "foo".into(),
                expression: one(),
            },
            Statement::Let {
                identifier: "foo".into(),
                expression: two(),
            },
        )];
        for (expr, expected) in tests {
            let node = Node::from(expr);
            let res = modify(node, turn_one_into_two)?;
            assert_eq!(res, expected.into());
        }
        Ok(())
    }

    #[test]
    fn modify_return_statement() -> Result<(), Box<dyn std::error::Error>> {
        let tests = vec![(Statement::Return(one()), Statement::Return(two()))];
        for (expr, expected) in tests {
            let node = Node::from(expr);
            let res = modify(node, turn_one_into_two)?;
            assert_eq!(res, expected.into());
        }
        Ok(())
    }

    // helpers

    fn one() -> Expression {
        Expression::Integer(1)
    }

    fn two() -> Expression {
        Expression::Integer(2)
    }

    fn turn_one_into_two(node: Node) -> Node {
        if let Node::Expression(expr) = &node {
            if let Expression::Integer(it) = expr {
                if it == &1 {
                    return Expression::Integer(2).into();
                }
            }
        }
        node
    }
}

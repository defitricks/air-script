use super::{Expression, NamedTraceAccess, Variable};
use std::fmt::Display;

// BOUNDARY STATEMENTS
// ================================================================================================

#[derive(Debug, Eq, PartialEq)]
pub enum BoundaryStmt {
    Constraint(BoundaryConstraint),
    Variable(Variable),
}

/// Stores the expression corresponding to the boundary constraint.
#[derive(Debug, Eq, PartialEq)]
pub struct BoundaryConstraint {
    access: NamedTraceAccess,
    boundary: Boundary,
    value: Expression,
}

impl BoundaryConstraint {
    pub fn new(access: NamedTraceAccess, boundary: Boundary, value: Expression) -> Self {
        Self {
            access,
            boundary,
            value,
        }
    }

    pub fn access(&self) -> &NamedTraceAccess {
        &self.access
    }

    pub fn boundary(&self) -> Boundary {
        self.boundary
    }

    /// Returns the constraint's value expression.
    pub fn value(&self) -> &Expression {
        &self.value
    }
}

/// Describes the type of boundary in the boundary constraint.
#[derive(Debug, Eq, Copy, Clone, PartialEq)]
pub enum Boundary {
    First,
    Last,
}

impl Display for Boundary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Boundary::First => write!(f, "first boundary"),
            Boundary::Last => write!(f, "last boundary"),
        }
    }
}
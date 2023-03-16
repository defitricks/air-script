use super::{Identifier, IntegrityStmt, TraceCols};

/// Evaluator functions take descriptions of the main and auxiliary traces as input, and enforce
/// integrity constraints on those trace columns.
#[derive(Debug, Eq, PartialEq)]
pub struct EvaluatorFunction {
    name: Identifier,
    main_trace: Vec<TraceCols>,
    aux_trace: Vec<TraceCols>,
    integrity_stmts: Vec<IntegrityStmt>,
}

impl EvaluatorFunction {
    /// Creates a new function.
    pub fn new(
        name: Identifier,
        main_trace: Vec<TraceCols>,
        aux_trace: Vec<TraceCols>,
        integrity_stmts: Vec<IntegrityStmt>,
    ) -> Self {
        Self {
            name,
            main_trace,
            aux_trace,
            integrity_stmts,
        }
    }

    /// Returns the name of the evaluator function.
    pub fn name(&self) -> &str {
        self.name.name()
    }

    /// Returns the main trace of the evaluator function.
    pub fn main_trace(&self) -> &Vec<TraceCols> {
        &self.main_trace
    }

    /// Returns the auxiliary trace of the evaluator function.
    pub fn aux_trace(&self) -> &Vec<TraceCols> {
        &self.aux_trace
    }

    /// Returns the integrity statements of the evaluator function.
    pub fn integrity_stmts(&self) -> &[IntegrityStmt] {
        &self.integrity_stmts
    }

    /// Returns the name, main trace columns, auxiliary trace columns, and integrity statements
    /// of the evaluator function.
    pub fn into_parts(self) -> (String, Vec<TraceCols>, Vec<TraceCols>, Vec<IntegrityStmt>) {
        (
            self.name.into_name(),
            self.main_trace,
            self.aux_trace,
            self.integrity_stmts,
        )
    }
}

/// Evaluator function call is used to invoke an evaluator function. It takes a list of trace
/// columns.
#[derive(Debug, Eq, PartialEq)]
pub struct EvaluatorFunctionCall {
    name: Identifier,
    args: Vec<Vec<TraceCols>>,
}

impl EvaluatorFunctionCall {
    /// Creates a new evaluator function call.
    pub fn new(name: Identifier, args: Vec<Vec<TraceCols>>) -> Self {
        Self { name, args }
    }

    /// Returns the name of the evaluator function.
    pub fn name(&self) -> &str {
        self.name.name()
    }

    /// Returns the arguments of the evaluator function.
    pub fn args(&self) -> &Vec<Vec<TraceCols>> {
        &self.args
    }

    /// Returns the name and arguments of the evaluator function.
    pub fn into_parts(self) -> (String, Vec<Vec<TraceCols>>) {
        (self.name.into_name(), self.args)
    }
}
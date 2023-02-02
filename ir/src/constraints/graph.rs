use air_script_core::Iterable;

use super::{
    BTreeMap, ConstantType, ConstraintDomain, ExprContext, ExprDetails, Expression, Identifier,
    IdentifierType, IndexedTraceAccess, IntegrityConstraintDegree, ListComprehensionContext,
    MatrixAccess, NamedTraceAccess, SemanticError, SymbolTable, TraceSegment, VariableRoots,
    VariableType, VectorAccess,
};

// CONSTANTS
// ================================================================================================

/// The offset of the "current" row during constraint evaluation.
const CURRENT_ROW: usize = 0;
/// The default segment against which a constraint is applied is the main trace segment.
const DEFAULT_SEGMENT: TraceSegment = 0;
/// The auxiliary trace segment.
const AUX_SEGMENT: TraceSegment = 1;

// ALGEBRAIC GRAPH
// ================================================================================================

/// The AlgebraicGraph is a directed acyclic graph used to represent integrity constraints. To
/// store it compactly, it is represented as a vector of nodes where each node references other
/// nodes by their index in the vector.
///
/// Within the graph, constraint expressions can overlap and share subgraphs, since new expressions
/// reuse matching existing nodes when they are added, rather than creating new nodes.
///
/// - Leaf nodes (with no outgoing edges) are constants or references to trace cells (i.e. column 0
///   in the current row or column 5 in the next row).
/// - Tip nodes with no incoming edges (no parent nodes) always represent constraints, although they
///   do not necessarily represent all constraints. There could be constraints which are also
///   subgraphs of other constraints.
#[derive(Default, Debug)]
pub struct AlgebraicGraph {
    /// All nodes in the graph.
    nodes: Vec<Node>,
}

impl AlgebraicGraph {
    // --- PUBLIC ACCESSORS -----------------------------------------------------------------------

    /// Returns the node with the specified index.
    pub fn node(&self, index: &NodeIndex) -> &Node {
        &self.nodes[index.0]
    }

    /// Returns the degree of the subgraph which has the specified node as its tip.
    pub fn degree(&self, index: &NodeIndex) -> IntegrityConstraintDegree {
        let mut cycles: BTreeMap<usize, usize> = BTreeMap::new();
        let base = self.accumulate_degree(&mut cycles, index);

        if cycles.is_empty() {
            IntegrityConstraintDegree::new(base)
        } else {
            IntegrityConstraintDegree::with_cycles(base, cycles.values().cloned().collect())
        }
    }

    // --- PUBLIC MUTATORS ------------------------------------------------------------------------

    /// Combines two subgraphs representing equal subexpressions and returns the [ExprDetails] of
    /// the new subgraph.
    ///
    /// TODO: we can optimize this in the future in the case where lhs or rhs equals zero to just
    /// return the other expression.
    pub(super) fn merge_equal_exprs(
        &mut self,
        lhs: &ExprDetails,
        rhs: &ExprDetails,
    ) -> Result<ExprDetails, SemanticError> {
        let node_index = self.insert_op(Operation::Sub(lhs.root_idx(), rhs.root_idx()));
        let trace_segment = lhs.trace_segment().max(rhs.trace_segment());
        let domain = lhs.domain().merge(&rhs.domain())?;

        Ok(ExprDetails::new(node_index, trace_segment, domain, None))
    }

    /// Adds the expression to the graph and returns the [ExprDetails] of the constraint.
    /// Expressions are added recursively to reuse existing matching nodes.
    pub(super) fn insert_expr(
        &mut self,
        symbol_table: &SymbolTable,
        expr: &Expression,
        variable_roots: &mut VariableRoots,
        default_domain: ConstraintDomain,
        context: &mut ExprContext,
    ) -> Result<ExprDetails, SemanticError> {
        match expr {
            Expression::Const(value) => {
                let node_index = self.insert_op(Operation::Constant(ConstantValue::Inline(*value)));
                Ok(ExprDetails::new(
                    node_index,
                    DEFAULT_SEGMENT,
                    default_domain,
                    Some(*value as usize),
                ))
            }
            Expression::Elem(Identifier(ident)) => self.insert_symbol_access(
                symbol_table,
                ident,
                variable_roots,
                default_domain,
                context,
            ),
            Expression::VectorAccess(vector_access) => self.insert_vector_access(
                symbol_table,
                vector_access,
                variable_roots,
                default_domain,
                context,
            ),
            Expression::MatrixAccess(matrix_access) => self.insert_matrix_access(
                symbol_table,
                matrix_access,
                variable_roots,
                default_domain,
                context,
            ),
            Expression::Rand(index) => {
                // The constraint target for random values defaults to the second (auxiliary) trace
                // segment.
                // TODO: make this more general, so random values from further trace segments can
                // be used. This requires having a way to describe different sets of randomness in
                // the AirScript syntax.
                let node_index = self.insert_op(Operation::RandomValue(*index));
                Ok(ExprDetails::new(
                    node_index,
                    AUX_SEGMENT,
                    default_domain,
                    None,
                ))
            }
            Expression::IndexedTraceAccess(column_access) => {
                self.insert_indexed_trace_access(symbol_table, column_access)
            }
            Expression::NamedTraceAccess(trace_access) => self.insert_named_trace_access(
                symbol_table,
                trace_access,
                ConstraintDomain::from(trace_access.row_offset()),
            ),
            Expression::Add(lhs, rhs) => {
                // add both subexpressions.
                let lhs =
                    self.insert_expr(symbol_table, lhs, variable_roots, default_domain, context)?;
                let rhs =
                    self.insert_expr(symbol_table, rhs, variable_roots, default_domain, context)?;
                // add the expression.
                self.insert_bin_op(&lhs, &rhs, Operation::Add(lhs.root_idx(), rhs.root_idx()))
            }
            Expression::Sub(lhs, rhs) => {
                // add both subexpressions.
                let lhs =
                    self.insert_expr(symbol_table, lhs, variable_roots, default_domain, context)?;
                let rhs =
                    self.insert_expr(symbol_table, rhs, variable_roots, default_domain, context)?;
                // add the expression.
                self.insert_bin_op(&lhs, &rhs, Operation::Sub(lhs.root_idx(), rhs.root_idx()))
            }
            Expression::Mul(lhs, rhs) => {
                // add both subexpressions.
                let lhs =
                    self.insert_expr(symbol_table, lhs, variable_roots, default_domain, context)?;
                let rhs =
                    self.insert_expr(symbol_table, rhs, variable_roots, default_domain, context)?;
                // add the expression.
                self.insert_bin_op(&lhs, &rhs, Operation::Mul(lhs.root_idx(), rhs.root_idx()))
            }
            Expression::Exp(lhs, rhs) => {
                // add base subexpression.
                let lhs =
                    self.insert_expr(symbol_table, lhs, variable_roots, default_domain, context)?;
                // add exponent subexpression.
                let node_index = if let Expression::Const(rhs) = **rhs {
                    self.insert_op(Operation::Exp(lhs.root_idx(), rhs as usize))
                } else {
                    todo!()
                };

                Ok(ExprDetails::new(
                    node_index,
                    lhs.trace_segment(),
                    lhs.domain(),
                    None,
                ))
            }
            Expression::ListFolding(_) => todo!(),
        }
    }

    /// Converts a [NamedTraceAccess] element into an [IndexedTraceAccess] by its identifier name,
    /// trace segment, and row offset, then adds it to the graph.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The identifier was not declared as a trace column type.
    pub(super) fn insert_named_trace_access(
        &mut self,
        symbol_table: &SymbolTable,
        trace_access: &NamedTraceAccess,
        domain: ConstraintDomain,
    ) -> Result<ExprDetails, SemanticError> {
        let elem_type = symbol_table.get_type(trace_access.name())?;
        match elem_type {
            IdentifierType::TraceColumns(columns) => {
                let trace_segment = columns.trace_segment();
                let trace_access = IndexedTraceAccess::new(
                    trace_segment,
                    columns.offset() + trace_access.idx(),
                    trace_access.row_offset(),
                );
                let node_index = self.insert_op(Operation::TraceElement(trace_access));
                Ok(ExprDetails::new(node_index, trace_segment, domain, None))
            }
            _ => Err(SemanticError::InvalidUsage(format!(
                "Identifier {} was declared as a {} not as a trace column",
                trace_access.name(),
                elem_type
            ))),
        }
    }

    // --- HELPERS --------------------------------------------------------------------------------

    /// Recursively accumulates the base degree and the cycle lengths of the periodic columns.
    fn accumulate_degree(&self, cycles: &mut BTreeMap<usize, usize>, index: &NodeIndex) -> usize {
        // recursively walk the subgraph and compute the degree from the operation and child nodes
        match self.node(index).op() {
            Operation::Constant(_) | Operation::RandomValue(_) => 0,
            Operation::TraceElement(_) => 1,
            Operation::PeriodicColumn(index, cycle_len) => {
                cycles.insert(*index, *cycle_len);
                0
            }
            Operation::Neg(index) => self.accumulate_degree(cycles, index),
            Operation::Add(lhs, rhs) => {
                let lhs_base = self.accumulate_degree(cycles, lhs);
                let rhs_base = self.accumulate_degree(cycles, rhs);
                lhs_base.max(rhs_base)
            }
            Operation::Sub(lhs, rhs) => {
                let lhs_base = self.accumulate_degree(cycles, lhs);
                let rhs_base = self.accumulate_degree(cycles, rhs);
                lhs_base.max(rhs_base)
            }
            Operation::Mul(lhs, rhs) => {
                let lhs_base = self.accumulate_degree(cycles, lhs);
                let rhs_base = self.accumulate_degree(cycles, rhs);
                lhs_base + rhs_base
            }
            Operation::Exp(lhs, rhs) => {
                let lhs_base = self.accumulate_degree(cycles, lhs);
                lhs_base * rhs
            }
        }
    }

    /// Insert the operation and return its node index. If an identical node already exists, return
    /// that index instead.
    fn insert_op(&mut self, op: Operation) -> NodeIndex {
        self.nodes.iter().position(|n| *n.op() == op).map_or_else(
            || {
                // create a new node.
                let index = self.nodes.len();
                self.nodes.push(Node { op });
                NodeIndex(index)
            },
            |index| {
                // return the existing node's index.
                NodeIndex(index)
            },
        )
    }

    /// Inserts a binary operation into the graph and returns the resulting expression details.
    fn insert_bin_op(
        &mut self,
        lhs: &ExprDetails,
        rhs: &ExprDetails,
        op: Operation,
    ) -> Result<ExprDetails, SemanticError> {
        let node_index = self.insert_op(op);
        let trace_segment = lhs.trace_segment().max(rhs.trace_segment());
        let domain = lhs.domain().merge(&rhs.domain())?;
        Ok(ExprDetails::new(node_index, trace_segment, domain, None))
    }

    /// Inserts the specified constant value into the graph and returns the resulting expression
    /// details.
    fn insert_constant(
        &mut self,
        constant: ConstantValue,
        domain: ConstraintDomain,
    ) -> Result<ExprDetails, SemanticError> {
        let node_index = self.insert_op(Operation::Constant(constant));
        Ok(ExprDetails::new(node_index, DEFAULT_SEGMENT, domain, None))
    }

    /// Looks up the specified variable value in the variable roots and returns the expression
    /// details if it is found. Otherwise, inserts the variable expression into the graph, adds it
    /// to the variable roots, and returns the resulting expression details.
    fn insert_variable(
        &mut self,
        symbol_table: &SymbolTable,
        variable_roots: &mut VariableRoots,
        domain: ConstraintDomain,
        variable_value: VariableValue,
        variable_expr: &Expression,
        context: &mut ExprContext,
    ) -> Result<ExprDetails, SemanticError> {
        if let Some(expr) = variable_roots.get(&variable_value) {
            // If the variable has already been inserted, return the existing expression details.
            Ok(*expr)
        } else {
            // Otherwise, insert the variable expression and create a new variable root.
            let expr =
                self.insert_expr(symbol_table, variable_expr, variable_roots, domain, context)?;
            variable_roots.insert(variable_value, expr);
            Ok(expr)
        }
    }

    /// Adds a trace element access to the graph and returns the node index, trace segment, and row
    /// offset.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The identifier is greater than overall number of columns in segment.
    /// - The segment is greater than the number of segments.
    fn insert_indexed_trace_access(
        &mut self,
        symbol_table: &SymbolTable,
        trace_access: &IndexedTraceAccess,
    ) -> Result<ExprDetails, SemanticError> {
        let segment_idx = trace_access.trace_segment() as usize;
        if segment_idx > symbol_table.segment_widths().len() {
            return Err(SemanticError::IndexOutOfRange(format!(
                "Segment index {} is greater than the number of segments in the trace ({}).",
                segment_idx,
                symbol_table.segment_widths().len()
            )));
        }
        if trace_access.col_idx() as u16 >= symbol_table.segment_widths()[segment_idx] {
            return Err(SemanticError::IndexOutOfRange(format!(
                "Out-of-range index {} in trace segment {} of length {}",
                trace_access.col_idx(),
                trace_access.trace_segment(),
                symbol_table.segment_widths()[segment_idx]
            )));
        }

        let trace_segment = trace_access.trace_segment();
        let domain = ConstraintDomain::from(trace_access.row_offset());
        let node_index = self.insert_op(Operation::TraceElement(*trace_access));
        Ok(ExprDetails::new(node_index, trace_segment, domain, None))
    }

    /// Adds a trace column, periodic column, named constant or a variable to the graph and returns
    /// the [ExprDetails] of the inserted expression.
    ///
    /// # Errors
    /// Returns an error if the identifier is not present in the symbol table or is not a supported
    /// type.
    fn insert_symbol_access(
        &mut self,
        symbol_table: &SymbolTable,
        ident: &str,
        variable_roots: &mut VariableRoots,
        domain: ConstraintDomain,
        context: &mut ExprContext,
    ) -> Result<ExprDetails, SemanticError> {
        let elem_type = symbol_table.get_type(ident)?;
        match elem_type {
            IdentifierType::Constant(ConstantType::Scalar(_)) => {
                self.insert_constant(ConstantValue::Scalar(ident.to_string()), domain)
            }
            IdentifierType::IntegrityVariable(integrity_variable) => {
                if let VariableType::Scalar(variable_expr) = integrity_variable.value() {
                    self.insert_variable(
                        symbol_table,
                        variable_roots,
                        domain,
                        VariableValue::Scalar(ident.to_string()),
                        variable_expr,
                        context,
                    )
                } else {
                    Err(SemanticError::InvalidUsage(format!(
                        "Identifier {ident} was declared as a {elem_type} which is not a supported type."
                    )))
                }
            }
            IdentifierType::PeriodicColumn(index, cycle_len) => {
                let node_index = self.insert_op(Operation::PeriodicColumn(*index, *cycle_len));
                Ok(ExprDetails::new(node_index, DEFAULT_SEGMENT, domain, None))
            }
            IdentifierType::TraceColumns(columns) => {
                let trace_segment = columns.trace_segment();
                let trace_access =
                    IndexedTraceAccess::new(trace_segment, columns.offset(), CURRENT_ROW);
                let node_index = self.insert_op(Operation::TraceElement(trace_access));
                Ok(ExprDetails::new(node_index, trace_segment, domain, None))
            }
            _ => {
                if !context.list_comprehension_context.context().is_empty() {
                    for (member, iterable) in context.list_comprehension_context.context().iter() {
                        if member.name() == ident {
                            if let Iterable::Identifier(ident) = iterable {
                                let iterable_type = symbol_table.get_type(ident.name())?;
                                match iterable_type {
                                    IdentifierType::TraceColumns(columns) => {
                                        let trace_segment = columns.trace_segment();
                                        let trace_access = IndexedTraceAccess::new(
                                            trace_segment,
                                            columns.offset(),
                                            CURRENT_ROW,
                                        );
                                        let node_index =
                                            self.insert_op(Operation::TraceElement(trace_access));
                                        return Ok(ExprDetails::new(
                                            node_index,
                                            trace_segment,
                                            domain,
                                            None,
                                        ));
                                    }
                                    // TODO: Add support for other iterable types.
                                    // TODO: Add support for list comprehensions in list comprehensions.
                                    // TODO: Add support for next ([x' - x for x in a])
                                    _ => todo!(),
                                }
                            }
                        }
                    }
                }
                Err(SemanticError::InvalidUsage(format!(
                    "Identifier {ident} was declared as a {elem_type} which is not a supported type."
                )))
            }
        }
    }

    /// Validates and adds a vector access to the graph and returns the [ExprDetails] of the
    /// inserted expression.
    ///
    /// # Errors
    /// Returns an error if the identifier's value is not of a supported type.
    fn insert_vector_access(
        &mut self,
        symbol_table: &SymbolTable,
        vector_access: &VectorAccess,
        variable_roots: &mut VariableRoots,
        domain: ConstraintDomain,
        context: &mut ExprContext,
    ) -> Result<ExprDetails, SemanticError> {
        let symbol_type = symbol_table.access_vector_element(vector_access)?;
        match symbol_type {
            IdentifierType::Constant(ConstantType::Vector(_)) => {
                self.insert_constant(ConstantValue::Vector(vector_access.clone()), domain)
            }
            IdentifierType::IntegrityVariable(integrity_variable) => {
                match integrity_variable.value() {
                    VariableType::Vector(vector) => self.insert_variable(
                        symbol_table,
                        variable_roots,
                        domain,
                        VariableValue::Vector(vector_access.clone()),
                        &vector[vector_access.idx()],
                        context,
                    ),
                    VariableType::ListComprehension(list_comprehension) => {
                        let list_comprehension_context = ListComprehensionContext::new(
                            list_comprehension.context().to_vec(),
                            vector_access.idx(),
                        );
                        context.set_list_comprehension_context(list_comprehension_context);
                        self.insert_variable(
                            symbol_table,
                            variable_roots,
                            domain,
                            VariableValue::ListComprehension(vector_access.clone()),
                            list_comprehension.expression(),
                            context,
                        )
                    }
                    _ => Err(SemanticError::invalid_vector_access(
                        vector_access,
                        symbol_type,
                    )),
                }
            }
            IdentifierType::TraceColumns(columns) => {
                let trace_segment = columns.trace_segment();
                let trace_access = IndexedTraceAccess::new(
                    trace_segment,
                    columns.offset() + vector_access.idx(),
                    CURRENT_ROW,
                );
                let node_index = self.insert_op(Operation::TraceElement(trace_access));
                Ok(ExprDetails::new(node_index, trace_segment, domain, None))
            }
            IdentifierType::PublicInput(_) => {
                unimplemented!("TODO: add support for public inputs.")
            }
            _ => Err(SemanticError::invalid_vector_access(
                vector_access,
                symbol_type,
            )),
        }
    }

    /// Validates and adds a matrix access to the graph and returns the [ExprDetails] of the
    /// inserted expression.
    ///
    /// # Errors
    /// Returns an error if the identifier's value is not of a supported type.
    fn insert_matrix_access(
        &mut self,
        symbol_table: &SymbolTable,
        matrix_access: &MatrixAccess,
        variable_roots: &mut VariableRoots,
        domain: ConstraintDomain,
        context: &mut ExprContext,
    ) -> Result<ExprDetails, SemanticError> {
        let symbol_type = symbol_table.access_matrix_element(matrix_access)?;
        match symbol_type {
            IdentifierType::Constant(ConstantType::Matrix(_)) => {
                self.insert_constant(ConstantValue::Matrix(matrix_access.clone()), domain)
            }
            IdentifierType::IntegrityVariable(integrity_variable) => {
                if let VariableType::Matrix(matrix) = integrity_variable.value() {
                    self.insert_variable(
                        symbol_table,
                        variable_roots,
                        domain,
                        VariableValue::Matrix(matrix_access.clone()),
                        &matrix[matrix_access.row_idx()][matrix_access.col_idx()],
                        context,
                    )
                } else {
                    Err(SemanticError::invalid_matrix_access(
                        matrix_access,
                        symbol_type,
                    ))
                }
            }
            _ => Err(SemanticError::invalid_matrix_access(
                matrix_access,
                symbol_type,
            )),
        }
    }
}

/// Reference to a node in a graph by its index in the nodes vector of the graph struct.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub struct NodeIndex(usize);

#[derive(Debug)]
pub struct Node {
    /// The operation represented by this node
    op: Operation,
}

impl Node {
    pub fn op(&self) -> &Operation {
        &self.op
    }
}

/// An integrity constraint operation or value reference.
#[derive(Debug, Eq, PartialEq)]
pub enum Operation {
    /// An inlined or named constant with identifier and access indices.
    Constant(ConstantValue),
    /// An identifier for an element in the trace segment, column, and row offset specified by the
    /// [IndexedTraceAccess]
    TraceElement(IndexedTraceAccess),
    /// An identifier for a periodic value from a specified periodic column. The first inner value
    /// is the index of the periodic column within the declared periodic columns. The second inner
    /// value is the length of the column's periodic cycle. The periodic value made available from
    /// the specified column is based on the current row of the trace.
    PeriodicColumn(usize, usize),
    /// A random value provided by the verifier. The inner value is the index of this random value
    /// in the array of all random values.
    RandomValue(usize),
    /// Negation operation applied to the node with the specified index.
    Neg(NodeIndex),
    /// Addition operation applied to the nodes with the specified indices.
    Add(NodeIndex, NodeIndex),
    /// Subtraction operation applied to the nodes with the specified indices.
    Sub(NodeIndex, NodeIndex),
    /// Multiplication operation applied to the nodes with the specified indices.
    Mul(NodeIndex, NodeIndex),
    /// Exponentiation operation applied to the node with the specified index, using the provided
    /// value as the power.
    /// TODO: Support non const exponents.
    Exp(NodeIndex, usize),
}

#[derive(Debug, Eq, PartialEq)]
pub enum ConstantValue {
    Inline(u64),
    Scalar(String),
    Vector(VectorAccess),
    Matrix(MatrixAccess),
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub enum VariableValue {
    Scalar(String),
    Vector(VectorAccess),
    ListComprehension(VectorAccess),
    Matrix(MatrixAccess),
}

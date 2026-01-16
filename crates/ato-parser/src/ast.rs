//! Abstract Syntax Tree types for the Ato language.

use ato_lexer::Span;
use serde::{Deserialize, Serialize};

/// A complete Ato source file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct File {
    pub statements: Vec<Statement>,
    pub span: Span,
}

/// A statement in the Ato language.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Statement {
    /// `#pragma experiment("NAME")`
    Pragma(PragmaStmt),
    /// `import X` or `from "path" import X`
    Import(ImportStmt),
    /// `import X from "path"` (deprecated form)
    DepImport(DepImportStmt),
    /// `module M:`, `component C:`, `interface I:`
    BlockDef(BlockDef),
    /// `x = value`
    Assignment(Assignment),
    /// `x += value` or `x -= value`
    CumAssignment(CumAssignment),
    /// `x |= value` or `x &= value`
    SetAssignment(SetAssignment),
    /// `a ~ b`
    Connection(Connection),
    /// `a ~> b ~> c` or `a <~ b <~ c`
    DirectedConnection(DirectedConnection),
    /// `x -> Type`
    Retype(Retype),
    /// `pin name`
    PinDeclaration(PinDeclaration),
    /// `signal name`
    SignalDef(SignalDef),
    /// `assert x within 1V to 2V`
    Assert(AssertStmt),
    /// `field: type`
    Declaration(Declaration),
    /// String literal as statement (docstring)
    StringStmt(StringStmt),
    /// `pass`
    Pass(PassStmt),
    /// `trait name<arg=value>`
    Trait(TraitStmt),
    /// `for item in container: body`
    For(ForStmt),
}

impl Statement {
    pub fn span(&self) -> Span {
        match self {
            Statement::Pragma(s) => s.span,
            Statement::Import(s) => s.span,
            Statement::DepImport(s) => s.span,
            Statement::BlockDef(s) => s.span,
            Statement::Assignment(s) => s.span,
            Statement::CumAssignment(s) => s.span,
            Statement::SetAssignment(s) => s.span,
            Statement::Connection(s) => s.span,
            Statement::DirectedConnection(s) => s.span,
            Statement::Retype(s) => s.span,
            Statement::PinDeclaration(s) => s.span,
            Statement::SignalDef(s) => s.span,
            Statement::Assert(s) => s.span,
            Statement::Declaration(s) => s.span,
            Statement::StringStmt(s) => s.span,
            Statement::Pass(s) => s.span,
            Statement::Trait(s) => s.span,
            Statement::For(s) => s.span,
        }
    }
}

// === Pragma ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PragmaStmt {
    pub content: String,
    pub span: Span,
}

// === Imports ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportStmt {
    /// Optional `from "path"` source
    pub from_path: Option<StringLiteral>,
    /// List of imported type references
    pub imports: Vec<TypeRef>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DepImportStmt {
    /// The imported type
    pub type_ref: TypeRef,
    /// The source path
    pub from_path: StringLiteral,
    pub span: Span,
}

// === Block Definitions ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockDef {
    pub kind: BlockKind,
    pub name: Identifier,
    /// Optional `from Base` super type
    pub super_type: Option<TypeRef>,
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockKind {
    Module,
    Component,
    Interface,
}

// === Assignments ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    pub target: AssignTarget,
    pub value: Assignable,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AssignTarget {
    FieldRef(FieldRef),
    Declaration(Declaration),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CumAssignment {
    pub target: AssignTarget,
    pub operator: CumOperator,
    pub value: Expression,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CumOperator {
    Add, // +=
    Sub, // -=
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetAssignment {
    pub target: AssignTarget,
    pub operator: SetOperator,
    pub value: Expression,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SetOperator {
    Or,  // |=
    And, // &=
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Assignable {
    String(StringLiteral),
    New(NewExpr),
    Physical(PhysicalLiteral),
    Arithmetic(Expression),
    Boolean(BoolLiteral),
}

// === Connections ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    pub left: Connectable,
    pub right: Connectable,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirectedConnection {
    pub direction: ConnectionDirection,
    pub elements: Vec<Connectable>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionDirection {
    Forward,  // ~>
    Backward, // <~
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Connectable {
    FieldRef(FieldRef),
    SignalDef(SignalDef),
    PinDef(PinDeclaration),
}

// === Retype ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Retype {
    pub field: FieldRef,
    pub new_type: TypeRef,
    pub span: Span,
}

// === Declarations ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Declaration {
    pub field: FieldRef,
    pub type_info: Identifier,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PinDeclaration {
    pub name: PinName,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PinName {
    Identifier(Identifier),
    Number(NumberLiteral),
    String(StringLiteral),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignalDef {
    pub name: Identifier,
    pub span: Span,
}

// === Assert ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssertStmt {
    pub comparison: Comparison,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    pub left: Expression,
    pub operations: Vec<CompareOp>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompareOp {
    pub kind: CompareOpKind,
    pub right: Expression,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareOpKind {
    LessThan,
    GreaterThan,
    LessEq,
    GreaterEq,
    Within,
    Is,
}

// === String/Pass Statements ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StringStmt {
    pub value: StringLiteral,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassStmt {
    pub span: Span,
}

// === Trait ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraitStmt {
    /// Optional target field
    pub target: Option<FieldRef>,
    pub type_ref: TypeRef,
    /// Optional constructor name
    pub constructor: Option<Identifier>,
    /// Optional template arguments
    pub template: Option<Template>,
    pub span: Span,
}

// === For Loop ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForStmt {
    pub variable: Identifier,
    pub iterable: Iterable,
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Iterable {
    FieldRef { field: FieldRef, slice: Option<Slice> },
    List(Vec<FieldRef>),
}

// === Expressions ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    /// Binary operation
    Binary(Box<BinaryExpr>),
    /// Unary operation (e.g., -x)
    Unary(Box<UnaryExpr>),
    /// Literal value
    Literal(Literal),
    /// Field reference (a.b.c, a[0])
    FieldRef(FieldRef),
    /// Grouped expression: (expr)
    Group(Box<Expression>),
    /// Function call: name(args)
    FunctionCall(FunctionCall),
}

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::Binary(e) => e.span,
            Expression::Unary(e) => e.span,
            Expression::Literal(l) => l.span(),
            Expression::FieldRef(f) => f.span,
            Expression::Group(e) => e.span(),
            Expression::FunctionCall(f) => f.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BinaryExpr {
    pub left: Expression,
    pub operator: BinaryOp,
    pub right: Expression,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,      // +
    Sub,      // -
    Mul,      // *
    Div,      // /
    Power,    // **
    BitOr,    // |
    BitAnd,   // &
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnaryExpr {
    pub operator: UnaryOp,
    pub operand: Expression,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Neg, // -
    Pos, // +
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: Identifier,
    pub args: Vec<Expression>,
    pub span: Span,
}

// === Literals ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    Number(NumberLiteral),
    String(StringLiteral),
    Bool(BoolLiteral),
    Physical(PhysicalLiteral),
}

impl Literal {
    pub fn span(&self) -> Span {
        match self {
            Literal::Number(n) => n.span,
            Literal::String(s) => s.span,
            Literal::Bool(b) => b.span,
            Literal::Physical(p) => p.span(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NumberLiteral {
    pub value: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StringLiteral {
    pub value: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoolLiteral {
    pub value: bool,
    pub span: Span,
}

/// Physical quantity literal (number with optional unit)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PhysicalLiteral {
    /// Simple quantity: `5V`, `10kohm`
    Quantity(Quantity),
    /// Range: `1V to 2V`
    Range(QuantityRange),
    /// Bilateral tolerance: `5V +/- 10%`
    Bilateral(BilateralQuantity),
}

impl PhysicalLiteral {
    pub fn span(&self) -> Span {
        match self {
            PhysicalLiteral::Quantity(q) => q.span,
            PhysicalLiteral::Range(r) => r.span,
            PhysicalLiteral::Bilateral(b) => b.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    pub number: NumberLiteral,
    pub unit: Option<Identifier>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuantityRange {
    pub from: Quantity,
    pub to: Quantity,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BilateralQuantity {
    pub base: Quantity,
    pub tolerance: Tolerance,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tolerance {
    pub value: String,
    pub is_percent: bool,
    pub unit: Option<Identifier>,
    pub span: Span,
}

// === New Expression ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewExpr {
    pub type_ref: TypeRef,
    /// Optional array count: `new Type[10]`
    pub count: Option<NumberLiteral>,
    /// Optional template args: `new Type<param=value>`
    pub template: Option<Template>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub args: Vec<TemplateArg>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateArg {
    pub name: Identifier,
    pub value: Literal,
    pub span: Span,
}

// === References ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldRef {
    pub parts: Vec<FieldRefPart>,
    /// Optional trailing pin reference: `.1`
    pub pin_ref: Option<NumberLiteral>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldRefPart {
    pub name: Identifier,
    /// Optional array index: `[0]`
    pub index: Option<NumberLiteral>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeRef {
    pub parts: Vec<Identifier>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slice {
    pub start: Option<NumberLiteral>,
    pub stop: Option<NumberLiteral>,
    pub step: Option<NumberLiteral>,
    pub span: Span,
}

// === Identifier ===

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Identifier {
    pub name: String,
    pub span: Span,
}

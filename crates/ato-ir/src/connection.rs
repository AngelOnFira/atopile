//! Connection types for the IR.
//!
//! Connections represent the electrical links between connectable fields
//! (pins, signals, and instances).

use crate::{ConnectionId, FieldId, FieldPath, ModuleId};
use ato_lexer::Span;
use serde::{Deserialize, Serialize};

/// A connection between two endpoints in the IR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// Unique identifier for this connection.
    pub id: ConnectionId,

    /// The module that contains this connection.
    pub parent: ModuleId,

    /// The connection endpoints.
    pub endpoints: Vec<ConnectionEndpoint>,

    /// The kind of connection.
    pub kind: ConnectionKind,

    /// Source location for error reporting.
    pub span: Option<Span>,
}

impl Connection {
    /// Create a new simple connection between two endpoints.
    pub fn new(
        id: ConnectionId,
        parent: ModuleId,
        left: ConnectionEndpoint,
        right: ConnectionEndpoint,
    ) -> Self {
        Self {
            id,
            parent,
            endpoints: vec![left, right],
            kind: ConnectionKind::Simple,
            span: None,
        }
    }

    /// Create a new directed (bridge) connection.
    pub fn directed(
        id: ConnectionId,
        parent: ModuleId,
        endpoints: Vec<ConnectionEndpoint>,
        direction: ConnectionDirection,
    ) -> Self {
        Self {
            id,
            parent,
            endpoints,
            kind: ConnectionKind::Directed(direction),
            span: None,
        }
    }

    /// Set the source span for this connection.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Check if this is a simple connection.
    pub fn is_simple(&self) -> bool {
        matches!(self.kind, ConnectionKind::Simple)
    }

    /// Check if this is a directed connection.
    pub fn is_directed(&self) -> bool {
        matches!(self.kind, ConnectionKind::Directed(_))
    }

    /// Get the left endpoint (first endpoint).
    pub fn left(&self) -> Option<&ConnectionEndpoint> {
        self.endpoints.first()
    }

    /// Get the right endpoint (second endpoint for simple connections).
    pub fn right(&self) -> Option<&ConnectionEndpoint> {
        self.endpoints.get(1)
    }
}

/// The kind of connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionKind {
    /// A simple bidirectional connection: `a ~ b`
    Simple,
    /// A directed (bridge) connection: `a ~> b ~> c` or `a <~ b <~ c`
    Directed(ConnectionDirection),
}

/// The direction of a directed connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionDirection {
    /// Forward direction: `~>`
    Forward,
    /// Backward direction: `<~`
    Backward,
}

/// An endpoint in a connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionEndpoint {
    /// The kind of endpoint.
    pub kind: EndpointKind,

    /// Resolved field ID (filled in during semantic analysis).
    pub resolved: Option<FieldId>,
}

impl ConnectionEndpoint {
    /// Create a field reference endpoint.
    pub fn field(path: FieldPath) -> Self {
        Self {
            kind: EndpointKind::FieldRef(path),
            resolved: None,
        }
    }

    /// Create a signal definition endpoint.
    pub fn signal(name: impl Into<String>) -> Self {
        Self {
            kind: EndpointKind::SignalDef(name.into()),
            resolved: None,
        }
    }

    /// Create a pin definition endpoint.
    pub fn pin(name: impl Into<String>) -> Self {
        Self {
            kind: EndpointKind::PinDef(name.into()),
            resolved: None,
        }
    }

    /// Create an endpoint with a resolved field ID.
    pub fn resolved(field_id: FieldId) -> Self {
        Self {
            kind: EndpointKind::Resolved,
            resolved: Some(field_id),
        }
    }

    /// Check if this endpoint is resolved.
    pub fn is_resolved(&self) -> bool {
        self.resolved.is_some()
    }

    /// Get the display name for this endpoint.
    pub fn display_name(&self) -> String {
        match &self.kind {
            EndpointKind::FieldRef(path) => {
                path.parts
                    .iter()
                    .map(|p| match p {
                        crate::FieldPathPart::Name(n) => n.clone(),
                        crate::FieldPathPart::Index(i) => format!("[{}]", i),
                        crate::FieldPathPart::PinRef(n) => format!(".{}", n),
                    })
                    .collect::<Vec<_>>()
                    .join(".")
            }
            EndpointKind::SignalDef(name) => format!("signal {}", name),
            EndpointKind::PinDef(name) => format!("pin {}", name),
            EndpointKind::Resolved => format!("Field({:?})", self.resolved),
        }
    }
}

/// The kind of connection endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EndpointKind {
    /// A reference to an existing field.
    FieldRef(FieldPath),
    /// An inline signal definition.
    SignalDef(String),
    /// An inline pin definition.
    PinDef(String),
    /// A resolved field (used after semantic analysis).
    Resolved,
}

/// A connection graph for efficient querying of connections.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConnectionGraph {
    /// Map from field ID to connected field IDs.
    connections: std::collections::HashMap<FieldId, Vec<FieldId>>,
}

impl ConnectionGraph {
    /// Create a new empty connection graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a connection between two fields.
    pub fn add_connection(&mut self, a: FieldId, b: FieldId) {
        self.connections.entry(a).or_default().push(b);
        self.connections.entry(b).or_default().push(a);
    }

    /// Get all fields connected to a given field.
    pub fn connected_to(&self, field: FieldId) -> &[FieldId] {
        self.connections.get(&field).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Check if two fields are directly connected.
    pub fn are_connected(&self, a: FieldId, b: FieldId) -> bool {
        self.connections
            .get(&a)
            .map(|v| v.contains(&b))
            .unwrap_or(false)
    }

    /// Get all fields that are transitively connected to a given field.
    pub fn connected_component(&self, start: FieldId) -> Vec<FieldId> {
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![start];

        while let Some(current) = stack.pop() {
            if visited.insert(current) {
                for &neighbor in self.connected_to(current) {
                    if !visited.contains(&neighbor) {
                        stack.push(neighbor);
                    }
                }
            }
        }

        visited.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_connection() {
        let conn = Connection::new(
            ConnectionId::new(0),
            ModuleId::new(0),
            ConnectionEndpoint::field(FieldPath::simple("a")),
            ConnectionEndpoint::field(FieldPath::simple("b")),
        );

        assert!(conn.is_simple());
        assert!(!conn.is_directed());
        assert_eq!(conn.endpoints.len(), 2);
    }

    #[test]
    fn test_directed_connection() {
        let conn = Connection::directed(
            ConnectionId::new(0),
            ModuleId::new(0),
            vec![
                ConnectionEndpoint::field(FieldPath::simple("a")),
                ConnectionEndpoint::field(FieldPath::simple("b")),
                ConnectionEndpoint::field(FieldPath::simple("c")),
            ],
            ConnectionDirection::Forward,
        );

        assert!(!conn.is_simple());
        assert!(conn.is_directed());
        assert_eq!(conn.endpoints.len(), 3);
    }

    #[test]
    fn test_endpoint_kinds() {
        let field_ep = ConnectionEndpoint::field(FieldPath::simple("pin1"));
        let signal_ep = ConnectionEndpoint::signal("sig1");
        let pin_ep = ConnectionEndpoint::pin("p1");

        assert_eq!(field_ep.display_name(), "pin1");
        assert_eq!(signal_ep.display_name(), "signal sig1");
        assert_eq!(pin_ep.display_name(), "pin p1");
    }

    #[test]
    fn test_connection_graph() {
        let mut graph = ConnectionGraph::new();

        let f0 = FieldId::new(0);
        let f1 = FieldId::new(1);
        let f2 = FieldId::new(2);
        let f3 = FieldId::new(3);

        // Connect f0 -- f1 -- f2, and f3 is isolated
        graph.add_connection(f0, f1);
        graph.add_connection(f1, f2);

        assert!(graph.are_connected(f0, f1));
        assert!(graph.are_connected(f1, f0)); // Bidirectional
        assert!(graph.are_connected(f1, f2));
        assert!(!graph.are_connected(f0, f2)); // Not directly connected
        assert!(!graph.are_connected(f0, f3)); // f3 is isolated

        // Transitive closure
        let component = graph.connected_component(f0);
        assert_eq!(component.len(), 3);
        assert!(component.contains(&f0));
        assert!(component.contains(&f1));
        assert!(component.contains(&f2));
    }
}

//! Solver tests - verify constraint solving works correctly.

use ato_domain::Unit;
use ato_solver::{Expression, Literal, Parameter, Predicate, Solver, SolverConfig, SolverError};

/// Create a solver with default config.
fn new_solver() -> Solver {
    Solver::new(SolverConfig::default())
}

// Basic solver tests
#[test]
fn test_solver_empty() {
    let mut solver = new_solver();
    let result = solver.solve();
    assert!(result.is_ok());
    assert!(result.unwrap().all_satisfied);
}

#[test]
fn test_solver_simple_true_comparison() {
    let mut solver = new_solver();
    // 2 <= 3 (true)
    let two = solver.add_expression(Expression::literal(Literal::Integer(2)));
    let three = solver.add_expression(Expression::literal(Literal::Integer(3)));
    solver.constrain(Predicate::less_or_equal(two, three));
    let result = solver.solve();
    assert!(result.is_ok());
    assert!(result.unwrap().all_satisfied);
}

#[test]
fn test_solver_equality() {
    let mut solver = new_solver();
    // 42 is 42 (true)
    let left = solver.add_expression(Expression::literal(Literal::Integer(42)));
    let right = solver.add_expression(Expression::literal(Literal::Integer(42)));
    solver.constrain(Predicate::is(left, right));
    let result = solver.solve();
    assert!(result.is_ok());
}

#[test]
fn test_solver_contradiction() {
    let mut solver = new_solver();
    // 5 is 3 (contradiction)
    let five = solver.add_expression(Expression::literal(Literal::Integer(5)));
    let three = solver.add_expression(Expression::literal(Literal::Integer(3)));
    solver.constrain(Predicate::is(five, three));
    let result = solver.solve();
    assert!(matches!(result, Err(SolverError::Contradiction(_))));
}

#[test]
fn test_solver_less_or_equal_true() {
    let mut solver = new_solver();
    // 5 <= 10 (true)
    let five = solver.add_expression(Expression::literal(Literal::Integer(5)));
    let ten = solver.add_expression(Expression::literal(Literal::Integer(10)));
    solver.constrain(Predicate::less_or_equal(five, ten));
    let result = solver.solve();
    assert!(result.is_ok());
}

#[test]
fn test_solver_greater_or_equal_true() {
    let mut solver = new_solver();
    // 10 >= 5 (true)
    let ten = solver.add_expression(Expression::literal(Literal::Integer(10)));
    let five = solver.add_expression(Expression::literal(Literal::Integer(5)));
    solver.constrain(Predicate::greater_or_equal(ten, five));
    let result = solver.solve();
    assert!(result.is_ok());
}

// Quantity with units tests
#[test]
fn test_solver_voltage_comparison() {
    let mut solver = new_solver();
    // 5V <= 10V (true)
    let five_v = solver.add_expression(Expression::literal(Literal::from_quantity(5.0, Unit::Volt)));
    let ten_v = solver.add_expression(Expression::literal(Literal::from_quantity(10.0, Unit::Volt)));
    solver.constrain(Predicate::less_or_equal(five_v, ten_v));
    let result = solver.solve();
    assert!(result.is_ok());
    assert!(result.unwrap().all_satisfied);
}

#[test]
fn test_solver_resistance_comparison() {
    let mut solver = new_solver();
    // 1k ohm <= 10k ohm (true)
    let one_k = solver.add_expression(Expression::literal(Literal::from_quantity(1000.0, Unit::Ohm)));
    let ten_k = solver.add_expression(Expression::literal(Literal::from_quantity(10000.0, Unit::Ohm)));
    solver.constrain(Predicate::less_or_equal(one_k, ten_k));
    let result = solver.solve();
    assert!(result.is_ok());
}

#[test]
fn test_solver_current_comparison() {
    let mut solver = new_solver();
    // 1mA <= 1A (true)
    let one_ma = solver.add_expression(Expression::literal(Literal::from_quantity(0.001, Unit::Ampere)));
    let one_a = solver.add_expression(Expression::literal(Literal::from_quantity(1.0, Unit::Ampere)));
    solver.constrain(Predicate::less_or_equal(one_ma, one_a));
    let result = solver.solve();
    assert!(result.is_ok());
}

// Parameter tests
#[test]
fn test_solver_add_parameter() {
    let mut solver = new_solver();
    let param = Parameter::new(Unit::Volt).with_name("voltage");
    let id = solver.add_parameter(param);
    assert!(solver.get_parameter(id).is_some());
}

#[test]
fn test_solver_multiple_parameters() {
    let mut solver = new_solver();
    let v_param = Parameter::new(Unit::Volt).with_name("voltage");
    let r_param = Parameter::new(Unit::Ohm).with_name("resistance");
    let v_id = solver.add_parameter(v_param);
    let r_id = solver.add_parameter(r_param);
    assert!(solver.get_parameter(v_id).is_some());
    assert!(solver.get_parameter(r_id).is_some());
    assert_ne!(v_id, r_id);
}

// Multiple constraint tests
#[test]
fn test_solver_multiple_constraints_consistent() {
    let mut solver = new_solver();
    // 5 <= 10 AND 3 <= 5 (both true)
    let five = solver.add_expression(Expression::literal(Literal::Integer(5)));
    let ten = solver.add_expression(Expression::literal(Literal::Integer(10)));
    let three = solver.add_expression(Expression::literal(Literal::Integer(3)));
    let five2 = solver.add_expression(Expression::literal(Literal::Integer(5)));
    solver.constrain(Predicate::less_or_equal(five, ten));
    solver.constrain(Predicate::less_or_equal(three, five2));
    let result = solver.solve();
    assert!(result.is_ok());
}

#[test]
fn test_solver_chain_of_constraints() {
    let mut solver = new_solver();
    // 1 <= 2 AND 2 <= 3 AND 3 <= 4
    let one = solver.add_expression(Expression::literal(Literal::Integer(1)));
    let two = solver.add_expression(Expression::literal(Literal::Integer(2)));
    let two2 = solver.add_expression(Expression::literal(Literal::Integer(2)));
    let three = solver.add_expression(Expression::literal(Literal::Integer(3)));
    let three2 = solver.add_expression(Expression::literal(Literal::Integer(3)));
    let four = solver.add_expression(Expression::literal(Literal::Integer(4)));
    solver.constrain(Predicate::less_or_equal(one, two));
    solver.constrain(Predicate::less_or_equal(two2, three));
    solver.constrain(Predicate::less_or_equal(three2, four));
    let result = solver.solve();
    assert!(result.is_ok());
}

// Float literal tests
#[test]
fn test_solver_float_equality() {
    let mut solver = new_solver();
    let pi = solver.add_expression(Expression::literal(Literal::Float(3.14159)));
    let pi2 = solver.add_expression(Expression::literal(Literal::Float(3.14159)));
    solver.constrain(Predicate::is(pi, pi2));
    let result = solver.solve();
    assert!(result.is_ok());
}

#[test]
fn test_solver_float_comparison() {
    let mut solver = new_solver();
    let small = solver.add_expression(Expression::literal(Literal::Float(0.001)));
    let large = solver.add_expression(Expression::literal(Literal::Float(1000.0)));
    solver.constrain(Predicate::less_or_equal(small, large));
    let result = solver.solve();
    assert!(result.is_ok());
}

// Predicate counting tests
#[test]
fn test_solver_predicate_count() {
    let mut solver = new_solver();
    assert_eq!(solver.predicate_count(), 0);

    let one = solver.add_expression(Expression::literal(Literal::Integer(1)));
    let two = solver.add_expression(Expression::literal(Literal::Integer(2)));
    solver.constrain(Predicate::less_or_equal(one, two));
    assert_eq!(solver.predicate_count(), 1);

    let three = solver.add_expression(Expression::literal(Literal::Integer(3)));
    let four = solver.add_expression(Expression::literal(Literal::Integer(4)));
    solver.constrain(Predicate::less_or_equal(three, four));
    assert_eq!(solver.predicate_count(), 2);
}

#[test]
fn test_solver_constrained_count() {
    let mut solver = new_solver();
    assert_eq!(solver.constrained_count(), 0);

    let one = solver.add_expression(Expression::literal(Literal::Integer(1)));
    let two = solver.add_expression(Expression::literal(Literal::Integer(2)));
    solver.constrain(Predicate::less_or_equal(one, two));
    assert_eq!(solver.constrained_count(), 1);
}

// Solver state tests
#[test]
fn test_solver_state_iterations() {
    let mut solver = new_solver();
    let result = solver.solve().unwrap();
    assert!(result.iterations >= 1);
}

#[test]
fn test_solver_state_elapsed() {
    let mut solver = new_solver();
    let result = solver.solve().unwrap();
    // Elapsed time should be non-negative (just check it doesn't panic)
    let _ = result.elapsed;
}

// Config tests
#[test]
fn test_solver_config_default() {
    let config = SolverConfig::default();
    assert!(config.max_iterations > 0);
    assert!(config.timeout.as_secs() > 0);
}

#[test]
fn test_solver_config_custom() {
    let config = SolverConfig {
        max_iterations: 100,
        timeout: std::time::Duration::from_secs(10),
        terminal: false,
        allow_partial: true,
    };
    let solver = Solver::new(config);
    // Just verify it doesn't panic
    let _ = solver;
}

// Default solver tests
#[test]
fn test_default_solver() {
    let solver = Solver::default();
    assert_eq!(solver.predicate_count(), 0);
}

// Expression retrieval tests
#[test]
fn test_get_expression() {
    let mut solver = new_solver();
    let expr = Expression::literal(Literal::Integer(42));
    let id = solver.add_expression(expr);
    let retrieved = solver.get_expression(id);
    assert!(retrieved.is_some());
}

#[test]
fn test_get_nonexistent_expression() {
    let solver = new_solver();
    use ato_solver::ExpressionId;
    let fake_id = ExpressionId::new();
    let retrieved = solver.get_expression(fake_id);
    assert!(retrieved.is_none());
}

// Predicate retrieval tests
#[test]
fn test_get_predicate() {
    let mut solver = new_solver();
    let one = solver.add_expression(Expression::literal(Literal::Integer(1)));
    let two = solver.add_expression(Expression::literal(Literal::Integer(2)));
    let pred = Predicate::less_or_equal(one, two);
    let id = solver.add_predicate(pred);
    let retrieved = solver.get_predicate(id);
    assert!(retrieved.is_some());
}

// Various integer values
#[test]
fn test_solver_zero() {
    let mut solver = new_solver();
    let zero = solver.add_expression(Expression::literal(Literal::Integer(0)));
    let one = solver.add_expression(Expression::literal(Literal::Integer(1)));
    solver.constrain(Predicate::less_or_equal(zero, one));
    let result = solver.solve();
    assert!(result.is_ok());
}

#[test]
fn test_solver_negative_numbers() {
    let mut solver = new_solver();
    let neg = solver.add_expression(Expression::literal(Literal::Integer(-10)));
    let zero = solver.add_expression(Expression::literal(Literal::Integer(0)));
    solver.constrain(Predicate::less_or_equal(neg, zero));
    let result = solver.solve();
    assert!(result.is_ok());
}

#[test]
fn test_solver_large_numbers() {
    let mut solver = new_solver();
    let million = solver.add_expression(Expression::literal(Literal::Integer(1_000_000)));
    let billion = solver.add_expression(Expression::literal(Literal::Integer(1_000_000_000)));
    solver.constrain(Predicate::less_or_equal(million, billion));
    let result = solver.solve();
    assert!(result.is_ok());
}

// Different units
#[test]
fn test_solver_farad() {
    let mut solver = new_solver();
    let small_cap = solver.add_expression(Expression::literal(Literal::from_quantity(1e-9, Unit::Farad)));
    let large_cap = solver.add_expression(Expression::literal(Literal::from_quantity(1e-6, Unit::Farad)));
    solver.constrain(Predicate::less_or_equal(small_cap, large_cap));
    let result = solver.solve();
    assert!(result.is_ok());
}

#[test]
fn test_solver_hertz() {
    let mut solver = new_solver();
    let low_freq = solver.add_expression(Expression::literal(Literal::from_quantity(1000.0, Unit::Hertz)));
    let high_freq = solver.add_expression(Expression::literal(Literal::from_quantity(1_000_000.0, Unit::Hertz)));
    solver.constrain(Predicate::less_or_equal(low_freq, high_freq));
    let result = solver.solve();
    assert!(result.is_ok());
}

#[test]
fn test_solver_watt() {
    let mut solver = new_solver();
    let low_power = solver.add_expression(Expression::literal(Literal::from_quantity(0.1, Unit::Watt)));
    let high_power = solver.add_expression(Expression::literal(Literal::from_quantity(10.0, Unit::Watt)));
    solver.constrain(Predicate::less_or_equal(low_power, high_power));
    let result = solver.solve();
    assert!(result.is_ok());
}

// Dimensionless values
#[test]
fn test_solver_dimensionless() {
    let mut solver = new_solver();
    let ratio1 = solver.add_expression(Expression::literal(Literal::from_quantity(0.5, Unit::Dimensionless)));
    let ratio2 = solver.add_expression(Expression::literal(Literal::from_quantity(1.0, Unit::Dimensionless)));
    solver.constrain(Predicate::less_or_equal(ratio1, ratio2));
    let result = solver.solve();
    assert!(result.is_ok());
}

// Edge cases
#[test]
fn test_solver_same_value_less_or_equal() {
    let mut solver = new_solver();
    // 5 <= 5 (true)
    let five1 = solver.add_expression(Expression::literal(Literal::Integer(5)));
    let five2 = solver.add_expression(Expression::literal(Literal::Integer(5)));
    solver.constrain(Predicate::less_or_equal(five1, five2));
    let result = solver.solve();
    assert!(result.is_ok());
}

#[test]
fn test_solver_same_value_greater_or_equal() {
    let mut solver = new_solver();
    // 5 >= 5 (true)
    let five1 = solver.add_expression(Expression::literal(Literal::Integer(5)));
    let five2 = solver.add_expression(Expression::literal(Literal::Integer(5)));
    solver.constrain(Predicate::greater_or_equal(five1, five2));
    let result = solver.solve();
    assert!(result.is_ok());
}

//! Python bindings for Ato domain types.

use ato_domain::{BilateralTolerance, Interval, Quantity, QuantityInterval, Unit};
use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;

/// A unit of measurement.
#[pyclass(name = "Unit")]
#[derive(Clone)]
pub struct PyUnit {
    inner: Unit,
}

#[pymethods]
impl PyUnit {
    #[new]
    #[pyo3(signature = (name))]
    fn new(name: &str) -> PyResult<Self> {
        let unit = match name.to_lowercase().as_str() {
            "volt" | "v" => Unit::Volt,
            "ampere" | "amp" | "a" => Unit::Ampere,
            "ohm" | "ω" => Unit::Ohm,
            "farad" | "f" => Unit::Farad,
            "henry" | "h" => Unit::Henry,
            "watt" | "w" => Unit::Watt,
            "hertz" | "hz" => Unit::Hertz,
            "second" | "s" => Unit::Second,
            "meter" | "m" => Unit::Meter,
            "millimeter" | "mm" => Unit::Millimeter,
            "dimensionless" | "" => Unit::Dimensionless,
            _ => return Err(PyValueError::new_err(format!("Unknown unit: {}", name))),
        };
        Ok(Self { inner: unit })
    }

    fn __repr__(&self) -> String {
        format!("Unit({:?})", self.inner)
    }

    fn __str__(&self) -> String {
        format!("{:?}", self.inner)
    }

    /// Get the unit name.
    #[getter]
    fn name(&self) -> String {
        format!("{:?}", self.inner)
    }

    /// Create Volt unit.
    #[staticmethod]
    fn volt() -> Self {
        Self { inner: Unit::Volt }
    }

    /// Create Ampere unit.
    #[staticmethod]
    fn ampere() -> Self {
        Self { inner: Unit::Ampere }
    }

    /// Create Ohm unit.
    #[staticmethod]
    fn ohm() -> Self {
        Self { inner: Unit::Ohm }
    }

    /// Create Farad unit.
    #[staticmethod]
    fn farad() -> Self {
        Self { inner: Unit::Farad }
    }

    /// Create Henry unit.
    #[staticmethod]
    fn henry() -> Self {
        Self { inner: Unit::Henry }
    }

    /// Create Watt unit.
    #[staticmethod]
    fn watt() -> Self {
        Self { inner: Unit::Watt }
    }

    /// Create Hertz unit.
    #[staticmethod]
    fn hertz() -> Self {
        Self { inner: Unit::Hertz }
    }

    /// Create dimensionless unit.
    #[staticmethod]
    fn dimensionless() -> Self {
        Self { inner: Unit::Dimensionless }
    }
}

impl From<Unit> for PyUnit {
    fn from(unit: Unit) -> Self {
        Self { inner: unit }
    }
}

impl PyUnit {
    pub fn inner(&self) -> Unit {
        self.inner
    }
}

/// A numeric interval [min, max].
#[pyclass(name = "Interval")]
#[derive(Clone)]
pub struct PyInterval {
    inner: Interval,
}

#[pymethods]
impl PyInterval {
    #[new]
    #[pyo3(signature = (min, max))]
    fn new(min: f64, max: f64) -> PyResult<Self> {
        Interval::new(min, max)
            .map(|inner| Self { inner })
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    fn __repr__(&self) -> String {
        format!("Interval({}, {})", self.inner.lower(), self.inner.upper())
    }

    fn __str__(&self) -> String {
        format!("[{}, {}]", self.inner.lower(), self.inner.upper())
    }

    /// Get the minimum value.
    #[getter]
    fn min(&self) -> f64 {
        self.inner.lower()
    }

    /// Get the maximum value.
    #[getter]
    fn max(&self) -> f64 {
        self.inner.upper()
    }

    /// Check if the interval contains a value.
    fn contains(&self, value: f64) -> bool {
        self.inner.contains(value)
    }

    /// Check if this interval is a subset of another.
    fn is_subset_of(&self, other: &PyInterval) -> bool {
        self.inner.is_subset_of(&other.inner)
    }

    /// Check if this interval overlaps with another.
    fn overlaps(&self, other: &PyInterval) -> bool {
        self.inner.overlaps(&other.inner)
    }

    /// Get the intersection with another interval.
    fn intersection(&self, other: &PyInterval) -> PyResult<Self> {
        self.inner
            .intersect(&other.inner)
            .map(|inner| Self { inner })
            .ok_or_else(|| PyValueError::new_err("Intervals do not overlap"))
    }

    /// Get the union with another interval (if contiguous).
    fn union(&self, other: &PyInterval) -> PyResult<Self> {
        self.inner
            .try_merge(&other.inner)
            .map(|inner| Self { inner })
            .ok_or_else(|| PyValueError::new_err("Intervals are not contiguous"))
    }

    /// Check if this is a singleton (single point).
    fn is_singleton(&self) -> bool {
        self.inner.is_singleton()
    }

    /// Get the width of the interval.
    fn width(&self) -> f64 {
        self.inner.width()
    }

    /// Get the center point of the interval.
    fn center(&self) -> f64 {
        (self.inner.lower() + self.inner.upper()) / 2.0
    }

    /// Create a singleton interval.
    #[staticmethod]
    fn singleton(value: f64) -> PyResult<Self> {
        Interval::singleton(value)
            .map(|inner| Self { inner })
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }
}

impl PyInterval {
    pub fn inner(&self) -> &Interval {
        &self.inner
    }
}

/// A physical quantity with a unit.
#[pyclass(name = "Quantity")]
#[derive(Clone)]
pub struct PyQuantity {
    inner: Quantity,
}

#[pymethods]
impl PyQuantity {
    #[new]
    #[pyo3(signature = (value, unit))]
    fn new(value: f64, unit: &PyUnit) -> Self {
        Self {
            inner: Quantity::new(value, unit.inner()),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "Quantity({}, {:?})",
            self.inner.value(),
            self.inner.unit()
        )
    }

    fn __str__(&self) -> String {
        format!("{} {:?}", self.inner.value(), self.inner.unit())
    }

    /// Get the numeric value.
    #[getter]
    fn value(&self) -> f64 {
        self.inner.value()
    }

    /// Get the unit.
    #[getter]
    fn unit(&self) -> PyUnit {
        PyUnit::from(self.inner.unit())
    }

    /// Add two quantities.
    fn __add__(&self, other: &PyQuantity) -> PyResult<PyQuantity> {
        use std::ops::Add;
        self.inner
            .add(other.inner)
            .map(|inner| PyQuantity { inner })
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Subtract two quantities.
    fn __sub__(&self, other: &PyQuantity) -> PyResult<PyQuantity> {
        use std::ops::Sub;
        self.inner
            .sub(other.inner)
            .map(|inner| PyQuantity { inner })
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Multiply quantity by a scalar.
    fn __mul__(&self, other: f64) -> PyQuantity {
        use std::ops::Mul;
        PyQuantity {
            inner: self.inner.mul(other),
        }
    }

    /// Divide quantity by a scalar.
    fn __truediv__(&self, other: f64) -> PyQuantity {
        use std::ops::Div;
        PyQuantity {
            inner: self.inner.div(other),
        }
    }
}

impl From<Quantity> for PyQuantity {
    fn from(q: Quantity) -> Self {
        Self { inner: q }
    }
}

impl PyQuantity {
    pub fn inner(&self) -> &Quantity {
        &self.inner
    }
}

/// A quantity interval (range of quantities with same unit).
#[pyclass(name = "QuantityInterval")]
#[derive(Clone)]
pub struct PyQuantityInterval {
    inner: QuantityInterval,
}

#[pymethods]
impl PyQuantityInterval {
    #[new]
    #[pyo3(signature = (min, max))]
    fn new(min: &PyQuantity, max: &PyQuantity) -> PyResult<Self> {
        QuantityInterval::new(min.inner().clone(), max.inner().clone())
            .map(|inner| Self { inner })
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    fn __repr__(&self) -> String {
        format!(
            "QuantityInterval({} to {} {:?})",
            self.inner.min().value(),
            self.inner.max().value(),
            self.inner.unit()
        )
    }

    fn __str__(&self) -> String {
        format!(
            "[{} to {}] {:?}",
            self.inner.min().value(),
            self.inner.max().value(),
            self.inner.unit()
        )
    }

    /// Get the minimum quantity.
    #[getter]
    fn min(&self) -> PyQuantity {
        PyQuantity::from(self.inner.min())
    }

    /// Get the maximum quantity.
    #[getter]
    fn max(&self) -> PyQuantity {
        PyQuantity::from(self.inner.max())
    }

    /// Get the unit.
    #[getter]
    fn unit(&self) -> PyUnit {
        PyUnit::from(self.inner.unit())
    }

    /// Check if the interval contains a quantity.
    fn contains(&self, value: &PyQuantity) -> bool {
        self.inner.contains(value.inner())
    }

    /// Check if this interval is a subset of another.
    fn is_subset_of(&self, other: &PyQuantityInterval) -> bool {
        self.inner.is_subset_of(&other.inner)
    }

    /// Check if this is a singleton.
    fn is_singleton(&self) -> bool {
        self.inner.is_singleton()
    }

    /// Check if this is unbounded.
    fn is_unbounded(&self) -> bool {
        self.inner.is_unbounded()
    }

    /// Create a singleton quantity interval.
    #[staticmethod]
    fn singleton(value: &PyQuantity) -> PyResult<Self> {
        QuantityInterval::singleton(value.inner().clone())
            .map(|inner| Self { inner })
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Create a quantity interval from a tolerance.
    #[staticmethod]
    #[pyo3(signature = (center, tolerance_fraction))]
    fn from_tolerance(center: &PyQuantity, tolerance_fraction: f64) -> PyResult<Self> {
        let tolerance = BilateralTolerance::relative(
            center.inner().clone(),
            tolerance_fraction,
        );
        tolerance.to_interval()
            .map(|inner| Self { inner })
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }
}

impl PyQuantityInterval {
    pub fn inner(&self) -> &QuantityInterval {
        &self.inner
    }
}

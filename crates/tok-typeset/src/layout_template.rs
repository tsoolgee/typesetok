//! Layout Template and Constraint Validation Model.
//!
//! Encapsulates concrete geometric constraints and specifications synthesized for
//! multi-flow spread allocation, providing mathematical validity verification.

use crate::layout_family::LayoutFamily;
use crate::multi_flow::{FlowGeometrySpec, FlowPlacementRole};
use std::collections::HashSet;
use std::fmt;
use tok_core::FlowId;

/// Constraint validation errors detected in a synthesized or user-specified layout template.
#[derive(Debug, Clone, PartialEq)]
pub enum TemplateConstraintError {
    /// Template has no flow geometry specifications.
    EmptyFlowSpecs,
    /// Dimensions leave non-positive printable content area.
    InsufficientPageGeometry {
        printable_width_pt: f32,
        printable_height_pt: f32,
    },
    /// Invalid or non-finite width proportions.
    InvalidProportions { message: String },
    /// The number of column proportions does not match the number of allocated columns.
    ProportionCountMismatch {
        proportions_count: usize,
        columns_count: usize,
    },
    /// A referenced flow ID (e.g. expansion flow or footnote) does not exist in the flow specs.
    MissingReferencedFlow { role: &'static str, flow_id: FlowId },
    /// Duplicate flow ID encountered in flow geometry specifications.
    DuplicateFlowId(FlowId),
}

impl fmt::Display for TemplateConstraintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFlowSpecs => write!(f, "Template contains no flow specifications"),
            Self::InsufficientPageGeometry {
                printable_width_pt,
                printable_height_pt,
            } => {
                write!(
                    f,
                    "Printable page area is non-positive (width: {:.2} pt, height: {:.2} pt)",
                    printable_width_pt, printable_height_pt
                )
            }
            Self::InvalidProportions { message } => {
                write!(f, "Invalid width proportions: {}", message)
            }
            Self::ProportionCountMismatch {
                proportions_count,
                columns_count,
            } => {
                write!(
                    f,
                    "Proportion count ({}) does not match column count ({})",
                    proportions_count, columns_count
                )
            }
            Self::MissingReferencedFlow { role, flow_id } => {
                write!(
                    f,
                    "Referenced {} flow '{}' does not exist in template",
                    role, flow_id.0
                )
            }
            Self::DuplicateFlowId(id) => write!(f, "Duplicate flow ID '{}' in template", id.0),
        }
    }
}

impl std::error::Error for TemplateConstraintError {}

/// Synthesized, concrete layout template ready for execution by [`MultiFlowSolver`].
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutTemplate {
    pub family: LayoutFamily,
    pub flow_specs: Vec<FlowGeometrySpec>,
    pub nominal_proportions: Vec<f32>,
    pub expansion_flow_id: Option<FlowId>,
    pub footnote_flow_id: Option<FlowId>,
    pub gutter_pt: f32,
    pub has_l_shape_expansion: bool,
    pub has_bottom_band: bool,
}

impl LayoutTemplate {
    /// Creates a new layout template instance.
    pub fn new(family: LayoutFamily) -> Self {
        Self {
            family,
            flow_specs: Vec::new(),
            nominal_proportions: Vec::new(),
            expansion_flow_id: None,
            footnote_flow_id: None,
            gutter_pt: 12.0,
            has_l_shape_expansion: false,
            has_bottom_band: false,
        }
    }

    /// Number of parallel column streams (excluding floating bottom bands).
    pub fn column_count(&self) -> usize {
        self.flow_specs
            .iter()
            .filter(|s| s.role != FlowPlacementRole::BottomBand)
            .count()
    }

    /// Validates mathematical consistency and geometric integrity of the template constraints.
    pub fn validate(
        &self,
        page_width_pt: f32,
        page_height_pt: f32,
        margin_x_total_pt: f32,
        margin_y_total_pt: f32,
    ) -> Result<(), TemplateConstraintError> {
        if self.flow_specs.is_empty() {
            return Err(TemplateConstraintError::EmptyFlowSpecs);
        }

        let printable_w = page_width_pt - margin_x_total_pt;
        let printable_h = page_height_pt - margin_y_total_pt;
        if printable_w <= 0.0
            || printable_h <= 0.0
            || !printable_w.is_finite()
            || !printable_h.is_finite()
        {
            return Err(TemplateConstraintError::InsufficientPageGeometry {
                printable_width_pt: printable_w,
                printable_height_pt: printable_h,
            });
        }

        // Check for duplicate flow IDs
        let mut seen_ids = HashSet::new();
        for spec in &self.flow_specs {
            if !seen_ids.insert(&spec.flow_id) {
                return Err(TemplateConstraintError::DuplicateFlowId(
                    spec.flow_id.clone(),
                ));
            }
        }

        // Check referenced expansion flow ID
        if let Some(ref exp_id) = self.expansion_flow_id {
            if !seen_ids.contains(exp_id) {
                return Err(TemplateConstraintError::MissingReferencedFlow {
                    role: "expansion",
                    flow_id: exp_id.clone(),
                });
            }
        }

        // Check referenced footnote flow ID
        if let Some(ref fn_id) = self.footnote_flow_id {
            if !seen_ids.contains(fn_id) {
                return Err(TemplateConstraintError::MissingReferencedFlow {
                    role: "footnote",
                    flow_id: fn_id.clone(),
                });
            }
        }

        // Validate width proportions
        let cols = self.column_count();
        if !self.nominal_proportions.is_empty() {
            if self.nominal_proportions.len() != cols {
                return Err(TemplateConstraintError::ProportionCountMismatch {
                    proportions_count: self.nominal_proportions.len(),
                    columns_count: cols,
                });
            }

            let sum: f32 = self.nominal_proportions.iter().sum();
            if sum <= 0.0 || !sum.is_finite() {
                return Err(TemplateConstraintError::InvalidProportions {
                    message: format!(
                        "Sum of proportions ({:.3}) must be strictly positive and finite",
                        sum
                    ),
                });
            }

            for (idx, &prop) in self.nominal_proportions.iter().enumerate() {
                if prop <= 0.0 || !prop.is_finite() {
                    return Err(TemplateConstraintError::InvalidProportions {
                        message: format!("Proportion at index {} is invalid ({})", idx, prop),
                    });
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_template_validation() {
        let mut t = LayoutTemplate::new(LayoutFamily::SingleFlow {
            flow_id: FlowId::main(),
        });
        t.flow_specs.push(FlowGeometrySpec::new(FlowId::main(), 1));
        t.nominal_proportions = vec![1.0];

        let res = t.validate(595.0, 842.0, 80.0, 72.0);
        assert!(res.is_ok());
    }

    #[test]
    fn test_empty_flow_specs_error() {
        let t = LayoutTemplate::new(LayoutFamily::SingleFlow {
            flow_id: FlowId::main(),
        });
        let res = t.validate(595.0, 842.0, 80.0, 72.0);
        assert_eq!(res, Err(TemplateConstraintError::EmptyFlowSpecs));
    }

    #[test]
    fn test_insufficient_page_geometry_error() {
        let mut t = LayoutTemplate::new(LayoutFamily::SingleFlow {
            flow_id: FlowId::main(),
        });
        t.flow_specs.push(FlowGeometrySpec::new(FlowId::main(), 1));

        // Margins exceed page width (595.0 < 600.0)
        let res = t.validate(595.0, 842.0, 600.0, 72.0);
        match res {
            Err(TemplateConstraintError::InsufficientPageGeometry {
                printable_width_pt, ..
            }) => {
                assert!(printable_width_pt <= 0.0);
            }
            other => panic!("Expected InsufficientPageGeometry, got {:?}", other),
        }
    }

    #[test]
    fn test_duplicate_flow_id_error() {
        let mut t = LayoutTemplate::new(LayoutFamily::ParallelColumns {
            column_count: 2,
            proportions: None,
            flow_ids: vec![FlowId::new("col"), FlowId::new("col")],
        });
        t.flow_specs
            .push(FlowGeometrySpec::new(FlowId::new("col"), 1));
        t.flow_specs
            .push(FlowGeometrySpec::new(FlowId::new("col"), 2));

        let res = t.validate(595.0, 842.0, 80.0, 72.0);
        assert_eq!(
            res,
            Err(TemplateConstraintError::DuplicateFlowId(FlowId::new("col")))
        );
    }

    #[test]
    fn test_missing_referenced_expansion_flow_error() {
        let mut t = LayoutTemplate::new(LayoutFamily::TzuratHaDaf {
            primary_flow: FlowId::new("gemara"),
            spine_inner_flow: Some(FlowId::new("rashi")),
            spine_outer_flow: Some(FlowId::new("tosafot")),
            expansion_flow: Some(FlowId::new("non_existent_stream")),
            has_bottom_band: false,
        });
        t.flow_specs
            .push(FlowGeometrySpec::new(FlowId::new("gemara"), 1));
        t.flow_specs
            .push(FlowGeometrySpec::new(FlowId::new("rashi"), 2));
        t.flow_specs
            .push(FlowGeometrySpec::new(FlowId::new("tosafot"), 3));
        t.expansion_flow_id = Some(FlowId::new("non_existent_stream"));

        let res = t.validate(595.0, 842.0, 80.0, 72.0);
        assert_eq!(
            res,
            Err(TemplateConstraintError::MissingReferencedFlow {
                role: "expansion",
                flow_id: FlowId::new("non_existent_stream"),
            })
        );
    }

    #[test]
    fn test_proportions_count_mismatch_error() {
        let mut t = LayoutTemplate::new(LayoutFamily::ParallelColumns {
            column_count: 2,
            proportions: Some(vec![0.5, 0.3, 0.2]),
            flow_ids: vec![FlowId::new("c1"), FlowId::new("c2")],
        });
        t.flow_specs
            .push(FlowGeometrySpec::new(FlowId::new("c1"), 1));
        t.flow_specs
            .push(FlowGeometrySpec::new(FlowId::new("c2"), 2));
        t.nominal_proportions = vec![0.5, 0.3, 0.2]; // 3 proportions for 2 columns

        let res = t.validate(595.0, 842.0, 80.0, 72.0);
        assert_eq!(
            res,
            Err(TemplateConstraintError::ProportionCountMismatch {
                proportions_count: 3,
                columns_count: 2,
            })
        );
    }

    #[test]
    fn test_invalid_proportions_non_positive_error() {
        let mut t = LayoutTemplate::new(LayoutFamily::ParallelColumns {
            column_count: 2,
            proportions: Some(vec![0.5, -0.1]),
            flow_ids: vec![FlowId::new("c1"), FlowId::new("c2")],
        });
        t.flow_specs
            .push(FlowGeometrySpec::new(FlowId::new("c1"), 1));
        t.flow_specs
            .push(FlowGeometrySpec::new(FlowId::new("c2"), 2));
        t.nominal_proportions = vec![0.5, -0.1];

        let res = t.validate(595.0, 842.0, 80.0, 72.0);
        match res {
            Err(TemplateConstraintError::InvalidProportions { message }) => {
                assert!(message.contains("invalid"));
            }
            other => panic!("Expected InvalidProportions, got {:?}", other),
        }
    }
}

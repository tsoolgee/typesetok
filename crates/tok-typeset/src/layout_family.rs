//! Layout Family Classification Models.
//!
//! Enumerates and describes target layout topologies supported by the typesetting engine.

use serde::{Deserialize, Serialize};
use tok_core::FlowId;

/// Target layout topology family synthesized for a document section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutFamily {
    /// Single continuous column (prose, running text, standard novels).
    SingleFlow {
        flow_id: FlowId,
    },
    /// Parallel columns of equal or custom proportions (bilingual, 2-column, Mikraot Gedolot 4-column).
    ParallelColumns {
        column_count: usize,
        proportions: Option<Vec<f32>>,
        flow_ids: Vec<FlowId>,
    },
    /// Classical Tzurat HaDaf (Talmud): Central primary column with spine-relative commentary
    /// columns and dynamic L-shaped expansion when primary text ends early.
    TzuratHaDaf {
        primary_flow: FlowId,
        spine_inner_flow: Option<FlowId>,
        spine_outer_flow: Option<FlowId>,
        expansion_flow: Option<FlowId>,
        has_bottom_band: bool,
    },
    /// Text stream(s) with dedicated floating bottom footnote band.
    FootnotesBand {
        primary_flow: FlowId,
        footnote_flow: FlowId,
        column_flows: Vec<FlowId>,
    },
    /// Fully custom user-defined geometric constraints.
    CustomConstraints {
        flow_ids: Vec<FlowId>,
    },
}

impl LayoutFamily {
    /// Canonical ASCII identifier for this layout family.
    pub fn id_name(&self) -> &'static str {
        match self {
            Self::SingleFlow { .. } => "single_flow",
            Self::ParallelColumns { .. } => "parallel_columns",
            Self::TzuratHaDaf { .. } => "tzurat_hadaf",
            Self::FootnotesBand { .. } => "footnotes_band",
            Self::CustomConstraints { .. } => "custom_constraints",
        }
    }

    /// Human-readable Hebrew title for UI and logging.
    pub fn display_name_he(&self) -> &'static str {
        match self {
            Self::SingleFlow { .. } => "זרם יחיד (פרוזה וספר רציף)",
            Self::ParallelColumns { column_count, .. } => {
                match column_count {
                    2 => "שני טורים מקבילים",
                    3 => "שלושה טורים מקבילים",
                    4 => "מקראות גדולות (ארבעה טורים)",
                    _ => "טורים מקבילים",
                }
            }
            Self::TzuratHaDaf { .. } => "צורת הדף (ש\"ס עם הרחבת פירוש)",
            Self::FootnotesBand { .. } => "רצועת הערות שוליים תחתונה",
            Self::CustomConstraints { .. } => "פריסת אילוצים מותאמת אישית",
        }
    }

    /// Whether this layout coordinates multiple text flows on a single page spread.
    pub fn is_multi_flow(&self) -> bool {
        !matches!(self, Self::SingleFlow { .. })
    }

    /// Whether this layout supports dynamic L-shaped commentary expansion.
    pub fn has_l_shape(&self) -> bool {
        matches!(self, Self::TzuratHaDaf { .. })
    }

    /// Whether this layout includes a dedicated bottom footnote band.
    pub fn has_footnotes(&self) -> bool {
        matches!(self, Self::FootnotesBand { .. })
            || matches!(self, Self::TzuratHaDaf { has_bottom_band: true, .. })
    }

    /// Attempts to parse an explicit user layout hint string into a [`LayoutFamily`],
    /// populating available flow identifiers where possible.
    pub fn from_explicit_hint(hint: &str, flows: &[FlowId]) -> Option<Self> {
        let clean = hint.trim().to_lowercase();
        let first_flow = flows.first().cloned().unwrap_or_else(FlowId::main);

        match clean.as_str() {
            "single" | "prose" | "single_flow" | "running_text" => {
                Some(Self::SingleFlow { flow_id: first_flow })
            }
            "parallel" | "columns" | "multi_column" | "two_column" => {
                let count = if flows.len() >= 2 { flows.len() } else { 2 };
                Some(Self::ParallelColumns {
                    column_count: count,
                    proportions: None,
                    flow_ids: flows.to_vec(),
                })
            }
            "mikraot_gedolot" | "mikraot" => {
                Some(Self::ParallelColumns {
                    column_count: flows.len().max(4),
                    proportions: None,
                    flow_ids: flows.to_vec(),
                })
            }
            "tzurat_hadaf" | "talmud" | "gemara" | "l_shape" => {
                let primary = flows.first().cloned().unwrap_or_else(|| FlowId::new("gemara"));
                let inner = flows.get(1).cloned();
                let outer = flows.get(2).cloned();
                Some(Self::TzuratHaDaf {
                    primary_flow: primary,
                    spine_inner_flow: inner,
                    spine_outer_flow: outer.clone(),
                    expansion_flow: outer,
                    has_bottom_band: flows.iter().any(|f| f.0.contains("note")),
                })
            }
            "footnotes" | "footnote_band" => {
                let footnote_flow = flows
                    .iter()
                    .find(|f| f.0.contains("note") || f.0.contains("footnote"))
                    .cloned()
                    .unwrap_or_else(|| FlowId::new("notes"));
                let column_flows: Vec<FlowId> = flows
                    .iter()
                    .filter(|f| *f != &footnote_flow)
                    .cloned()
                    .collect();
                Some(Self::FootnotesBand {
                    primary_flow: first_flow,
                    footnote_flow,
                    column_flows,
                })
            }
            "custom" | "custom_constraints" => {
                Some(Self::CustomConstraints {
                    flow_ids: flows.to_vec(),
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_family_id_and_names() {
        let f1 = LayoutFamily::SingleFlow {
            flow_id: FlowId::main(),
        };
        assert_eq!(f1.id_name(), "single_flow");
        assert!(!f1.is_multi_flow());
        assert!(!f1.has_l_shape());

        let f2 = LayoutFamily::TzuratHaDaf {
            primary_flow: FlowId::new("gemara"),
            spine_inner_flow: Some(FlowId::new("rashi")),
            spine_outer_flow: Some(FlowId::new("tosafot")),
            expansion_flow: Some(FlowId::new("tosafot")),
            has_bottom_band: false,
        };
        assert_eq!(f2.id_name(), "tzurat_hadaf");
        assert!(f2.is_multi_flow());
        assert!(f2.has_l_shape());
    }

    #[test]
    fn test_from_explicit_hint() {
        let flows = vec![FlowId::new("torah"), FlowId::new("onkelos"), FlowId::new("rashi")];
        let fam = LayoutFamily::from_explicit_hint("talmud", &flows);
        assert!(fam.is_some());
        if let Some(LayoutFamily::TzuratHaDaf { primary_flow, .. }) = fam {
            assert_eq!(primary_flow.0, "torah");
        } else {
            panic!("Expected TzuratHaDaf");
        }
    }
}

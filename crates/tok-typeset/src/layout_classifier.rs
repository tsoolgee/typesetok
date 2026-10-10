//! Deterministic Document Layout Classifier.
//!
//! Analyzes document structural features and constraints to classify the appropriate
//! layout topology family, prioritizing explicit declarations before structural heuristics.

use crate::layout_family::LayoutFamily;
use crate::layout_features::DocumentLayoutFeatures;
use tok_core::FlowId;

/// Confidence level of the classification decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassificationConfidence {
    /// Specified explicitly by the user or document metadata.
    Explicit,
    /// Determined with high certainty from unambiguous structural indicators.
    StrongStructural,
    /// Inferred using deterministic disambiguation rules.
    Disambiguated,
    /// Default fallback when structural indicators are incomplete.
    Fallback,
}

/// Output of the layout classification process.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassificationResult {
    pub family: LayoutFamily,
    pub confidence: ClassificationConfidence,
    pub reasoning: String,
    pub warnings: Vec<String>,
}

pub struct DocumentClassifier;

impl DocumentClassifier {
    /// Classifies a document section into the most appropriate [`LayoutFamily`].
    pub fn classify(features: &DocumentLayoutFeatures) -> ClassificationResult {
        let mut warnings = Vec::new();
        let all_flow_ids: Vec<FlowId> = features.flows.iter().map(|f| f.id.clone()).collect();
        let first_flow_id = all_flow_ids.first().cloned().unwrap_or_else(FlowId::main);

        // 1. Explicit layout hint takes highest precedence
        if let Some(ref hint) = features.explicit_layout_hint {
            if let Some(explicit_family) = LayoutFamily::from_explicit_hint(hint, &all_flow_ids) {
                // Validate compatibility with existing document structure
                match &explicit_family {
                    LayoutFamily::SingleFlow { .. } if features.active_flow_count() > 1 => {
                        warnings.push(format!(
                            "Explicit single_flow layout requested, but section defines {} active flows; secondary flows will not receive dedicated columns",
                            features.active_flow_count()
                        ));
                    }
                    LayoutFamily::TzuratHaDaf { .. } if features.total_flow_count() < 2 => {
                        warnings.push(
                            "Explicit tzurat_hadaf layout requested, but section contains fewer than 2 flows; will synthesize default commentary columns".to_string()
                        );
                    }
                    _ => {}
                }

                return ClassificationResult {
                    family: explicit_family,
                    confidence: ClassificationConfidence::Explicit,
                    reasoning: format!("Explicit layout hint '{}' honored", hint),
                    warnings,
                };
            } else {
                warnings.push(format!(
                    "Unrecognized explicit layout hint '{}'; falling back to structural classification",
                    hint
                ));
            }
        }

        // 2. Single-flow documents (prose, continuous reading)
        if features.total_flow_count() <= 1
            || (features.active_flow_count() <= 1
                && !features.has_spine_relative_commentaries()
                && !features.has_explicit_width_constraints()
                && features.footnote_flow().is_none())
        {
            return ClassificationResult {
                family: LayoutFamily::SingleFlow { flow_id: first_flow_id },
                confidence: ClassificationConfidence::StrongStructural,
                reasoning: "Single active text flow detected; continuous single-column layout selected".to_string(),
                warnings,
            };
        }

        // 3. Footnote band layout detection
        let footnote_flow = features.footnote_flow();
        let non_footnote_flows: Vec<FlowId> = features
            .flows
            .iter()
            .filter(|f| !f.is_footnote)
            .map(|f| f.id.clone())
            .collect();

        if let Some(fn_feat) = footnote_flow {
            if non_footnote_flows.len() == 1 {
                return ClassificationResult {
                    family: LayoutFamily::FootnotesBand {
                        primary_flow: non_footnote_flows[0].clone(),
                        footnote_flow: fn_feat.id.clone(),
                        column_flows: non_footnote_flows,
                    },
                    confidence: ClassificationConfidence::StrongStructural,
                    reasoning: "Primary text stream with dedicated bottom footnote band detected".to_string(),
                    warnings,
                };
            } else if features.has_spine_relative_commentaries() {
                // Tzurat HaDaf with footnotes band
                let primary = features.primary_flow().map(|f| f.id.clone()).unwrap_or_else(|| non_footnote_flows[0].clone());
                let inner = features.spine_inner_flow().map(|f| f.id.clone());
                let outer = features.spine_outer_flow().map(|f| f.id.clone());
                let expansion = features.explicit_expansion_flow_id.clone().or_else(|| outer.clone());

                return ClassificationResult {
                    family: LayoutFamily::TzuratHaDaf {
                        primary_flow: primary,
                        spine_inner_flow: inner,
                        spine_outer_flow: outer,
                        expansion_flow: expansion,
                        has_bottom_band: true,
                    },
                    confidence: ClassificationConfidence::StrongStructural,
                    reasoning: "Spine-relative commentary structure with bottom footnote band detected".to_string(),
                    warnings,
                };
            }
        }

        // 4. Tzurat HaDaf (Talmud / Classical Jewish layout with central primary and spine commentaries)
        if features.has_spine_relative_commentaries()
            || (features.total_flow_count() == 3 && features.primary_flow().is_some())
        {
            let primary = features
                .primary_flow()
                .map(|f| f.id.clone())
                .unwrap_or_else(|| all_flow_ids[0].clone());

            let inner = features
                .spine_inner_flow()
                .map(|f| f.id.clone())
                .or_else(|| all_flow_ids.get(1).cloned());

            let outer = features
                .spine_outer_flow()
                .map(|f| f.id.clone())
                .or_else(|| all_flow_ids.get(2).cloned());

            let expansion = features
                .explicit_expansion_flow_id
                .clone()
                .or_else(|| outer.clone())
                .or_else(|| inner.clone());

            let has_bottom = footnote_flow.is_some();

            return ClassificationResult {
                family: LayoutFamily::TzuratHaDaf {
                    primary_flow: primary,
                    spine_inner_flow: inner,
                    spine_outer_flow: outer,
                    expansion_flow: expansion,
                    has_bottom_band: has_bottom,
                },
                confidence: if features.has_spine_relative_commentaries() {
                    ClassificationConfidence::StrongStructural
                } else {
                    ClassificationConfidence::Disambiguated
                },
                reasoning: "Classical Tzurat HaDaf structure with dynamic commentary expansion identified".to_string(),
                warnings,
            };
        }

        // 5. Parallel Multi-Column Layout (2 columns, 4 columns / Mikraot Gedolot, or N columns)
        let col_count = features.column_flow_count();
        if col_count >= 2 {
            return ClassificationResult {
                family: LayoutFamily::ParallelColumns {
                    column_count: col_count,
                    proportions: features.explicit_column_proportions.clone(),
                    flow_ids: non_footnote_flows,
                },
                confidence: ClassificationConfidence::StrongStructural,
                reasoning: format!("Parallel multi-column layout with {} streams detected", col_count),
                warnings,
            };
        }

        // 6. Fallback
        ClassificationResult {
            family: LayoutFamily::CustomConstraints {
                flow_ids: all_flow_ids,
            },
            confidence: ClassificationConfidence::Fallback,
            reasoning: "General multi-stream layout with custom constraints applied as fallback".to_string(),
            warnings,
        }
    }
}

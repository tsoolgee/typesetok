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

#[cfg(test)]
mod tests {
    use super::*;
    use tok_core::id::FractionalIndex;
    use tok_core::model::{Flow, FlowType, ParagraphNode, SectionNode};

    fn make_flow_with_text(id: &str, flow_type: FlowType, text: &str) -> Flow {
        let mut f = Flow::new(FlowId::new(id), flow_type);
        f.paragraphs.push(ParagraphNode::new(
            FractionalIndex::initial(),
            "normal",
            text,
        ));
        f
    }

    #[test]
    fn test_classify_single_flow_prose() {
        let mut sec = SectionNode::new("מבוא", "default");
        sec.flows.clear();
        sec.flows.push(make_flow_with_text("main", FlowType::Main, "טקסט רציף"));

        let feat = DocumentLayoutFeatures::from_section(&sec);
        let res = DocumentClassifier::classify(&feat);

        assert_eq!(res.confidence, ClassificationConfidence::StrongStructural);
        match res.family {
            LayoutFamily::SingleFlow { flow_id } => assert_eq!(flow_id.0, "main"),
            _ => panic!("Expected SingleFlow, got {:?}", res.family),
        }
    }

    #[test]
    fn test_classify_parallel_two_columns_unfamiliar_names() {
        let mut sec = SectionNode::new("תרגום מקביל", "default");
        sec.flows.clear();
        sec.flows.push(make_flow_with_text("source_alpha", FlowType::Main, "טקסט מקור"));
        sec.flows.push(make_flow_with_text("target_beta", FlowType::Main, "תרגום"));

        let feat = DocumentLayoutFeatures::from_section(&sec);
        let res = DocumentClassifier::classify(&feat);

        assert_eq!(res.confidence, ClassificationConfidence::StrongStructural);
        match res.family {
            LayoutFamily::ParallelColumns { column_count, flow_ids, .. } => {
                assert_eq!(column_count, 2);
                assert_eq!(flow_ids.len(), 2);
                assert_eq!(flow_ids[0].0, "source_alpha");
                assert_eq!(flow_ids[1].0, "target_beta");
            }
            _ => panic!("Expected ParallelColumns, got {:?}", res.family),
        }
    }

    #[test]
    fn test_classify_mikraot_gedolot_four_columns() {
        let mut sec = SectionNode::new("מקראות", "default");
        sec.flows.clear();
        sec.flows.push(make_flow_with_text("torah", FlowType::Main, "חומש"));
        sec.flows.push(make_flow_with_text("targum", FlowType::Main, "תרגום"));
        sec.flows.push(make_flow_with_text("comm_a", FlowType::Main, "פירוש א"));
        sec.flows.push(make_flow_with_text("comm_b", FlowType::Main, "פירוש ב"));

        let feat = DocumentLayoutFeatures::from_section(&sec);
        let res = DocumentClassifier::classify(&feat);

        assert_eq!(res.confidence, ClassificationConfidence::StrongStructural);
        match res.family {
            LayoutFamily::ParallelColumns { column_count, flow_ids, .. } => {
                assert_eq!(column_count, 4);
                assert_eq!(flow_ids.len(), 4);
            }
            _ => panic!("Expected ParallelColumns 4, got {:?}", res.family),
        }
    }

    #[test]
    fn test_classify_tzurat_hadaf_with_unfamiliar_flow_names() {
        let mut sec = SectionNode::new("דף יומי", "default");
        sec.flows.clear();

        let mut primary = make_flow_with_text("central_corpus", FlowType::Main, "טקסט מרכזי");
        primary.placement_role = Some("primary".to_string());
        sec.flows.push(primary);

        let mut inner = make_flow_with_text("spine_commentary", FlowType::CommentA, "פירוש שדרה פנימית");
        inner.placement_role = Some("inner_spine".to_string());
        sec.flows.push(inner);

        let mut outer = make_flow_with_text("margin_gloss", FlowType::CommentB, "הערת שוליים חיצונית");
        outer.placement_role = Some("outer_margin".to_string());
        sec.flows.push(outer);

        let feat = DocumentLayoutFeatures::from_section(&sec);
        let res = DocumentClassifier::classify(&feat);

        assert_eq!(res.confidence, ClassificationConfidence::StrongStructural);
        match res.family {
            LayoutFamily::TzuratHaDaf {
                primary_flow,
                spine_inner_flow,
                spine_outer_flow,
                expansion_flow,
                has_bottom_band,
            } => {
                assert_eq!(primary_flow.0, "central_corpus");
                assert_eq!(spine_inner_flow.unwrap().0, "spine_commentary");
                assert_eq!(spine_outer_flow.unwrap().0, "margin_gloss");
                assert_eq!(expansion_flow.unwrap().0, "margin_gloss");
                assert!(!has_bottom_band);
            }
            _ => panic!("Expected TzuratHaDaf, got {:?}", res.family),
        }
    }

    #[test]
    fn test_classify_footnote_band() {
        let mut sec = SectionNode::new("ספר מחקר", "default");
        sec.flows.clear();
        sec.flows.push(make_flow_with_text("body", FlowType::Main, "גוף המאמר"));
        sec.flows.push(make_flow_with_text("critical_apparatus", FlowType::Footnote, "הערות"));

        let feat = DocumentLayoutFeatures::from_section(&sec);
        let res = DocumentClassifier::classify(&feat);

        assert_eq!(res.confidence, ClassificationConfidence::StrongStructural);
        match res.family {
            LayoutFamily::FootnotesBand {
                primary_flow,
                footnote_flow,
                column_flows,
            } => {
                assert_eq!(primary_flow.0, "body");
                assert_eq!(footnote_flow.0, "critical_apparatus");
                assert_eq!(column_flows.len(), 1);
            }
            _ => panic!("Expected FootnotesBand, got {:?}", res.family),
        }
    }

    #[test]
    fn test_classify_tzurat_hadaf_with_footnote_band() {
        let mut sec = SectionNode::new("תלמוד עם מסורת", "default");
        sec.flows.clear();

        let mut gemara = make_flow_with_text("gemara", FlowType::Main, "גמרא");
        gemara.placement_role = Some("primary".to_string());
        sec.flows.push(gemara);

        let mut rashi = make_flow_with_text("rashi", FlowType::CommentA, "רש״י");
        rashi.placement_role = Some("inner_spine".to_string());
        sec.flows.push(rashi);

        let mut tosafot = make_flow_with_text("tosafot", FlowType::CommentB, "תוספות");
        tosafot.placement_role = Some("outer_margin".to_string());
        sec.flows.push(tosafot);

        let notes = make_flow_with_text("masoret", FlowType::Footnote, "הערות הש״ס");
        sec.flows.push(notes);

        let feat = DocumentLayoutFeatures::from_section(&sec);
        let res = DocumentClassifier::classify(&feat);

        assert_eq!(res.confidence, ClassificationConfidence::StrongStructural);
        match res.family {
            LayoutFamily::TzuratHaDaf { has_bottom_band, .. } => {
                assert!(has_bottom_band);
            }
            _ => panic!("Expected TzuratHaDaf with footnotes, got {:?}", res.family),
        }
    }

    #[test]
    fn test_explicit_hint_overrides_structural_detection() {
        let mut sec = SectionNode::new("כפיית פרוזה", "default");
        sec.layout_kind = Some("prose".to_string());
        sec.flows.clear();
        sec.flows.push(make_flow_with_text("flow1", FlowType::Main, "זרם 1"));
        sec.flows.push(make_flow_with_text("flow2", FlowType::CommentA, "זרם 2"));

        let feat = DocumentLayoutFeatures::from_section(&sec);
        let res = DocumentClassifier::classify(&feat);

        assert_eq!(res.confidence, ClassificationConfidence::Explicit);
        match res.family {
            LayoutFamily::SingleFlow { flow_id } => assert_eq!(flow_id.0, "flow1"),
            _ => panic!("Expected explicit SingleFlow override, got {:?}", res.family),
        }
        assert!(!res.warnings.is_empty(), "Expected warning about multi-flow mismatch");
    }

    #[test]
    fn test_unrecognized_explicit_hint_falls_back_to_structural() {
        let mut sec = SectionNode::new("רמז לא מוכר", "default");
        sec.layout_kind = Some("unknown_magic_layout".to_string());
        sec.flows.clear();
        sec.flows.push(make_flow_with_text("col_1", FlowType::Main, "טור 1"));
        sec.flows.push(make_flow_with_text("col_2", FlowType::Main, "טור 2"));

        let feat = DocumentLayoutFeatures::from_section(&sec);
        let res = DocumentClassifier::classify(&feat);

        assert_eq!(res.confidence, ClassificationConfidence::StrongStructural);
        match res.family {
            LayoutFamily::ParallelColumns { column_count, .. } => assert_eq!(column_count, 2),
            _ => panic!("Expected fallback to ParallelColumns, got {:?}", res.family),
        }
        assert!(res.warnings.iter().any(|w| w.contains("Unrecognized explicit layout hint")));
    }
}


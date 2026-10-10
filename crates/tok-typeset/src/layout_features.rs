//! Document and Stream Feature Model for Layout Classification.
//!
//! Extracts structural and content characteristics from document sections and flows
//! to inform deterministic layout classification and template synthesis.

use crate::multi_flow::FlowPlacementRole;
use tok_core::model::{DocumentRoot, Flow, FlowType, SectionNode};
use tok_core::FlowId;

/// Extracted structural features of an individual text flow.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowFeature {
    pub id: FlowId,
    pub flow_type: FlowType,
    pub placement_role: Option<FlowPlacementRole>,
    pub width_ratio: Option<f32>,
    pub paragraph_count: usize,
    pub char_count: usize,
    pub is_empty: bool,
    pub is_primary_candidate: bool,
    pub is_footnote: bool,
    pub is_spine_inner: bool,
    pub is_spine_outer: bool,
}

impl FlowFeature {
    /// Extracts layout features from a document [`Flow`].
    pub fn from_flow(flow: &Flow, index: usize) -> Self {
        let paragraph_count = flow.paragraphs.len();
        let char_count: usize = flow.paragraphs.iter().map(|p| p.text.chars().count()).sum();
        let is_empty = paragraph_count == 0;

        let placement_role =
            flow.placement_role
                .as_ref()
                .map(|r| match r.trim().to_lowercase().as_str() {
                    "primary" | "main" => FlowPlacementRole::Primary,
                    "inner_spine" | "innerspine" | "inner" => FlowPlacementRole::InnerSpine,
                    "outer_margin" | "outermargin" | "outer" => FlowPlacementRole::OuterMargin,
                    "bottom_band" | "bottomband" | "footnote" => FlowPlacementRole::BottomBand,
                    _ => FlowPlacementRole::Column(index),
                });

        let is_footnote = matches!(placement_role, Some(FlowPlacementRole::BottomBand))
            || flow.flow_type == FlowType::Footnote
            || flow.id.0.eq_ignore_ascii_case("notes")
            || flow.id.0.eq_ignore_ascii_case("heorot")
            || flow.id.0.contains("footnote");

        let is_spine_inner = matches!(placement_role, Some(FlowPlacementRole::InnerSpine))
            || flow.flow_type == FlowType::CommentA
            || flow.id.0.eq_ignore_ascii_case("rashi");

        let is_spine_outer = matches!(placement_role, Some(FlowPlacementRole::OuterMargin))
            || flow.flow_type == FlowType::CommentB
            || flow.id.0.eq_ignore_ascii_case("tosafot");

        let is_primary_candidate = matches!(placement_role, Some(FlowPlacementRole::Primary))
            || flow.flow_type == FlowType::Main
            || flow.id.0 == "main"
            || flow.id.0 == "gemara"
            || flow.id.0 == "torah"
            || (!is_footnote && !is_spine_inner && !is_spine_outer && index == 0);

        Self {
            id: flow.id.clone(),
            flow_type: flow.flow_type,
            placement_role,
            width_ratio: flow.width_ratio,
            paragraph_count,
            char_count,
            is_empty,
            is_primary_candidate,
            is_footnote,
            is_spine_inner,
            is_spine_outer,
        }
    }
}

/// Structural and constraint features extracted from a document section.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentLayoutFeatures {
    pub section_name: String,
    pub page_style: String,
    pub explicit_layout_hint: Option<String>,
    pub explicit_column_proportions: Option<Vec<f32>>,
    pub explicit_expansion_flow_id: Option<FlowId>,
    pub flows: Vec<FlowFeature>,
}

impl DocumentLayoutFeatures {
    /// Extracts features from a [`SectionNode`].
    pub fn from_section(section: &SectionNode) -> Self {
        let flows: Vec<FlowFeature> = section
            .flows
            .iter()
            .enumerate()
            .map(|(idx, f)| FlowFeature::from_flow(f, idx))
            .collect();

        Self {
            section_name: section.name.clone(),
            page_style: section.page_style.clone(),
            explicit_layout_hint: section.layout_kind.clone(),
            explicit_column_proportions: section.column_proportions.clone(),
            explicit_expansion_flow_id: section.expansion_flow_id.clone(),
            flows,
        }
    }

    /// Extracts features from the specified section in a [`DocumentRoot`].
    pub fn from_document(doc: &DocumentRoot, section_index: usize) -> Option<Self> {
        doc.sections.get(section_index).map(Self::from_section)
    }

    /// Total number of defined flows in the section.
    pub fn total_flow_count(&self) -> usize {
        self.flows.len()
    }

    /// Number of flows that contain at least one paragraph.
    pub fn active_flow_count(&self) -> usize {
        self.flows.iter().filter(|f| !f.is_empty).count()
    }

    /// Number of flows that are not bottom footnote bands.
    pub fn column_flow_count(&self) -> usize {
        self.flows.iter().filter(|f| !f.is_footnote).count()
    }

    /// Primary flow candidate, if identified.
    pub fn primary_flow(&self) -> Option<&FlowFeature> {
        self.flows
            .iter()
            .find(|f| f.is_primary_candidate)
            .or_else(|| self.flows.iter().find(|f| !f.is_footnote))
    }

    /// Footnote flow, if present.
    pub fn footnote_flow(&self) -> Option<&FlowFeature> {
        self.flows.iter().find(|f| f.is_footnote)
    }

    /// Inner spine commentary flow, if present.
    pub fn spine_inner_flow(&self) -> Option<&FlowFeature> {
        self.flows.iter().find(|f| f.is_spine_inner)
    }

    /// Outer margin commentary flow, if present.
    pub fn spine_outer_flow(&self) -> Option<&FlowFeature> {
        self.flows.iter().find(|f| f.is_spine_outer)
    }

    /// Whether any flow specifies explicit width constraints.
    pub fn has_explicit_width_constraints(&self) -> bool {
        self.explicit_column_proportions.is_some()
            || self.flows.iter().any(|f| f.width_ratio.is_some())
    }

    /// Whether any spine-relative placement role is explicitly or semantically present.
    pub fn has_spine_relative_commentaries(&self) -> bool {
        self.spine_inner_flow().is_some() || self.spine_outer_flow().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tok_core::id::FractionalIndex;
    use tok_core::model::ParagraphNode;

    #[test]
    fn test_flow_features_extraction() {
        let mut f = Flow::new(FlowId::new("commentary_stream"), FlowType::CommentA);
        f.width_ratio = Some(0.35);
        f.placement_role = Some("inner_spine".to_string());
        f.paragraphs.push(ParagraphNode::new(
            FractionalIndex::initial(),
            "normal",
            "שלום עולם של בדיקות",
        ));

        let feat = FlowFeature::from_flow(&f, 1);
        assert_eq!(feat.id.0, "commentary_stream");
        assert_eq!(feat.placement_role, Some(FlowPlacementRole::InnerSpine));
        assert_eq!(feat.width_ratio, Some(0.35));
        assert_eq!(feat.paragraph_count, 1);
        assert!(!feat.is_empty);
        assert!(feat.is_spine_inner);
        assert!(!feat.is_footnote);
    }

    #[test]
    fn test_document_layout_features_extraction() {
        let mut sec = SectionNode::new("שער ראשון", "default");
        sec.layout_kind = Some("tzurat_hadaf".to_string());
        sec.flows.clear();

        let mut main_f = Flow::new(FlowId::new("text_core"), FlowType::Main);
        main_f.paragraphs.push(ParagraphNode::new(
            FractionalIndex::initial(),
            "normal",
            "טקסט עיקרי",
        ));
        sec.flows.push(main_f);

        let mut comm_f = Flow::new(FlowId::new("scholia"), FlowType::CommentA);
        comm_f.placement_role = Some("outer_margin".to_string());
        sec.flows.push(comm_f);

        let notes_f = Flow::new(FlowId::new("apparatus"), FlowType::Footnote);
        sec.flows.push(notes_f);

        let doc_features = DocumentLayoutFeatures::from_section(&sec);
        assert_eq!(doc_features.section_name, "שער ראשון");
        assert_eq!(
            doc_features.explicit_layout_hint.as_deref(),
            Some("tzurat_hadaf")
        );
        assert_eq!(doc_features.total_flow_count(), 3);
        assert_eq!(doc_features.active_flow_count(), 1);
        assert_eq!(doc_features.column_flow_count(), 2);
        assert!(doc_features.footnote_flow().is_some());
        assert_eq!(doc_features.footnote_flow().unwrap().id.0, "apparatus");
        assert!(doc_features.primary_flow().is_some());
        assert_eq!(doc_features.primary_flow().unwrap().id.0, "text_core");
    }
}

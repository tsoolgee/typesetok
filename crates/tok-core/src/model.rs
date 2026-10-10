use crate::error::ModelError;
use crate::id::{FractionalIndex, NodeId};
use crate::normalizer::HebrewNormalizer;
use crate::styles::{CharacterStyle, ParagraphStyle, Progression, StylePatch};
use ropey::Rope;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FlowId(pub String);

impl FlowId {
    pub fn main() -> Self {
        Self("main".to_string())
    }

    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowType {
    Main,
    CommentA,
    CommentB,
    Footnote,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParagraphNode {
    pub id: NodeId,
    pub index: FractionalIndex,
    pub style_id: String,
    pub text: String,
    #[serde(default)]
    pub style_patches: Vec<StylePatch>,
}

impl ParagraphNode {
    pub fn new(index: FractionalIndex, style_id: impl Into<String>, text: &str) -> Self {
        let normalized = HebrewNormalizer::normalize(text);
        Self {
            id: NodeId::new(),
            index,
            style_id: style_id.into(),
            text: normalized,
            style_patches: Vec::new(),
        }
    }

    pub fn rope(&self) -> Rope {
        Rope::from_str(&self.text)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Flow {
    pub id: FlowId,
    pub flow_type: FlowType,
    pub paragraphs: Vec<ParagraphNode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_ratio: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement_role: Option<String>,
}

impl Flow {
    pub fn new(id: FlowId, flow_type: FlowType) -> Self {
        Self {
            id,
            flow_type,
            paragraphs: Vec::new(),
            width_ratio: None,
            placement_role: None,
        }
    }

    pub fn with_width_ratio(mut self, ratio: f32) -> Self {
        self.width_ratio = Some(ratio);
        self
    }

    pub fn with_placement_role(mut self, role: impl Into<String>) -> Self {
        self.placement_role = Some(role.into());
        self
    }

    pub fn add_paragraph(&mut self, p: ParagraphNode) {
        if self.paragraphs.is_sorted_by(|a, b| a.index <= b.index) {
            // O(n) insertion after any paragraphs with an equal index
            // (same placement as push + stable sort).
            let pos = self.paragraphs.partition_point(|q| q.index <= p.index);
            self.paragraphs.insert(pos, p);
        } else {
            self.paragraphs.push(p);
            self.paragraphs.sort_by(|a, b| a.index.cmp(&b.index));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionNode {
    pub id: NodeId,
    pub name: String,
    pub page_style: String,
    pub flows: Vec<Flow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_proportions: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expansion_flow_id: Option<FlowId>,
}

impl SectionNode {
    pub fn new(name: impl Into<String>, page_style: impl Into<String>) -> Self {
        let main_flow = Flow::new(FlowId::main(), FlowType::Main);
        Self {
            id: NodeId::new(),
            name: name.into(),
            page_style: page_style.into(),
            flows: vec![main_flow],
            layout_kind: None,
            column_proportions: None,
            expansion_flow_id: None,
        }
    }

    pub fn with_layout_kind(mut self, kind: impl Into<String>) -> Self {
        self.layout_kind = Some(kind.into());
        self
    }

    pub fn with_column_proportions(mut self, proportions: Vec<f32>) -> Self {
        self.column_proportions = Some(proportions);
        self
    }

    pub fn with_expansion_flow_id(mut self, id: FlowId) -> Self {
        self.expansion_flow_id = Some(id);
        self
    }

    pub fn main_flow_mut(&mut self) -> Option<&mut Flow> {
        self.flows
            .iter_mut()
            .find(|f| f.id == FlowId::main() || f.id.0 == "gemara" || f.flow_type == FlowType::Main)
    }

    pub fn main_flow(&self) -> Option<&Flow> {
        self.flows
            .iter()
            .find(|f| f.id == FlowId::main() || f.id.0 == "gemara" || f.flow_type == FlowType::Main)
    }

    pub fn with_flows(mut self, flows: Vec<Flow>) -> Self {
        self.flows = flows;
        self
    }

    pub fn with_flow(mut self, flow: Flow) -> Self {
        self.flows.push(flow);
        self
    }

    pub fn find_flow(&self, id: &str) -> Option<&Flow> {
        self.flows.iter().find(|f| f.id.0 == id)
    }

    pub fn find_flow_mut(&mut self, id: &str) -> Option<&mut Flow> {
        self.flows.iter_mut().find(|f| f.id.0 == id)
    }

    pub fn has_flow(&self, id: &str) -> bool {
        self.flows.iter().any(|f| f.id.0 == id)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub title: String,
    pub author: String,
    pub progression: Progression,
    pub primary_language: String,
    pub schema_version: String,
}

impl Default for DocumentMetadata {
    fn default() -> Self {
        Self {
            title: "מסמך חדש".to_string(),
            author: "מחבר".to_string(),
            progression: Progression::Rtl,
            primary_language: "he".to_string(),
            schema_version: "1.0".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentRoot {
    pub id: NodeId,
    pub metadata: DocumentMetadata,
    pub paragraph_styles: Vec<ParagraphStyle>,
    pub character_styles: Vec<CharacterStyle>,
    pub sections: Vec<SectionNode>,
}

impl DocumentRoot {
    pub fn new(title: impl Into<String>) -> Self {
        let mut root = Self {
            id: NodeId::new(),
            metadata: DocumentMetadata {
                title: title.into(),
                ..Default::default()
            },
            paragraph_styles: vec![ParagraphStyle::default()],
            character_styles: Vec::new(),
            sections: Vec::new(),
        };

        // Add a default first section
        let default_section = SectionNode::new("שער ראשון", "chapter-first");
        root.sections.push(default_section);
        root
    }

    pub fn find_paragraph(&self, id: NodeId) -> Option<&ParagraphNode> {
        for sec in &self.sections {
            for flow in &sec.flows {
                for p in &flow.paragraphs {
                    if p.id == id {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    pub fn find_paragraph_mut(&mut self, id: NodeId) -> Option<&mut ParagraphNode> {
        for sec in &mut self.sections {
            for flow in &mut sec.flows {
                for p in &mut flow.paragraphs {
                    if p.id == id {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    pub fn to_json(&self) -> Result<String, ModelError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| ModelError::SerializationError(e.to_string()))
    }

    pub fn from_json(json_str: &str) -> Result<Self, ModelError> {
        serde_json::from_str(json_str).map_err(|e| ModelError::SerializationError(e.to_string()))
    }
}

/// The document model wrapper that handles snapshots and concurrency.
#[derive(Debug, Clone)]
pub struct DocumentModel {
    root: Arc<DocumentRoot>,
}

impl DocumentModel {
    pub fn new(root: DocumentRoot) -> Self {
        Self {
            root: Arc::new(root),
        }
    }

    pub fn snapshot(&self) -> Arc<DocumentRoot> {
        Arc::clone(&self.root)
    }

    pub fn update_root(&mut self, new_root: DocumentRoot) {
        self.root = Arc::new(new_root);
    }

    pub fn root(&self) -> &DocumentRoot {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::FractionalIndex;

    #[test]
    fn test_add_paragraph_maintains_order() {
        let mut flow = Flow::new(FlowId::main(), FlowType::Main);
        let p3 = ParagraphNode::new(FractionalIndex::new("p"), "normal", "third");
        let p1 = ParagraphNode::new(FractionalIndex::new("b"), "normal", "first");
        let p2 = ParagraphNode::new(FractionalIndex::new("m"), "normal", "second");

        // Add in shuffled order
        flow.add_paragraph(p3);
        flow.add_paragraph(p1);
        flow.add_paragraph(p2);

        // They should be sorted by index
        assert_eq!(flow.paragraphs[0].text, "first");
        assert_eq!(flow.paragraphs[1].text, "second");
        assert_eq!(flow.paragraphs[2].text, "third");
    }

    #[test]
    fn test_document_to_json_and_back() {
        let mut root = DocumentRoot::new("Test Doc");
        let sec = &mut root.sections[0];
        let flow = sec.main_flow_mut().unwrap();
        flow.add_paragraph(ParagraphNode::new(
            FractionalIndex::initial(),
            "normal",
            "Hello World",
        ));

        let json = root.to_json().expect("Serialization must succeed");
        let loaded = DocumentRoot::from_json(&json).expect("Deserialization must succeed");
        assert_eq!(loaded.metadata.title, "Test Doc");
        assert_eq!(loaded.sections[0].main_flow().unwrap().paragraphs.len(), 1);
    }

    #[test]
    fn test_multi_flow_document_serialization_roundtrip() {
        let mut root = DocumentRoot::new("דף גמרא - ברכות");
        root.metadata.progression = Progression::Rtl;
        root.metadata.primary_language = "he".to_string();

        let sec = &mut root.sections[0];
        sec.flows.clear();

        // Main Gemara flow
        let mut gemara_flow = Flow::new(FlowId::new("gemara"), FlowType::Main);
        gemara_flow.add_paragraph(ParagraphNode::new(
            FractionalIndex::new("a0"),
            "style-gemara-main",
            "מֵאֵימָתַי קוֹרִין אֶת שְׁמַע בְּעַרְבִית?",
        ));
        let gemara_expected_text = gemara_flow.paragraphs[0].text.clone();
        sec.flows.push(gemara_flow);

        // Rashi commentary flow
        let mut rashi_flow = Flow::new(FlowId::new("rashi"), FlowType::CommentA);
        rashi_flow.add_paragraph(ParagraphNode::new(
            FractionalIndex::new("a0"),
            "style-rashi-body",
            "תַּנָּא אַקְּרָא קָאֵי דִּכְתִיב בְּשָׁכְבְּךָ וּבְקוּמֶךָ",
        ));
        let rashi_expected_text = rashi_flow.paragraphs[0].text.clone();
        sec.flows.push(rashi_flow);

        // Tosafot commentary flow
        let mut tosafot_flow = Flow::new(FlowId::new("tosafot"), FlowType::CommentB);
        tosafot_flow.add_paragraph(ParagraphNode::new(
            FractionalIndex::new("a0"),
            "style-tosafot-body",
            "פֵּרֵשׁ רַשִׁ\"י דְּתַנָּא אַקְּרָא קָאֵי",
        ));
        sec.flows.push(tosafot_flow);

        // Notes flow
        let mut notes_flow = Flow::new(FlowId::new("notes"), FlowType::Footnote);
        notes_flow.add_paragraph(ParagraphNode::new(
            FractionalIndex::new("a0"),
            "style-footnotes",
            "תורה אור: דברים ו, ז.",
        ));
        sec.flows.push(notes_flow);

        let json = root.to_json().expect("Multi-flow serialization must succeed");
        let restored = DocumentRoot::from_json(&json).expect("Deserialization must succeed");

        assert_eq!(restored.metadata.title, "דף גמרא - ברכות");
        assert_eq!(restored.metadata.progression, Progression::Rtl);
        assert_eq!(restored.sections.len(), 1);
        assert_eq!(restored.sections[0].flows.len(), 4);

        assert_eq!(restored.sections[0].flows[0].id.0, "gemara");
        assert_eq!(
            restored.sections[0].flows[0].paragraphs[0].text,
            gemara_expected_text
        );

        assert_eq!(restored.sections[0].flows[1].id.0, "rashi");
        assert_eq!(
            restored.sections[0].flows[1].paragraphs[0].text,
            rashi_expected_text
        );
        assert_eq!(restored.sections[0].flows[2].id.0, "tosafot");
        assert_eq!(restored.sections[0].flows[3].id.0, "notes");

        // Verify main_flow() finds the gemara flow
        assert!(restored.sections[0].main_flow().is_some());
        assert_eq!(restored.sections[0].main_flow().unwrap().id.0, "gemara");
    }

    #[test]
    fn test_hebrew_niqqud_preservation_in_document() {
        let text_with_niqqud = "שָׁלוֹם עֲלֵיכֶם מַלְאֲכֵי הַשָּׁרֵת מַלְאֲכֵי עֶלְיוֹן";
        let p = ParagraphNode::new(FractionalIndex::initial(), "default-body", text_with_niqqud);
        assert_eq!(p.text, HebrewNormalizer::normalize(text_with_niqqud));

        let mut root = DocumentRoot::new("מנוקד");
        root.sections[0].main_flow_mut().unwrap().add_paragraph(p.clone());

        let json = root.to_json().unwrap();
        let loaded = DocumentRoot::from_json(&json).unwrap();
        let loaded_text = &loaded.sections[0].main_flow().unwrap().paragraphs[0].text;
        assert_eq!(loaded_text, &p.text);
    }

    #[test]
    fn test_empty_document_root() {
        let root = DocumentRoot::new("");
        let json = root.to_json().unwrap();
        let loaded = DocumentRoot::from_json(&json).unwrap();
        assert_eq!(loaded.metadata.title, "");
        assert_eq!(loaded.sections.len(), 1);
        assert_eq!(loaded.sections[0].flows[0].paragraphs.len(), 0);
    }

    #[test]
    fn test_flow_layout_constraints_serialization_roundtrip() {
        let mut root = DocumentRoot::new("תבנית רב-זרימתית");
        let sec = &mut root.sections[0];
        *sec = SectionNode::new("פרק ראשון", "talmud-page")
            .with_layout_kind("dynamic_talmud")
            .with_column_proportions(vec![0.40, 0.28, 0.32])
            .with_expansion_flow_id(FlowId::new("rashi"));

        let flow1 = Flow::new(FlowId::new("main"), FlowType::Main)
            .with_width_ratio(0.40)
            .with_placement_role("primary");
        let flow2 = Flow::new(FlowId::new("rashi"), FlowType::CommentA)
            .with_width_ratio(0.28)
            .with_placement_role("inner_spine");
        let flow3 = Flow::new(FlowId::new("tosafot"), FlowType::CommentB)
            .with_width_ratio(0.32)
            .with_placement_role("outer_margin");

        sec.flows = vec![flow1, flow2, flow3];

        let json = root.to_json().expect("Serialization must succeed");
        let loaded = DocumentRoot::from_json(&json).expect("Deserialization must succeed");

        let loaded_sec = &loaded.sections[0];
        assert_eq!(loaded_sec.layout_kind.as_deref(), Some("dynamic_talmud"));
        assert_eq!(loaded_sec.column_proportions, Some(vec![0.40, 0.28, 0.32]));
        assert_eq!(loaded_sec.expansion_flow_id, Some(FlowId::new("rashi")));

        assert_eq!(loaded_sec.flows[0].width_ratio, Some(0.40));
        assert_eq!(loaded_sec.flows[0].placement_role.as_deref(), Some("primary"));
        assert_eq!(loaded_sec.flows[1].width_ratio, Some(0.28));
        assert_eq!(loaded_sec.flows[1].placement_role.as_deref(), Some("inner_spine"));
        assert_eq!(loaded_sec.flows[2].width_ratio, Some(0.32));
        assert_eq!(loaded_sec.flows[2].placement_role.as_deref(), Some("outer_margin"));
    }

    #[test]
    fn test_flow_layout_backwards_compatibility() {
        let legacy_json = r#"{
            "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "metadata": {
                "title": "ישן",
                "author": "מחבר",
                "progression": "Rtl",
                "primary_language": "he",
                "schema_version": "1.0"
            },
            "paragraph_styles": [],
            "character_styles": [],
            "sections": [
                {
                    "id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                    "name": "שער",
                    "page_style": "default",
                    "flows": [
                        {
                            "id": "main",
                            "flow_type": "Main",
                            "paragraphs": []
                        }
                    ]
                }
            ]
        }"#;

        let loaded = DocumentRoot::from_json(legacy_json).expect("Legacy JSON must parse");
        let sec = &loaded.sections[0];
        assert!(sec.layout_kind.is_none());
        assert!(sec.column_proportions.is_none());
        assert!(sec.expansion_flow_id.is_none());
        assert!(sec.flows[0].width_ratio.is_none());
        assert!(sec.flows[0].placement_role.is_none());
    }

    #[test]
    fn test_section_node_builder_and_query_methods() {
        let sec = SectionNode::new("פרק ראשון", "default")
            .with_layout_kind("parallel_columns")
            .with_column_proportions(vec![0.6, 0.4])
            .with_expansion_flow_id(FlowId::new("commentary"))
            .with_flows(vec![
                Flow::new(FlowId::new("stream_a"), FlowType::Main),
                Flow::new(FlowId::new("stream_b"), FlowType::CommentA),
            ])
            .with_flow(Flow::new(FlowId::new("stream_c"), FlowType::Footnote));

        assert_eq!(sec.flows.len(), 3);
        assert!(sec.has_flow("stream_a"));
        assert!(sec.has_flow("stream_b"));
        assert!(sec.has_flow("stream_c"));
        assert!(!sec.has_flow("stream_d"));
        assert_eq!(sec.find_flow("stream_b").unwrap().flow_type, FlowType::CommentA);
        assert_eq!(sec.layout_kind.as_deref(), Some("parallel_columns"));
        assert_eq!(sec.column_proportions, Some(vec![0.6, 0.4]));
    }
}

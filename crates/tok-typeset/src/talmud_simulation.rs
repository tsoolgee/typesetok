//! Prototype Talmud Simulation: 3-Stream Co-Pagination and Anchor Synchronization.
//!
//! Demonstrates:
//! - Multi-measure paragraph breaking (`\parshape` / `MeasureProfile`) for central Gemara,
//!   inner Rashi (with dynamic L-shape expansion), and outer Tosafot.
//! - Independent verification of line breaking vs anchor synchronization.
//! - Concrete failure reporting when commentaries overflow or anchors desynchronize.

use crate::engine::TypesettingEngine;
use crate::geometry::LineBox;
use crate::knuth_plass::MeasureProfile;
use crate::multi_flow::SpreadSide;
use std::collections::HashMap;
use std::time::Instant;
use tok_core::id::FractionalIndex;
use tok_core::model::ParagraphNode;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnchorId(pub String);

impl AnchorId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone)]
pub struct GemaraInputChunk {
    pub text: String,
    /// (anchor_id, search_phrase): exact phrase in `text` that this anchor corresponds to.
    pub anchors: Vec<(AnchorId, String)>,
}

#[derive(Debug, Clone)]
pub struct CommentaryInputItem {
    pub target_anchor: AnchorId,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct TalmudSimulationConfig {
    pub page_width_pt: f32,
    pub page_height_pt: f32,
    pub margin_inner_pt: f32,
    pub margin_outer_pt: f32,
    pub margin_top_pt: f32,
    pub margin_bottom_pt: f32,
    pub gutter_pt: f32,
    pub side: SpreadSide,
    pub gemara_font_family: String,
    pub gemara_font_size: f32,
    pub gemara_line_height: f32,
    pub commentary_font_family: String,
    pub commentary_font_size: f32,
    pub commentary_line_height: f32,
    /// Maximum allowed vertical lead (pt) by which commentary may precede its Gemara anchor.
    pub max_anchor_lead_pt: f32,
    /// Maximum allowed vertical lag (pt) by which commentary may trail behind its Gemara anchor.
    pub max_anchor_lag_pt: f32,
}

impl Default for TalmudSimulationConfig {
    fn default() -> Self {
        Self {
            page_width_pt: 595.28,
            page_height_pt: 841.89,
            margin_inner_pt: 40.0,
            margin_outer_pt: 30.0,
            margin_top_pt: 36.0,
            margin_bottom_pt: 36.0,
            gutter_pt: 10.0,
            side: SpreadSide::Verso,
            gemara_font_family: "Frank Ruhl Libre".to_string(),
            gemara_font_size: 13.0,
            gemara_line_height: 18.0,
            commentary_font_family: "Noto Rashi Hebrew".to_string(),
            commentary_font_size: 10.0,
            commentary_line_height: 13.5,
            max_anchor_lead_pt: 40.0, // ~3 commentary lines lead
            max_anchor_lag_pt: 80.0,   // ~6 commentary lines lag
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TalmudSyncError {
    GemaraOverflow {
        gemara_height_pt: f32,
        max_height_pt: f32,
    },
    UnresolvedAnchor {
        flow: String,
        anchor: AnchorId,
    },
    PrematureCommentary {
        flow: String,
        anchor: AnchorId,
        gemara_y_pt: f32,
        commentary_y_pt: f32,
        lead_pt: f32,
    },
    AnchorLag {
        flow: String,
        anchor: AnchorId,
        gemara_y_pt: f32,
        commentary_y_pt: f32,
        lag_pt: f32,
    },
    CommentaryOverflow {
        flow: String,
        commentary_height_pt: f32,
        max_height_pt: f32,
        overflow_pt: f32,
        unplaced_anchors: Vec<AnchorId>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowSummary {
    pub flow_name: String,
    pub line_count: usize,
    pub total_height_pt: f32,
    pub lines: Vec<LineBox>,
    /// Number of lines in this flow that actually use the expanded L-shape width.
    pub expanded_lines_count: usize,
    /// Whether any line in this flow physically formatted in the expanded measure.
    pub uses_expanded_measure: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnchorMapping {
    pub anchor: AnchorId,
    pub gemara_line_index: usize,
    pub gemara_y_pt: f32,
    pub commentary_flow: String,
    pub commentary_line_index: usize,
    pub commentary_y_pt: f32,
    /// Vertical delta (commentary_y - gemara_y). Negative = commentary leads, Positive = commentary lags.
    pub delta_y_pt: f32,
    /// Whether this anchor's vertical delta is within the configured [lead, lag] tolerance window.
    pub within_tolerance: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TalmudSimulationResult {
    /// True if no layout errors occurred and all anchors satisfied tolerance constraints.
    pub passes_tolerance_check: bool,
    /// Alias for `passes_tolerance_check` for backward compatibility.
    pub success: bool,
    /// Explicit flag: this prototype performs passive tolerance validation against geometry thresholds,
    /// NOT active baseline grid locking, vertical spring stretching, or co-pagination convergence.
    pub is_actively_synchronized: bool,
    /// True if the page geometry has vertical clearance under Gemara for an L-shape column.
    pub l_shape_available: bool,
    /// Backward-compatible alias for `l_shape_available`.
    pub has_l_shape: bool,
    /// True if commentary (Rashi) actually reached and typeset into the expanded L-shape measure.
    pub l_shape_actually_used: bool,
    pub gemara: FlowSummary,
    pub rashi: FlowSummary,
    pub tosafot: FlowSummary,
    pub anchor_mappings: Vec<AnchorMapping>,
    pub errors: Vec<TalmudSyncError>,
    pub duration_us: u128,
}

pub struct TalmudSimulator;

impl TalmudSimulator {
    /// Simulates typesetting a 3-stream Talmud page and validates both line breaking and anchor synchronization.
    pub fn simulate_page(
        config: &TalmudSimulationConfig,
        gemara_input: &[GemaraInputChunk],
        rashi_input: &[CommentaryInputItem],
        tosafot_input: &[CommentaryInputItem],
        engine: &TypesettingEngine,
    ) -> TalmudSimulationResult {
        let start_time = Instant::now();
        let mut errors = Vec::new();

        // 1. Compute geometry margins based on Recto / Verso
        let (margin_left, margin_right) = match config.side {
            SpreadSide::Recto => (config.margin_inner_pt, config.margin_outer_pt),
            SpreadSide::Verso => (config.margin_outer_pt, config.margin_inner_pt),
        };
        let content_width = (config.page_width_pt - margin_left - margin_right).max(0.0);
        let content_height = (config.page_height_pt - config.margin_top_pt - config.margin_bottom_pt).max(0.0);
        let available_cols_width = (content_width - 2.0 * config.gutter_pt).max(0.0);

        let gemara_col_width = available_cols_width * 0.40;
        let rashi_narrow_width = available_cols_width * 0.28;
        let tosafot_narrow_width = available_cols_width * 0.32;

        // 2. Typeset Gemara in central column
        let gemara_profile = MeasureProfile::uniform(gemara_col_width);
        let mut gemara_lines: Vec<LineBox> = Vec::new();
        let mut gemara_anchor_positions: HashMap<AnchorId, (usize, f32)> = HashMap::new();

        let mut current_gemara_y = 0.0;
        for (p_idx, chunk) in gemara_input.iter().enumerate() {
            let p_node = ParagraphNode::new(
                FractionalIndex::new(format!("p_{}", p_idx)),
                "gemara",
                &chunk.text,
            );
            let chunk_lines = engine.typeset_paragraph_shaped(
                &p_node,
                &config.gemara_font_family,
                &gemara_profile,
                config.gemara_font_size,
                config.gemara_line_height,
            );

            // Locate each anchor in the broken Gemara lines
            for (anchor_id, phrase) in &chunk.anchors {
                let mut anchor_line_rel = 0;
                let mut found = false;
                for (l_idx, line) in chunk_lines.iter().enumerate() {
                    if line.text.contains(phrase) {
                        anchor_line_rel = l_idx;
                        found = true;
                        break;
                    }
                }
                if !found {
                    // Default to first line of paragraph if exact phrase split across lines
                    anchor_line_rel = 0;
                }
                let abs_line_idx = gemara_lines.len() + anchor_line_rel;
                let abs_y = current_gemara_y + (anchor_line_rel as f32 + 1.0) * config.gemara_line_height;
                gemara_anchor_positions.insert(anchor_id.clone(), (abs_line_idx, abs_y));
            }

            current_gemara_y += chunk_lines.len() as f32 * config.gemara_line_height;
            gemara_lines.extend(chunk_lines);
        }

        let gemara_height = gemara_lines.len() as f32 * config.gemara_line_height;
        if gemara_height > content_height {
            errors.push(TalmudSyncError::GemaraOverflow {
                gemara_height_pt: gemara_height,
                max_height_pt: content_height,
            });
        }

        // 3. Determine L-shape geometry under Gemara
        let space_below_gemara = content_height - gemara_height - config.gutter_pt;
        let l_shape_min_gap = 2.0 * config.commentary_line_height;
        let has_l_shape = space_below_gemara >= l_shape_min_gap;

        let (rashi_narrow_lines, _rashi_profile, rashi_max_lines) = if has_l_shape {
            let narrow_lines = (gemara_height / config.commentary_line_height).ceil() as usize;
            let expanded_width = rashi_narrow_width + gemara_col_width + config.gutter_pt;
            let profile = MeasureProfile::l_shape(narrow_lines, rashi_narrow_width, expanded_width);
            let expanded_lines = (space_below_gemara / config.commentary_line_height).floor() as usize;
            (narrow_lines, profile, narrow_lines + expanded_lines)
        } else {
            let max_lines = (content_height / config.commentary_line_height).floor() as usize;
            (max_lines, MeasureProfile::uniform(rashi_narrow_width), max_lines)
        };

        let tosafot_max_lines = (content_height / config.commentary_line_height).floor() as usize;
        let tosafot_profile = MeasureProfile::uniform(tosafot_narrow_width);

        // 4. Typeset Rashi using dynamic profile
        let mut rashi_lines: Vec<LineBox> = Vec::new();
        let mut rashi_anchor_positions: HashMap<AnchorId, (usize, f32)> = HashMap::new();
        let mut rashi_line_counter = 0;

        for (p_idx, item) in rashi_input.iter().enumerate() {
            let start_line = rashi_line_counter;
            let start_y = start_line as f32 * config.commentary_line_height;
            rashi_anchor_positions.insert(item.target_anchor.clone(), (start_line, start_y));

            // Offset the page profile so this paragraph begins at line `start_line`
            let paragraph_profile = if has_l_shape {
                if start_line < rashi_narrow_lines {
                    let remaining_narrow = rashi_narrow_lines - start_line;
                    let expanded_width = rashi_narrow_width + gemara_col_width + config.gutter_pt;
                    MeasureProfile::l_shape(remaining_narrow, rashi_narrow_width, expanded_width)
                } else {
                    let expanded_width = rashi_narrow_width + gemara_col_width + config.gutter_pt;
                    MeasureProfile::uniform(expanded_width)
                }
            } else {
                MeasureProfile::uniform(rashi_narrow_width)
            };

            let p_node = ParagraphNode::new(
                FractionalIndex::new(format!("p_{}", p_idx)),
                "rashi",
                &item.text,
            );
            let item_lines = engine.typeset_paragraph_shaped(
                &p_node,
                &config.commentary_font_family,
                &paragraph_profile,
                config.commentary_font_size,
                config.commentary_line_height,
            );

            rashi_line_counter += item_lines.len();
            rashi_lines.extend(item_lines);
        }

        // Check Rashi overflow
        if rashi_lines.len() > rashi_max_lines {
            let excess_lines = rashi_lines.len() - rashi_max_lines;
            let excess_pt = excess_lines as f32 * config.commentary_line_height;
            let unplaced: Vec<AnchorId> = rashi_anchor_positions
                .iter()
                .filter(|(_, (l, _))| *l >= rashi_max_lines)
                .map(|(a, _)| a.clone())
                .collect();

            errors.push(TalmudSyncError::CommentaryOverflow {
                flow: "Rashi".to_string(),
                commentary_height_pt: rashi_lines.len() as f32 * config.commentary_line_height,
                max_height_pt: rashi_max_lines as f32 * config.commentary_line_height,
                overflow_pt: excess_pt,
                unplaced_anchors: unplaced,
            });
        }

        // 5. Typeset Tosafot
        let mut tosafot_lines: Vec<LineBox> = Vec::new();
        let mut tosafot_anchor_positions: HashMap<AnchorId, (usize, f32)> = HashMap::new();
        let mut tosafot_line_counter = 0;

        for (p_idx, item) in tosafot_input.iter().enumerate() {
            let start_line = tosafot_line_counter;
            let start_y = start_line as f32 * config.commentary_line_height;
            tosafot_anchor_positions.insert(item.target_anchor.clone(), (start_line, start_y));

            let p_node = ParagraphNode::new(
                FractionalIndex::new(format!("p_{}", p_idx)),
                "tosafot",
                &item.text,
            );
            let item_lines = engine.typeset_paragraph_shaped(
                &p_node,
                &config.commentary_font_family,
                &tosafot_profile,
                config.commentary_font_size,
                config.commentary_line_height,
            );

            tosafot_line_counter += item_lines.len();
            tosafot_lines.extend(item_lines);
        }

        // Check Tosafot overflow
        if tosafot_lines.len() > tosafot_max_lines {
            let excess_lines = tosafot_lines.len() - tosafot_max_lines;
            let excess_pt = excess_lines as f32 * config.commentary_line_height;
            let unplaced: Vec<AnchorId> = tosafot_anchor_positions
                .iter()
                .filter(|(_, (l, _))| *l >= tosafot_max_lines)
                .map(|(a, _)| a.clone())
                .collect();

            errors.push(TalmudSyncError::CommentaryOverflow {
                flow: "Tosafot".to_string(),
                commentary_height_pt: tosafot_lines.len() as f32 * config.commentary_line_height,
                max_height_pt: tosafot_max_lines as f32 * config.commentary_line_height,
                overflow_pt: excess_pt,
                unplaced_anchors: unplaced,
            });
        }

        // 6. Anchor Synchronization Verification (Passive Geometric Tolerance Validation)
        let mut anchor_mappings = Vec::new();

        // Verify Rashi anchors
        for item in rashi_input {
            let anchor_id = &item.target_anchor;
            match gemara_anchor_positions.get(anchor_id) {
                None => {
                    errors.push(TalmudSyncError::UnresolvedAnchor {
                        flow: "Rashi".to_string(),
                        anchor: anchor_id.clone(),
                    });
                }
                Some(&(g_line, g_y)) => {
                    let &(r_line, r_y) = rashi_anchor_positions.get(anchor_id).unwrap();
                    let delta_y = r_y - g_y;
                    let lead_ok = r_y + config.max_anchor_lead_pt >= g_y;
                    let lag_ok = r_y <= g_y + config.max_anchor_lag_pt;
                    let within_tol = lead_ok && lag_ok;

                    anchor_mappings.push(AnchorMapping {
                        anchor: anchor_id.clone(),
                        gemara_line_index: g_line,
                        gemara_y_pt: g_y,
                        commentary_flow: "Rashi".to_string(),
                        commentary_line_index: r_line,
                        commentary_y_pt: r_y,
                        delta_y_pt: delta_y,
                        within_tolerance: within_tol,
                    });

                    // Check for premature commentary (starts too early before Gemara anchor)
                    if !lead_ok {
                        errors.push(TalmudSyncError::PrematureCommentary {
                            flow: "Rashi".to_string(),
                            anchor: anchor_id.clone(),
                            gemara_y_pt: g_y,
                            commentary_y_pt: r_y,
                            lead_pt: g_y - r_y,
                        });
                    }

                    // Check for anchor lag (starts excessively late after Gemara anchor)
                    if !lag_ok {
                        errors.push(TalmudSyncError::AnchorLag {
                            flow: "Rashi".to_string(),
                            anchor: anchor_id.clone(),
                            gemara_y_pt: g_y,
                            commentary_y_pt: r_y,
                            lag_pt: r_y - g_y,
                        });
                    }
                }
            }
        }

        // Verify Tosafot anchors
        for item in tosafot_input {
            let anchor_id = &item.target_anchor;
            match gemara_anchor_positions.get(anchor_id) {
                None => {
                    errors.push(TalmudSyncError::UnresolvedAnchor {
                        flow: "Tosafot".to_string(),
                        anchor: anchor_id.clone(),
                    });
                }
                Some(&(g_line, g_y)) => {
                    let &(t_line, t_y) = tosafot_anchor_positions.get(anchor_id).unwrap();
                    let delta_y = t_y - g_y;
                    let lead_ok = t_y + config.max_anchor_lead_pt >= g_y;
                    let lag_ok = t_y <= g_y + config.max_anchor_lag_pt;
                    let within_tol = lead_ok && lag_ok;

                    anchor_mappings.push(AnchorMapping {
                        anchor: anchor_id.clone(),
                        gemara_line_index: g_line,
                        gemara_y_pt: g_y,
                        commentary_flow: "Tosafot".to_string(),
                        commentary_line_index: t_line,
                        commentary_y_pt: t_y,
                        delta_y_pt: delta_y,
                        within_tolerance: within_tol,
                    });

                    if !lead_ok {
                        errors.push(TalmudSyncError::PrematureCommentary {
                            flow: "Tosafot".to_string(),
                            anchor: anchor_id.clone(),
                            gemara_y_pt: g_y,
                            commentary_y_pt: t_y,
                            lead_pt: g_y - t_y,
                        });
                    }

                    if !lag_ok {
                        errors.push(TalmudSyncError::AnchorLag {
                            flow: "Tosafot".to_string(),
                            anchor: anchor_id.clone(),
                            gemara_y_pt: g_y,
                            commentary_y_pt: t_y,
                            lag_pt: t_y - g_y,
                        });
                    }
                }
            }
        }

        // Count how many lines in Rashi actually format into the expanded L-shape measure
        let rashi_expanded_count = if has_l_shape {
            rashi_lines
                .iter()
                .enumerate()
                .filter(|(idx, line)| *idx >= rashi_narrow_lines || line.width > rashi_narrow_width + 1.0)
                .count()
        } else {
            0
        };
        let rashi_uses_expanded = rashi_expanded_count > 0;
        let l_shape_actually_used = has_l_shape && rashi_uses_expanded;

        let elapsed = start_time.elapsed().as_micros();
        let passes_tolerance = errors.is_empty();

        TalmudSimulationResult {
            passes_tolerance_check: passes_tolerance,
            success: passes_tolerance,
            is_actively_synchronized: false,
            l_shape_available: has_l_shape,
            has_l_shape,
            l_shape_actually_used,
            gemara: FlowSummary {
                flow_name: "Gemara".to_string(),
                line_count: gemara_lines.len(),
                total_height_pt: gemara_height,
                lines: gemara_lines,
                expanded_lines_count: 0,
                uses_expanded_measure: false,
            },
            rashi: FlowSummary {
                flow_name: "Rashi".to_string(),
                line_count: rashi_lines.len(),
                total_height_pt: rashi_lines.len() as f32 * config.commentary_line_height,
                lines: rashi_lines,
                expanded_lines_count: rashi_expanded_count,
                uses_expanded_measure: rashi_uses_expanded,
            },
            tosafot: FlowSummary {
                flow_name: "Tosafot".to_string(),
                line_count: tosafot_lines.len(),
                total_height_pt: tosafot_lines.len() as f32 * config.commentary_line_height,
                lines: tosafot_lines,
                expanded_lines_count: 0,
                uses_expanded_measure: false,
            },
            anchor_mappings,
            errors,
            duration_us: elapsed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::TypesettingEngineConfig;

    fn test_engine() -> TypesettingEngine {
        TypesettingEngine::new(TypesettingEngineConfig::default())
    }

    #[test]
    fn test_talmud_simulation_success_and_l_shape() {
        let engine = test_engine();
        let config = TalmudSimulationConfig::default();

        let gemara = vec![
            GemaraInputChunk {
                text: "מאימתי קורין את שמע בערבין משעה שהכהנים נכנסים לאכול בתרומתן עד סוף האשמורה הראשונה דברי רבי אליעזר וחכמים אומרים עד חצות רבן גמליאל אומר עד שיעלה עמוד השחר".to_string(),
                anchors: vec![
                    (AnchorId::new("dh_meematai"), "מאימתי".to_string()),
                    (AnchorId::new("dh_ad_chatzot"), "עד חצות".to_string()),
                ],
            },
            GemaraInputChunk {
                text: "תנא היכא קאי דקתני מאימתי ותו מאי שנא דתני בערבית ברישא לתני דשחרית ברישא תנא אקרא קאי דכתיב בשכבך ובקומך והכי קתני זמן קריאת שמע דשכיבה אימת משעה שהכהנים נכנסים לאכול בתרומתן".to_string(),
                anchors: vec![
                    (AnchorId::new("dh_tana_heicha"), "תנא היכא קאי".to_string()),
                ],
            },
        ];

        // Rashi items: sized so that lines 0..13 stay in the narrow column, and lines 14+
        // actually cross into and utilize the expanded L-shape area under Gemara.
        let rashi = vec![
            CommentaryInputItem {
                target_anchor: AnchorId::new("dh_meematai"),
                text: "מאימתי קורין וכו' - לא קאי אקרא אלא אמשנה דלעיל דתנן תפלת השחר עד חצות, ומשום דבעי למיתני דיני תפלת ערבית פתח במאימתי קורין את שמע בערבין כדי לסמוך גאולה לתפלה".to_string(),
            },
            CommentaryInputItem {
                target_anchor: AnchorId::new("dh_ad_chatzot"),
                text: "עד חצות - כדי להרחיק את האדם מן העבירה שלא יאמר אדם יש לי עוד שהות ועוד יום גדול ומתוך כך יעבור הזמן וימצא עובר על דברי חכמים, וכל העובר על דברי חכמים חייב מיתה כדאיתא בברכות".to_string(),
            },
            CommentaryInputItem {
                target_anchor: AnchorId::new("dh_tana_heicha"),
                text: "תנא היכא קאי - מאיזה מקום למד דין זה שהתחיל לשאול מאימתי קורין כאילו כבר למדנו שחובה לקרות. ומשני תנא אקרא קאי דכתיב בשכבך ובקומך והכי קתני זמן קריאת שמע דשכיבה אימת, משעה שהכהנים נכנסים לאכול בתרומתן. ורש\"י מפרש כאן באריכות את כל סדר המשנה והקשר בין תורה שבכתב לתורה שבעל פה, ומבאר כיצד סדרי התפילה וקריאת שמע נתקנו במקביל לקרבנות התמיד של שחר ושל בין הערבים, וכל הלכותיהן מבוארות בסוגיא זו בהרחבה רבה, ומכאן נלמד לכל שאר המקומות שבהם פתח התנא בדין מבלי להקדים לו מקור מפורש.".to_string(),
            },
        ];

        let tosafot = vec![
            CommentaryInputItem {
                target_anchor: AnchorId::new("dh_meematai"),
                text: "מאימתי קורין את שמע בערבין - תימא דבכל מקום מקדים תפלת שחרית לתפלת ערבית כדכתיב ויהי ערב ויהי בוקר, ואומר ר\"י דמשום דקרא פתח בשכיבה ברישא לכך תני נמי סדר זה כדי שיהא לילה ויום כסדר הבריאה, ועוד יש לומר דמתוך שהלילה קודם ליום בדיני התורה לכך פתח בדיני מעריב.".to_string(),
            },
            CommentaryInputItem {
                target_anchor: AnchorId::new("dh_tana_heicha"),
                text: "תנא היכא קאי - אור\"י דקאי אקרא דבשכבך ובקומך ומשום הכי פתח בשכיבה ברישא, ואף על גב דבסדר התמיד הקדים של שחר לשל בין הערבים, הכא גבי קריאת שמע שהיא חובת הגוף בפני עצמה תלוי בזמן שכיבה וקימה.".to_string(),
            },
        ];

        let res = TalmudSimulator::simulate_page(&config, &gemara, &rashi, &tosafot, &engine);

        println!("Simulation duration: {} µs ({} ms)", res.duration_us, res.duration_us as f64 / 1000.0);
        println!("Gemara lines: {}, Rashi lines: {}, Tosafot lines: {}", res.gemara.line_count, res.rashi.line_count, res.tosafot.line_count);
        println!("L-shape clearance available: {}, L-shape actually used by Rashi: {}, expanded lines: {}", res.l_shape_available, res.l_shape_actually_used, res.rashi.expanded_lines_count);

        assert!(res.passes_tolerance_check, "Simulation failed with errors: {:?}", res.errors);
        assert!(res.l_shape_available, "Expected L-shape clearance under early-terminating Gemara");
        // Strict verification: Rashi text MUST physically reach and typeset into the expanded region:
        assert!(res.l_shape_actually_used, "Expected Rashi to physically format lines in the expanded L-shape region");
        assert!(res.rashi.expanded_lines_count >= 5, "Expected >= 5 expanded lines in Rashi, got {}", res.rashi.expanded_lines_count);
        assert!(!res.is_actively_synchronized, "Explicit: prototype is passive geometric tolerance check, not active sync");
        assert_eq!(res.errors.len(), 0);
        assert_eq!(res.anchor_mappings.len(), 5); // 3 Rashi + 2 Tosafot
        for m in &res.anchor_mappings {
            assert!(m.within_tolerance, "Anchor {:?} out of tolerance: delta_y = {}", m.anchor, m.delta_y_pt);
        }

        // Rigorously verify that the expanded lines are actually wider than the narrow lines
        let narrow_width = res.rashi.lines[0].width;
        let expanded_width = res.rashi.lines[res.rashi.lines.len() - 2].width;
        assert!(
            expanded_width > narrow_width + 40.0,
            "Expanded line width ({}) must be significantly wider than narrow line width ({})",
            expanded_width,
            narrow_width
        );
    }

    #[test]
    fn test_negative_rashi_unreached_l_shape() {
        let engine = test_engine();
        let config = TalmudSimulationConfig::default();

        let gemara = vec![
            GemaraInputChunk {
                text: "מאימתי קורין את שמע בערבין משעה שהכהנים נכנסים לאכול בתרומתן עד סוף האשמורה הראשונה".to_string(),
                anchors: vec![
                    (AnchorId::new("dh_1"), "מאימתי".to_string()),
                    (AnchorId::new("dh_2"), "שהכהנים".to_string()),
                ],
            },
        ];

        // Short Rashi: only 2 lines total, well below the narrow lines needed to reach the L-shape
        let rashi = vec![
            CommentaryInputItem {
                target_anchor: AnchorId::new("dh_1"),
                text: "מאימתי קורין וכו' - פירוש קצרצר".to_string(),
            },
            CommentaryInputItem {
                target_anchor: AnchorId::new("dh_2"),
                text: "שהכהנים - פירוש קצרצר נוסף".to_string(),
            },
        ];

        let res = TalmudSimulator::simulate_page(&config, &gemara, &rashi, &[], &engine);

        assert!(res.passes_tolerance_check, "Expected tolerance to pass: {:?}", res.errors);
        assert!(res.l_shape_available, "Geometry has clearance under Gemara");
        // Proves that when text does not reach the expanded area, the system accurately detects it
        assert!(!res.l_shape_actually_used, "Short commentary must NOT be reported as using L-shape");
        assert_eq!(res.rashi.expanded_lines_count, 0, "No lines should be in expanded measure");
        assert!(!res.rashi.uses_expanded_measure);
    }

    #[test]
    fn test_negative_anchor_lag_drift_detected() {
        let engine = test_engine();
        let config = TalmudSimulationConfig::default();

        let gemara = vec![
            GemaraInputChunk {
                text: "מאימתי קורין את שמע בערבין משעה שהכהנים נכנסים לאכול בתרומתן".to_string(),
                anchors: vec![
                    (AnchorId::new("dh_early_1"), "מאימתי".to_string()),
                    (AnchorId::new("dh_early_2"), "שהכהנים".to_string()),
                ],
            },
        ];

        // Commentary 1 is very long (~25 lines), pushing Commentary 2 far down the page
        // Anchor 2 in Gemara is at line 0 or 1 (y <= 36pt).
        // Commentary 2 in Rashi will start around line 25 (y > 300pt).
        // Lag is > 250pt > 80pt max_anchor_lag_pt!
        let long_c1 = (0..20)
            .map(|i| format!("הסבר ארוך ומפותל על המשנה שורה מספר {} שממשיך ומאריך בלי הפסקה", i))
            .collect::<Vec<_>>()
            .join(" ");

        let rashi = vec![
            CommentaryInputItem {
                target_anchor: AnchorId::new("dh_early_1"),
                text: long_c1,
            },
            CommentaryInputItem {
                target_anchor: AnchorId::new("dh_early_2"),
                text: "פירוש על שהכהנים שמתחיל באיחור עצום ביחס למיקום שלו בגמרא".to_string(),
            },
        ];

        let res = TalmudSimulator::simulate_page(&config, &gemara, &rashi, &[], &engine);

        assert!(!res.passes_tolerance_check, "Expected tolerance failure due to excessive anchor lag");
        let has_lag_error = res.errors.iter().any(|e| {
            matches!(e, TalmudSyncError::AnchorLag { flow, anchor, lag_pt, .. }
                if flow == "Rashi" && anchor.0 == "dh_early_2" && *lag_pt > config.max_anchor_lag_pt)
        });
        assert!(has_lag_error, "Expected AnchorLag error for dh_early_2 in {:?}", res.errors);
    }

    #[test]
    fn test_negative_gemara_overflow_detected() {
        let engine = test_engine();
        let config = TalmudSimulationConfig::default();

        // Gemara text repeated enough times to exceed full content height (~770 pt, ~43 lines)
        let huge_gemara = (0..30)
            .map(|i| format!("שורה מספר {} בגמרא שחוזרת על עצמה שוב ושוב כדי לעבור את כל גובה הדף המותר", i))
            .collect::<Vec<_>>()
            .join(" ");

        let gemara = vec![
            GemaraInputChunk {
                text: huge_gemara,
                anchors: vec![(AnchorId::new("dh_1"), "שורה מספר 0".to_string())],
            },
        ];

        let res = TalmudSimulator::simulate_page(&config, &gemara, &[], &[], &engine);

        assert!(!res.passes_tolerance_check, "Expected failure due to Gemara overflow");
        let has_gemara_overflow = res.errors.iter().any(|e| matches!(e, TalmudSyncError::GemaraOverflow { .. }));
        assert!(has_gemara_overflow, "Expected GemaraOverflow in {:?}", res.errors);
    }

    #[test]
    fn test_talmud_simulation_commentary_overflow_reported() {
        let engine = test_engine();
        let config = TalmudSimulationConfig::default();

        let gemara = vec![GemaraInputChunk {
            text: "מאימתי קורין את שמע בערבין משעה שהכהנים נכנסים לאכול בתרומתן".to_string(),
            anchors: vec![(AnchorId::new("dh_1"), "מאימתי".to_string())],
        }];

        // 5 commentary paragraphs that together exceed page capacity (5 * ~15 lines = ~75 lines vs 56 max)
        let paragraph_text = (0..12)
            .map(|i| format!("שורה מספר {} בפירוש רש\"י המבאר בהרחבה את דברי הגמרא כדי להמחיש גלישה מעבר לקיבולת הדף", i))
            .collect::<Vec<_>>()
            .join(" ");

        let rashi: Vec<CommentaryInputItem> = (0..5)
            .map(|i| CommentaryInputItem {
                target_anchor: AnchorId::new("dh_1"),
                text: format!("דיבור המתחיל חלק {} - {}", i, paragraph_text),
            })
            .collect();

        let res = TalmudSimulator::simulate_page(&config, &gemara, &rashi, &[], &engine);

        assert!(!res.passes_tolerance_check, "Expected failure due to commentary overflow");
        assert!(!res.success);
        let has_overflow_error = res.errors.iter().any(|e| matches!(e, TalmudSyncError::CommentaryOverflow { flow, .. } if flow == "Rashi"));
        assert!(has_overflow_error, "Expected CommentaryOverflow error in {:?}", res.errors);
    }

    #[test]
    fn test_talmud_simulation_unresolved_anchor_reported() {
        let engine = test_engine();
        let config = TalmudSimulationConfig::default();

        let gemara = vec![GemaraInputChunk {
            text: "מאימתי קורין את שמע בערבין".to_string(),
            anchors: vec![(AnchorId::new("dh_exists"), "מאימתי".to_string())],
        }];

        let tosafot = vec![CommentaryInputItem {
            target_anchor: AnchorId::new("dh_missing"),
            text: "פירוש על עוגן שלא קיים כלל בטקסט הגמרא".to_string(),
        }];

        let res = TalmudSimulator::simulate_page(&config, &gemara, &[], &tosafot, &engine);

        assert!(!res.success);
        let has_unresolved = res.errors.iter().any(|e| matches!(e, TalmudSyncError::UnresolvedAnchor { anchor, .. } if anchor.0 == "dh_missing"));
        assert!(has_unresolved, "Expected UnresolvedAnchor error in {:?}", res.errors);
    }

    #[test]
    fn test_talmud_simulation_premature_commentary_reported() {
        let engine = test_engine();
        let mut config = TalmudSimulationConfig::default();
        config.max_anchor_lead_pt = 20.0; // tight lead constraint

        // Gemara text with anchor appearing only at the very end
        let filler = (0..20).map(|i| format!("שורה מספר {} בגמרא למילוי המרחק", i)).collect::<Vec<_>>().join(" ");
        let gemara_text = format!("{} סיום הגמרא עם עוגן מאוחר מאוד", filler);

        let gemara = vec![GemaraInputChunk {
            text: gemara_text,
            anchors: vec![(AnchorId::new("dh_late"), "סיום הגמרא".to_string())],
        }];

        // Rashi commentary starting at line 0, while its anchor is at ~line 15
        let rashi = vec![CommentaryInputItem {
            target_anchor: AnchorId::new("dh_late"),
            text: "פירוש שמתחיל מיד בראש העמוד לפני שהטקסט שלו הוזכר בגמרא".to_string(),
        }];

        let res = TalmudSimulator::simulate_page(&config, &gemara, &rashi, &[], &engine);

        assert!(!res.success);
        let has_premature = res.errors.iter().any(|e| matches!(e, TalmudSyncError::PrematureCommentary { anchor, .. } if anchor.0 == "dh_late"));
        assert!(has_premature, "Expected PrematureCommentary error in {:?}", res.errors);
    }

    #[test]
    fn test_talmud_simulation_performance_benchmark() {
        let engine = test_engine();
        let config = TalmudSimulationConfig::default();

        let gemara = vec![
            GemaraInputChunk {
                text: "מאימתי קורין את שמע בערבין משעה שהכהנים נכנסים לאכול בתרומתן עד סוף האשמורה הראשונה דברי רבי אליעזר וחכמים אומרים עד חצות".to_string(),
                anchors: vec![(AnchorId::new("dh_1"), "מאימתי".to_string())],
            },
            GemaraInputChunk {
                text: "תנא היכא קאי דקתני מאימתי ותו מאי שנא דתני בערבית ברישא לתני דשחרית ברישא תנא אקרא קאי דכתיב בשכבך ובקומך".to_string(),
                anchors: vec![(AnchorId::new("dh_2"), "תנא היכא קאי".to_string())],
            },
        ];

        let rashi = vec![
            CommentaryInputItem {
                target_anchor: AnchorId::new("dh_1"),
                text: "מאימתי קורין וכו' - לא קאי אקרא אלא אמשנה דלעיל דתנן תפלת השחר עד חצות, ומשום דבעי למיתני דיני תפלת ערבית פתח במאימתי קורין את שמע בערבין כדי לסמוך גאולה לתפלה".to_string(),
            },
            CommentaryInputItem {
                target_anchor: AnchorId::new("dh_2"),
                text: "תנא היכא קאי - מאיזה מקום למד דין זה שהתחיל לשאול מאימתי קורין כאילו כבר למדנו שחובה לקרות".to_string(),
            },
        ];

        let tosafot = vec![
            CommentaryInputItem {
                target_anchor: AnchorId::new("dh_1"),
                text: "מאימתי קורין את שמע בערבין - תימא דבכל מקום מקדים תפלת שחרית לתפלת ערבית".to_string(),
            },
        ];

        // Benchmark 10 iterations
        let iterations = 10;
        let mut total_duration_us: u128 = 0;
        for _ in 0..iterations {
            let res = TalmudSimulator::simulate_page(&config, &gemara, &rashi, &tosafot, &engine);
            assert!(res.success);
            total_duration_us += res.duration_us;
        }

        let avg_us = total_duration_us / iterations;
        let avg_ms = avg_us as f64 / 1000.0;
        println!("\n==========================================");
        println!("Talmud Simulation Benchmark (average over {} runs):", iterations);
        println!("Average page layout duration: {} µs ({:.3} ms)", avg_us, avg_ms);
        println!("==========================================\n");
    }
}


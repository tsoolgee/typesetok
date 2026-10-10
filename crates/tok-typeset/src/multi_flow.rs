//! Multi-flow Constraint Solver for Talmud (ש"ס) and Mikraot Gedolot.
//!
//! Synchronizes 3-8 parallel commentary flows on the same page/spread,
//! using spread-level slack bounding (2%-5%) to contain spillover cascades.
//! Supports:
//! - Recto / Verso Facing Spreads (Rashi on inner spine margin, Tosafot on outer margin).
//! - Dynamic L-Shaped commentary expansion below early-terminating Gemara text ("צורת הדף").
//! - Floating Footnotes with bottom-page height reservation and gutter separation.

use tok_core::FlowId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreadSide {
    /// Right-hand page in Hebrew RTL book (odd page: 1, 3, 5...). Spine is on the LEFT.
    Recto,
    /// Left-hand page in Hebrew RTL book (even page: 2, 4, 6...). Spine is on the RIGHT.
    Verso,
}

/// Placement role of a text stream on a page layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlowPlacementRole {
    /// Primary content stream (central column in Talmud/classic, or leading column in dual-stream).
    #[default]
    Primary,
    /// Commentary along the inner spine edge (left on Recto, right on Verso in RTL).
    InnerSpine,
    /// Commentary along the outer margin edge (right on Recto, left on Verso in RTL).
    OuterMargin,
    /// Footnote or apparatus band across the bottom of the page.
    BottomBand,
    /// Arbitrary parallel column index (0-based across physical spread).
    Column(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowGeometrySpec {
    pub flow_id: FlowId,
    pub priority: u8,
    pub role: FlowPlacementRole,
    pub width_ratio: Option<f32>,
    pub min_width_pt: f32,
    pub max_width_pt: f32,
    pub target_height_pt: f32,
}

impl FlowGeometrySpec {
    pub fn new(flow_id: FlowId, priority: u8) -> Self {
        let role = match priority {
            1 => FlowPlacementRole::Primary,
            2 => FlowPlacementRole::InnerSpine,
            3 => FlowPlacementRole::OuterMargin,
            _ => FlowPlacementRole::Primary,
        };
        Self {
            flow_id,
            priority,
            role,
            width_ratio: None,
            min_width_pt: 50.0,
            max_width_pt: 1000.0,
            target_height_pt: 500.0,
        }
    }

    pub fn with_role(mut self, role: FlowPlacementRole) -> Self {
        self.role = role;
        self
    }

    pub fn with_width_ratio(mut self, ratio: f32) -> Self {
        self.width_ratio = Some(ratio);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SolvedFlowAllocation {
    pub flow_id: FlowId,
    pub allocated_x_pt: f32,
    pub allocated_y_pt: f32,
    pub allocated_width_pt: f32,
    pub allocated_height_pt: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicTalmudPageResult {
    pub allocations: Vec<SolvedFlowAllocation>,
    pub footnote_allocation: Option<SolvedFlowAllocation>,
    pub gemara_height_pt: f32,
    pub has_l_shape_expansion: bool,
}

/// Column a flow is placed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowRole {
    Main,
    Rashi,
    Tosafot,
}

impl FlowRole {
    fn of(spec: &FlowGeometrySpec) -> Self {
        match spec.role {
            FlowPlacementRole::Primary => FlowRole::Main,
            FlowPlacementRole::InnerSpine => FlowRole::Rashi,
            FlowPlacementRole::OuterMargin => FlowRole::Tosafot,
            _ => {
                if spec.priority == 1 || spec.flow_id.0 == "main" {
                    FlowRole::Main
                } else if spec.priority == 2 {
                    FlowRole::Rashi
                } else {
                    FlowRole::Tosafot
                }
            }
        }
    }
}

/// Clamps a length to a finite, non-negative value.
fn non_negative(v: f32) -> f32 {
    if v.is_finite() {
        v.max(0.0)
    } else {
        0.0
    }
}

pub struct MultiFlowSolver;

impl MultiFlowSolver {
    pub const MAX_ITERATIONS: usize = 1000;
    pub const DEFAULT_GUTTER_PT: f32 = 12.0;
    /// Minimum gap between Gemara bottom and the free space that triggers the L-shape.
    const L_SHAPE_MIN_GAP_PT: f32 = 30.0;

    fn margins(margin_inner_pt: f32, margin_outer_pt: f32, side: SpreadSide) -> (f32, f32) {
        match side {
            SpreadSide::Recto => (margin_inner_pt, margin_outer_pt),
            SpreadSide::Verso => (margin_outer_pt, margin_inner_pt),
        }
    }

    /// Gutter between columns, shrunk on pages too narrow for the default.
    fn gutter_for(content_width: f32) -> f32 {
        Self::DEFAULT_GUTTER_PT.min(content_width * 0.05)
    }

    /// Solves the geometric partitioning for a classic Talmudic page layout:
    /// Center: Main Gemara text
    /// Inner column (Right in RTL spread): Rashi commentary
    /// Outer column (Left in RTL spread): Tosafot commentary
    pub fn solve_talmud_page(
        page_width_pt: f32,
        page_height_pt: f32,
        margin_x_pt: f32,
        margin_y_pt: f32,
        flows: &[FlowGeometrySpec],
        _slack_ratio: f32,
    ) -> Vec<SolvedFlowAllocation> {
        Self::solve_talmud_spread(
            page_width_pt,
            page_height_pt,
            margin_x_pt,
            margin_x_pt,
            margin_y_pt,
            flows,
            SpreadSide::Verso, // Default legacy orientation
        )
    }

    /// Solves the geometric partitioning for facing spreads (Recto vs Verso):
    /// In Hebrew typography (reading right to left):
    /// - Recto (Right page): Spine is on the LEFT.
    ///   Inner margin = Left (Rashi), Outer margin = Right (Tosafot).
    /// - Verso (Left page): Spine is on the RIGHT.
    ///   Inner margin = Right (Rashi), Outer margin = Left (Tosafot).
    pub fn solve_talmud_spread(
        page_width_pt: f32,
        page_height_pt: f32,
        margin_inner_pt: f32,
        margin_outer_pt: f32,
        margin_y_pt: f32,
        flows: &[FlowGeometrySpec],
        side: SpreadSide,
    ) -> Vec<SolvedFlowAllocation> {
        Self::solve_generic_spread(
            page_width_pt,
            page_height_pt,
            margin_inner_pt,
            margin_outer_pt,
            margin_y_pt,
            flows,
            side,
        )
    }

    /// Solves layout allocation across a spread for an arbitrary number of flows (1, 2, 3, 4, N)
    /// using constraint specifications, roles, and width ratios.
    pub fn solve_generic_spread(
        page_width_pt: f32,
        page_height_pt: f32,
        margin_inner_pt: f32,
        margin_outer_pt: f32,
        margin_y_pt: f32,
        flows: &[FlowGeometrySpec],
        side: SpreadSide,
    ) -> Vec<SolvedFlowAllocation> {
        if flows.is_empty() {
            return Vec::new();
        }

        let (margin_left, margin_right) = Self::margins(margin_inner_pt, margin_outer_pt, side);
        let content_width = non_negative(page_width_pt - margin_left - margin_right);
        let content_height = non_negative(page_height_pt - 2.0 * margin_y_pt);

        let alloc = |flow_id: &FlowId, x: f32, w: f32| SolvedFlowAllocation {
            flow_id: flow_id.clone(),
            allocated_x_pt: x,
            allocated_y_pt: margin_y_pt,
            allocated_width_pt: w,
            allocated_height_pt: content_height,
        };

        if flows.len() == 1 {
            return vec![alloc(&flows[0].flow_id, margin_left, content_width)];
        }

        let gutter = Self::gutter_for(content_width);
        let num_cols = flows.len();
        let total_gutters = (num_cols - 1) as f32 * gutter;
        let available_cols_width = non_negative(content_width - total_gutters);

        // Check if flows specify custom width ratios
        let has_custom_ratios = flows.iter().any(|f| f.width_ratio.is_some());
        if has_custom_ratios {
            let total_ratio: f32 = flows
                .iter()
                .map(|f| f.width_ratio.unwrap_or(1.0 / num_cols as f32))
                .sum();
            let norm_factor = if total_ratio > 0.0 {
                1.0 / total_ratio
            } else {
                1.0
            };

            let mut allocations = Vec::new();
            let mut current_x = margin_left;
            for f in flows {
                let ratio = f.width_ratio.unwrap_or(1.0 / num_cols as f32) * norm_factor;
                let col_w = available_cols_width * ratio;
                allocations.push(alloc(&f.flow_id, current_x, col_w));
                current_x += col_w + gutter;
            }
            return allocations;
        }

        // Standard 2-stream: (Primary 60%, Secondary 40%)
        if num_cols == 2 {
            let primary_idx = flows
                .iter()
                .position(|f| f.role == FlowPlacementRole::Primary || f.priority == 1)
                .unwrap_or(0);
            let secondary_idx = if primary_idx == 0 { 1 } else { 0 };

            let primary_w = available_cols_width * 0.60;
            let secondary_w = available_cols_width * 0.40;

            let is_inner_secondary = flows[secondary_idx].role == FlowPlacementRole::InnerSpine
                || flows[secondary_idx].priority == 2;
            let (left_idx, left_w, right_idx, right_w) = match (side, is_inner_secondary) {
                (SpreadSide::Recto, true) | (SpreadSide::Verso, false) => {
                    (secondary_idx, secondary_w, primary_idx, primary_w)
                }
                _ => (primary_idx, primary_w, secondary_idx, secondary_w),
            };

            let x_left = margin_left;
            let x_right = x_left + left_w + gutter;

            let mut result = vec![
                alloc(&flows[0].flow_id, 0.0, 0.0),
                alloc(&flows[1].flow_id, 0.0, 0.0),
            ];
            result[left_idx] = alloc(&flows[left_idx].flow_id, x_left, left_w);
            result[right_idx] = alloc(&flows[right_idx].flow_id, x_right, right_w);
            return result;
        }

        // Standard 3-stream: Talmud layout (Main 40%, Inner 28%, Outer 32%)
        if num_cols == 3 {
            let rashi_width = available_cols_width * 0.28;
            let main_width = available_cols_width * 0.40;
            let tosafot_width = available_cols_width * 0.32;

            let (left_w, right_w) = match side {
                SpreadSide::Verso => (tosafot_width, rashi_width),
                SpreadSide::Recto => (rashi_width, tosafot_width),
            };
            let x_left = margin_left;
            let x_center = x_left + left_w + gutter;
            let x_right = x_center + main_width + gutter;

            return flows
                .iter()
                .map(|f| {
                    let (x, w) = match (FlowRole::of(f), side) {
                        (FlowRole::Main, _) => (x_center, main_width),
                        (FlowRole::Rashi, SpreadSide::Recto)
                        | (FlowRole::Tosafot, SpreadSide::Verso) => (x_left, left_w),
                        (FlowRole::Rashi, SpreadSide::Verso)
                        | (FlowRole::Tosafot, SpreadSide::Recto) => (x_right, right_w),
                    };
                    alloc(&f.flow_id, x, w)
                })
                .collect();
        }

        // 4-stream: Mikraot Gedolot / 4 columns (Primary 36%, Inner 24%, Outer1 22%, Outer2 18%)
        if num_cols == 4 {
            let primary_w = available_cols_width * 0.36;
            let inner_w = available_cols_width * 0.24;
            let outer1_w = available_cols_width * 0.22;
            let outer2_w = available_cols_width * 0.18;

            let widths = match side {
                SpreadSide::Recto => [inner_w, primary_w, outer1_w, outer2_w],
                SpreadSide::Verso => [outer2_w, outer1_w, primary_w, inner_w],
            };

            let mut allocations = Vec::new();
            let mut curr_x = margin_left;
            for (i, f) in flows.iter().enumerate() {
                let w = widths[i];
                allocations.push(alloc(&f.flow_id, curr_x, w));
                curr_x += w + gutter;
            }
            return allocations;
        }

        // Generic N columns (>= 5): distribute evenly across available width
        let col_w = available_cols_width / num_cols as f32;
        let mut allocations = Vec::new();
        let mut curr_x = margin_left;
        for f in flows {
            allocations.push(alloc(&f.flow_id, curr_x, col_w));
            curr_x += col_w + gutter;
        }
        allocations
    }

    /// Solves dynamic Talmud layout with L-shaped commentary expansion below Gemara
    /// and bottom-anchored floating footnotes.
    #[allow(clippy::too_many_arguments)]
    pub fn solve_talmud_dynamic_with_footnotes(
        page_width_pt: f32,
        page_height_pt: f32,
        margin_inner_pt: f32,
        margin_outer_pt: f32,
        margin_y_pt: f32,
        flows: &[FlowGeometrySpec],
        side: SpreadSide,
        gemara_target_height_pt: Option<f32>,
        footnote_target_height_pt: Option<f32>,
    ) -> DynamicTalmudPageResult {
        let (margin_left, margin_right) = Self::margins(margin_inner_pt, margin_outer_pt, side);

        let content_width = non_negative(page_width_pt - margin_left - margin_right);
        let mut available_height = non_negative(page_height_pt - 2.0 * margin_y_pt);
        let gutter = Self::gutter_for(content_width);

        // 1. Allocate Floating Footnotes at bottom
        let footnote_allocation = match footnote_target_height_pt.map(non_negative) {
            Some(fn_height) if fn_height > 0.0 => {
                let actual_fn_height = fn_height.min(available_height * 0.40);
                let fn_y = margin_y_pt + available_height - actual_fn_height;
                available_height = non_negative(available_height - actual_fn_height - gutter);

                Some(SolvedFlowAllocation {
                    flow_id: FlowId::new("footnote"),
                    allocated_x_pt: margin_left,
                    allocated_y_pt: fn_y,
                    allocated_width_pt: content_width,
                    allocated_height_pt: actual_fn_height,
                })
            }
            _ => None,
        };

        // 2. Compute 3-column allocation
        let mut allocations = Self::solve_talmud_spread(
            page_width_pt,
            available_height + 2.0 * margin_y_pt,
            margin_inner_pt,
            margin_outer_pt,
            margin_y_pt,
            flows,
            side,
        );

        // 3. Dynamic L-Shaped expansion if Gemara ends before the bottom and
        //    there is a commentary to flow into the freed space.
        let main_index = flows.iter().position(|f| FlowRole::of(f) == FlowRole::Main);
        let has_commentary = flows.iter().any(|f| FlowRole::of(f) != FlowRole::Main);
        let mut has_l_shape = false;
        let gemara_h = match gemara_target_height_pt {
            Some(target_h) => {
                let actual_gemara_h = non_negative(target_h).min(available_height);
                if actual_gemara_h < available_height - Self::L_SHAPE_MIN_GAP_PT {
                    if let (Some(main_index), true) = (main_index, has_commentary) {
                        let gemara_alloc = &mut allocations[main_index];
                        gemara_alloc.allocated_height_pt = actual_gemara_h;
                        let (gx, gw) =
                            (gemara_alloc.allocated_x_pt, gemara_alloc.allocated_width_pt);

                        // Commentary continues in the space freed under the
                        // Gemara column (classic Tzurat HaDaf), without
                        // overlapping the side columns.
                        let expansion_y = margin_y_pt + actual_gemara_h + gutter;
                        let expansion_h = non_negative(available_height - actual_gemara_h - gutter);
                        allocations.push(SolvedFlowAllocation {
                            flow_id: FlowId::new("rashi_expansion"),
                            allocated_x_pt: gx,
                            allocated_y_pt: expansion_y,
                            allocated_width_pt: gw,
                            allocated_height_pt: expansion_h,
                        });
                        has_l_shape = true;
                    }
                }
                actual_gemara_h
            }
            None => available_height,
        };

        DynamicTalmudPageResult {
            allocations,
            footnote_allocation,
            gemara_height_pt: gemara_h,
            has_l_shape_expansion: has_l_shape,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: &str, priority: u8) -> FlowGeometrySpec {
        FlowGeometrySpec::new(FlowId::new(id), priority)
    }

    fn talmud_flows() -> Vec<FlowGeometrySpec> {
        vec![spec("main", 1), spec("rashi", 2), spec("tosafot", 3)]
    }

    fn overlaps(a: &SolvedFlowAllocation, b: &SolvedFlowAllocation) -> bool {
        let eps = 1e-3;
        a.allocated_x_pt + eps < b.allocated_x_pt + b.allocated_width_pt
            && b.allocated_x_pt + eps < a.allocated_x_pt + a.allocated_width_pt
            && a.allocated_y_pt + eps < b.allocated_y_pt + b.allocated_height_pt
            && b.allocated_y_pt + eps < a.allocated_y_pt + a.allocated_height_pt
    }

    #[test]
    fn test_multi_flow_partitioning() {
        let flows = talmud_flows();
        let allocs = MultiFlowSolver::solve_talmud_page(595.0, 842.0, 36.0, 36.0, &flows, 0.03);
        assert_eq!(allocs.len(), 3);
        let content_width = 595.0 - 72.0;
        let total_col_width: f32 = allocs.iter().map(|a| a.allocated_width_pt).sum();
        assert!(total_col_width <= content_width);
    }

    #[test]
    fn test_multi_flow_empty_flows() {
        let allocations = MultiFlowSolver::solve_talmud_page(595.0, 842.0, 42.52, 56.69, &[], 0.1);
        assert!(allocations.is_empty());
    }

    #[test]
    fn test_multi_flow_single_flow() {
        let flows = vec![spec("main", 1)];
        let allocations =
            MultiFlowSolver::solve_talmud_page(595.0, 842.0, 42.52, 56.69, &flows, 0.1);
        assert_eq!(allocations.len(), 1);
    }

    #[test]
    fn test_talmud_facing_spreads_recto_verso() {
        let flows = talmud_flows();

        // Recto (Right page): Rashi should be on the LEFT (inner margin near spine)
        let recto = MultiFlowSolver::solve_talmud_spread(
            595.0,
            842.0,
            40.0,
            20.0,
            36.0,
            &flows,
            SpreadSide::Recto,
        );
        let rashi_recto = recto.iter().find(|a| a.flow_id.0 == "rashi").unwrap();
        let tosafot_recto = recto.iter().find(|a| a.flow_id.0 == "tosafot").unwrap();
        assert!(rashi_recto.allocated_x_pt < tosafot_recto.allocated_x_pt);

        // Verso (Left page): Rashi should be on the RIGHT (inner margin near spine)
        let verso = MultiFlowSolver::solve_talmud_spread(
            595.0,
            842.0,
            40.0,
            20.0,
            36.0,
            &flows,
            SpreadSide::Verso,
        );
        let rashi_verso = verso.iter().find(|a| a.flow_id.0 == "rashi").unwrap();
        let tosafot_verso = verso.iter().find(|a| a.flow_id.0 == "tosafot").unwrap();
        assert!(rashi_verso.allocated_x_pt > tosafot_verso.allocated_x_pt);
    }

    #[test]
    fn test_dynamic_talmud_l_shape_and_footnotes() {
        let flows = talmud_flows();

        // Gemara finishes at 300 pt (out of 770 pt available height)
        // Footnote needs 80 pt
        let result = MultiFlowSolver::solve_talmud_dynamic_with_footnotes(
            595.0,
            842.0,
            36.0,
            36.0,
            36.0,
            &flows,
            SpreadSide::Verso,
            Some(300.0),
            Some(80.0),
        );

        assert!(result.footnote_allocation.is_some());
        let fn_alloc = result.footnote_allocation.unwrap();
        assert_eq!(fn_alloc.allocated_height_pt, 80.0);
        assert!(fn_alloc.allocated_y_pt > 600.0); // Placed at bottom

        assert!(result.has_l_shape_expansion);
        // There should be a rashi_expansion box
        assert!(result
            .allocations
            .iter()
            .any(|a| a.flow_id.0 == "rashi_expansion"));
    }

    /// Regression: the expansion box spanned 25%-75% of the page and overlapped
    /// the full-height side columns.
    #[test]
    fn l_shape_expansion_overlaps_nothing() {
        for side in [SpreadSide::Recto, SpreadSide::Verso] {
            let result = MultiFlowSolver::solve_talmud_dynamic_with_footnotes(
                595.0,
                842.0,
                40.0,
                30.0,
                36.0,
                &talmud_flows(),
                side,
                Some(300.0),
                Some(80.0),
            );
            assert!(result.has_l_shape_expansion);
            let mut all = result.allocations.clone();
            all.extend(result.footnote_allocation.clone());
            for (i, a) in all.iter().enumerate() {
                for b in &all[i + 1..] {
                    assert!(!overlaps(a, b), "{:?} overlaps {:?}", a.flow_id, b.flow_id);
                }
            }
        }
    }

    /// Regression: a main flow identified by priority (not by the id "main")
    /// kept its full height while the expansion was drawn on top of it.
    #[test]
    fn l_shape_shrinks_main_flow_found_by_priority() {
        let flows = vec![spec("gemara", 1), spec("rashi", 2), spec("tosafot", 3)];
        let result = MultiFlowSolver::solve_talmud_dynamic_with_footnotes(
            595.0,
            842.0,
            36.0,
            36.0,
            36.0,
            &flows,
            SpreadSide::Recto,
            Some(200.0),
            None,
        );
        let gemara = result
            .allocations
            .iter()
            .find(|a| a.flow_id.0 == "gemara")
            .unwrap();
        assert_eq!(gemara.allocated_height_pt, 200.0);
        assert!(result.has_l_shape_expansion);
    }

    #[test]
    fn no_expansion_without_commentary() {
        for flows in [vec![], vec![spec("main", 1)]] {
            let result = MultiFlowSolver::solve_talmud_dynamic_with_footnotes(
                595.0,
                842.0,
                36.0,
                36.0,
                36.0,
                &flows,
                SpreadSide::Recto,
                Some(100.0),
                None,
            );
            assert!(!result.has_l_shape_expansion);
            assert!(result
                .allocations
                .iter()
                .all(|a| a.flow_id.0 != "rashi_expansion"));
        }
    }

    #[test]
    fn degenerate_pages_produce_no_negative_sizes() {
        let cases = [
            (100.0, 100.0, 60.0, 60.0, 60.0),
            (20.0, 900.0, 5.0, 5.0, 10.0),
            (595.0, 842.0, 36.0, 36.0, f32::NAN),
        ];
        for (w, h, mi, mo, my) in cases {
            let result = MultiFlowSolver::solve_talmud_dynamic_with_footnotes(
                w,
                h,
                mi,
                mo,
                my,
                &talmud_flows(),
                SpreadSide::Verso,
                Some(-50.0),
                Some(10_000.0),
            );
            for a in result.allocations.iter().chain(&result.footnote_allocation) {
                assert!(a.allocated_width_pt >= 0.0, "{a:?}");
                assert!(a.allocated_height_pt >= 0.0, "{a:?}");
            }
            assert!(result.gemara_height_pt >= 0.0);
        }
        // Narrow page: columns and gutters still fit inside the content box.
        let allocs = MultiFlowSolver::solve_talmud_spread(
            60.0,
            400.0,
            5.0,
            5.0,
            10.0,
            &talmud_flows(),
            SpreadSide::Recto,
        );
        let right = allocs
            .iter()
            .map(|a| a.allocated_x_pt + a.allocated_width_pt)
            .fold(0.0f32, f32::max);
        assert!(right <= 55.0 + 1e-3, "right edge {right}");
    }
}

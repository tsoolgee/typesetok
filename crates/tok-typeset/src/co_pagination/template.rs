//! Template Layout Mathematics & Spread Geometry.
//!
//! Models multi-stream layout mathematics across facing book spreads:
//! - Talmud Classic (Central Gemara, Inner Rashi at spine, Outer Tosafot at margin).
//! - Mikraot Gedolot (Center Scripture, Targum, Rashi inner, Ramban/Ibn Ezra outer).
//! - Academic Dual-Stream (Primary text + commentary/apparatus).
//!
//! Provides mathematically exact Recto/Verso spine-relative geometry, gutter calculation,
//! and dynamic [`MeasureProfile`] synthesis for standard and L-shaped columns.

use crate::knuth_plass::MeasureProfile;
use crate::multi_flow::SpreadSide;

/// Layout template presets for Jewish and academic multi-stream books.
#[derive(Debug, Clone, PartialEq)]
pub enum TemplateKind {
    /// Classic Talmud: Gemara (40%), Rashi (28% on spine), Tosafot (32% on outer margin).
    TalmudClassic,
    /// Mikraot Gedolot: Torah (38%), Targum (18%), Rashi (24%), Additional Commentary (20%).
    MikraotGedolot,
    /// Dual stream: Main text (60%), Commentary (40%).
    AcademicDualStream,
    /// Custom column proportions (percentages summing to 1.0).
    Custom {
        main_ratio: f32,
        inner_commentary_ratio: f32,
        outer_commentary_ratio: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateConfig {
    pub page_width_pt: f32,
    pub page_height_pt: f32,
    pub margin_inner_pt: f32,
    pub margin_outer_pt: f32,
    pub margin_top_pt: f32,
    pub margin_bottom_pt: f32,
    pub gutter_pt: f32,
    pub kind: TemplateKind,
    pub main_font_family: String,
    pub main_font_size: f32,
    pub main_line_height: f32,
    pub commentary_font_family: String,
    pub commentary_font_size: f32,
    pub commentary_line_height: f32,
}

impl Default for TemplateConfig {
    fn default() -> Self {
        Self {
            page_width_pt: 595.28,
            page_height_pt: 841.89,
            margin_inner_pt: 40.0,
            margin_outer_pt: 30.0,
            margin_top_pt: 36.0,
            margin_bottom_pt: 36.0,
            gutter_pt: 10.0,
            kind: TemplateKind::TalmudClassic,
            main_font_family: "Frank Ruhl Libre".to_string(),
            main_font_size: 13.0,
            main_line_height: 18.0,
            commentary_font_family: "Noto Rashi Hebrew".to_string(),
            commentary_font_size: 10.0,
            commentary_line_height: 13.5,
        }
    }
}

/// Geometric allocation of a single column on a page.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnAllocation {
    pub stream_id: String,
    pub x_pt: f32,
    pub y_pt: f32,
    pub width_pt: f32,
    pub max_height_pt: f32,
    pub is_inner_spine: bool,
}

/// Solved mathematics and column allocations for a specific page in a spread.
#[derive(Debug, Clone, PartialEq)]
pub struct SpreadMathematics {
    pub page_number: usize,
    pub side: SpreadSide,
    pub content_width_pt: f32,
    pub content_height_pt: f32,
    pub margin_left_pt: f32,
    pub margin_right_pt: f32,
    pub gutter_pt: f32,
    pub columns: Vec<ColumnAllocation>,
    pub main_column_width_pt: f32,
    pub inner_column_width_pt: f32,
    pub outer_column_width_pt: f32,
}

impl SpreadMathematics {
    /// Determines spread side for a 1-based page number in Hebrew RTL books:
    /// - Odd pages (1, 3, 5...): Recto (right page, spine is on LEFT).
    /// - Even pages (2, 4, 6...): Verso (left page, spine is on RIGHT).
    pub fn side_for_page(page_number: usize) -> SpreadSide {
        if page_number % 2 == 1 {
            SpreadSide::Recto
        } else {
            SpreadSide::Verso
        }
    }

    /// Solves the exact geometry and column coordinates for a given page number.
    pub fn for_page(config: &TemplateConfig, page_number: usize) -> Self {
        let side = Self::side_for_page(page_number);

        // In Hebrew typography (RTL reading order):
        // Recto (right-hand page): Spine is on the LEFT -> margin_left = margin_inner, margin_right = margin_outer.
        // Verso (left-hand page): Spine is on the RIGHT -> margin_left = margin_outer, margin_right = margin_inner.
        let (margin_left, margin_right) = match side {
            SpreadSide::Recto => (config.margin_inner_pt, config.margin_outer_pt),
            SpreadSide::Verso => (config.margin_outer_pt, config.margin_inner_pt),
        };

        let content_width = (config.page_width_pt - margin_left - margin_right).max(0.0);
        let content_height = (config.page_height_pt - config.margin_top_pt - config.margin_bottom_pt).max(0.0);

        let gutter = config.gutter_pt.min(content_width * 0.05).max(0.0);
        let available_cols_width = (content_width - 2.0 * gutter).max(0.0);

        let (main_ratio, inner_ratio, outer_ratio) = match &config.kind {
            TemplateKind::TalmudClassic => (0.40, 0.28, 0.32),
            TemplateKind::MikraotGedolot => (0.38, 0.30, 0.32),
            TemplateKind::AcademicDualStream => (0.60, 0.40, 0.0),
            TemplateKind::Custom {
                main_ratio,
                inner_commentary_ratio,
                outer_commentary_ratio,
            } => (*main_ratio, *inner_commentary_ratio, *outer_commentary_ratio),
        };

        let main_width = available_cols_width * main_ratio;
        let inner_width = available_cols_width * inner_ratio;
        let outer_width = available_cols_width * outer_ratio;

        let mut columns = Vec::new();

        // Position columns left to right across physical page width:
        // Recto: Left is Inner Spine (Rashi), Center is Gemara, Right is Outer Margin (Tosafot).
        // Verso: Left is Outer Margin (Tosafot), Center is Gemara, Right is Inner Spine (Rashi).
        let (col1_name, col1_w, col1_inner) = match side {
            SpreadSide::Recto => ("rashi", inner_width, true),
            SpreadSide::Verso => ("tosafot", outer_width, false),
        };
        let (col3_name, col3_w, col3_inner) = match side {
            SpreadSide::Recto => ("tosafot", outer_width, false),
            SpreadSide::Verso => ("rashi", inner_width, true),
        };

        let x1 = margin_left;
        let x2 = x1 + col1_w + gutter;
        let x3 = x2 + main_width + gutter;

        columns.push(ColumnAllocation {
            stream_id: col1_name.to_string(),
            x_pt: x1,
            y_pt: config.margin_top_pt,
            width_pt: col1_w,
            max_height_pt: content_height,
            is_inner_spine: col1_inner,
        });

        columns.push(ColumnAllocation {
            stream_id: "main".to_string(),
            x_pt: x2,
            y_pt: config.margin_top_pt,
            width_pt: main_width,
            max_height_pt: content_height,
            is_inner_spine: false,
        });

        if outer_ratio > 0.0 {
            columns.push(ColumnAllocation {
                stream_id: col3_name.to_string(),
                x_pt: x3,
                y_pt: config.margin_top_pt,
                width_pt: col3_w,
                max_height_pt: content_height,
                is_inner_spine: col3_inner,
            });
        }

        Self {
            page_number,
            side,
            content_width_pt: content_width,
            content_height_pt: content_height,
            margin_left_pt: margin_left,
            margin_right_pt: margin_right,
            gutter_pt: gutter,
            columns,
            main_column_width_pt: main_width,
            inner_column_width_pt: inner_width,
            outer_column_width_pt: outer_width,
        }
    }

    /// Synthesizes the dynamic [`MeasureProfile`] for a given stream on this page.
    ///
    /// If `gemara_height_pt` indicates that the central text terminates early with clearance
    /// ($\ge 2$ commentary lines below Gemara), the inner commentary (Rashi) expands into the
    /// freed space under Gemara, creating the classical Talmudic L-shape ("צורת הדף").
    pub fn create_measure_profile(
        &self,
        config: &TemplateConfig,
        stream_id: &str,
        gemara_height_pt: Option<f32>,
    ) -> MeasureProfile {
        let is_rashi = stream_id.contains("rashi");
        let is_main = stream_id == "main" || stream_id == "gemara";

        if is_main {
            return MeasureProfile::uniform(self.main_column_width_pt);
        }

        if !is_rashi {
            // Tosafot or other outer commentary uses regular uniform width
            return MeasureProfile::uniform(self.outer_column_width_pt);
        }

        // Inner commentary (Rashi): check for L-shape clearance under Gemara
        if let Some(gemara_h) = gemara_height_pt {
            let space_below = self.content_height_pt - gemara_h - self.gutter_pt;
            let min_gap = 2.0 * config.commentary_line_height;

            if space_below >= min_gap {
                let narrow_lines = (gemara_h / config.commentary_line_height).ceil() as usize;
                let expanded_width = self.inner_column_width_pt + self.main_column_width_pt + self.gutter_pt;
                return MeasureProfile::l_shape(narrow_lines, self.inner_column_width_pt, expanded_width);
            }
        }

        MeasureProfile::uniform(self.inner_column_width_pt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spread_side_recto_verso_alternation() {
        assert_eq!(SpreadMathematics::side_for_page(1), SpreadSide::Recto);
        assert_eq!(SpreadMathematics::side_for_page(2), SpreadSide::Verso);
        assert_eq!(SpreadMathematics::side_for_page(3), SpreadSide::Recto);
        assert_eq!(SpreadMathematics::side_for_page(4), SpreadSide::Verso);
    }

    #[test]
    fn test_spine_relative_margin_mapping() {
        let config = TemplateConfig::default();
        let recto = SpreadMathematics::for_page(&config, 1);
        let verso = SpreadMathematics::for_page(&config, 2);

        // Recto (page 1): Spine on left -> margin_left = 40 (inner), margin_right = 30 (outer)
        assert_eq!(recto.margin_left_pt, 40.0);
        assert_eq!(recto.margin_right_pt, 30.0);

        // Verso (page 2): Spine on right -> margin_left = 30 (outer), margin_right = 40 (inner)
        assert_eq!(verso.margin_left_pt, 30.0);
        assert_eq!(verso.margin_right_pt, 40.0);

        // Content width is invariant across facing spreads
        assert_eq!(recto.content_width_pt, verso.content_width_pt);
    }

    #[test]
    fn test_rashi_and_tosafot_spine_switching() {
        let config = TemplateConfig::default();
        let recto = SpreadMathematics::for_page(&config, 1);
        let verso = SpreadMathematics::for_page(&config, 2);

        // On Recto, column 0 (left) is Rashi (inner spine), column 2 (right) is Tosafot (outer)
        assert_eq!(recto.columns[0].stream_id, "rashi");
        assert!(recto.columns[0].is_inner_spine);
        assert_eq!(recto.columns[1].stream_id, "main");
        assert_eq!(recto.columns[2].stream_id, "tosafot");
        assert!(!recto.columns[2].is_inner_spine);

        // On Verso, column 0 (left) is Tosafot (outer), column 2 (right) is Rashi (inner spine)
        assert_eq!(verso.columns[0].stream_id, "tosafot");
        assert!(!verso.columns[0].is_inner_spine);
        assert_eq!(verso.columns[1].stream_id, "main");
        assert_eq!(verso.columns[2].stream_id, "rashi");
        assert!(verso.columns[2].is_inner_spine);
    }

    #[test]
    fn test_dynamic_l_shape_profile_generation() {
        let config = TemplateConfig::default();
        let math = SpreadMathematics::for_page(&config, 1);

        // Gemara occupies 180 pt of ~770 pt content height
        let gemara_h = 180.0;
        let profile = math.create_measure_profile(&config, "rashi", Some(gemara_h));

        assert!(!profile.is_uniform());
        assert_eq!(profile.width_for_line(0), math.inner_column_width_pt);
        // At line 14 (ceil(180 / 13.5)), should expand to full width (inner + main + gutter)
        let expected_expanded = math.inner_column_width_pt + math.main_column_width_pt + math.gutter_pt;
        assert_eq!(profile.width_for_line(14), expected_expanded);
        assert_eq!(profile.width_for_line(20), expected_expanded);
    }
}

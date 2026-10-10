//! Layout Template Synthesizer.
//!
//! Synthesizes mathematically complete, validated [`LayoutTemplate`] instances
//! from classified layout families and document constraints.

use crate::layout_family::LayoutFamily;
use crate::layout_template::{LayoutTemplate, TemplateConstraintError};
use crate::multi_flow::{FlowGeometrySpec, FlowPlacementRole};
use tok_core::model::{DocumentRoot, FlowType, SectionNode};
use tok_core::FlowId;

pub struct TemplateSynthesizer;

impl TemplateSynthesizer {
    /// Synthesizes a valid, executable [`LayoutTemplate`] for the given section and layout family.
    pub fn synthesize(
        _doc: &DocumentRoot,
        section: &SectionNode,
        family: &LayoutFamily,
        page_width_pt: f32,
        page_height_pt: f32,
        margin_x_pt: f32,
        margin_y_pt: f32,
    ) -> Result<LayoutTemplate, TemplateConstraintError> {
        let mut template = LayoutTemplate::new(family.clone());

        match family {
            LayoutFamily::SingleFlow { flow_id } => {
                Self::synthesize_single_flow(&mut template, section, flow_id)?;
            }
            LayoutFamily::ParallelColumns { column_count, proportions, flow_ids } => {
                Self::synthesize_parallel_columns(
                    &mut template,
                    section,
                    *column_count,
                    proportions.as_deref(),
                    flow_ids,
                )?;
            }
            LayoutFamily::TzuratHaDaf {
                primary_flow,
                spine_inner_flow,
                spine_outer_flow,
                expansion_flow,
                has_bottom_band,
            } => {
                Self::synthesize_tzurat_hadaf(
                    &mut template,
                    section,
                    primary_flow,
                    spine_inner_flow.as_ref(),
                    spine_outer_flow.as_ref(),
                    expansion_flow.as_ref(),
                    *has_bottom_band,
                )?;
            }
            LayoutFamily::FootnotesBand {
                primary_flow,
                footnote_flow,
                column_flows,
            } => {
                Self::synthesize_footnotes_band(
                    &mut template,
                    section,
                    primary_flow,
                    footnote_flow,
                    column_flows,
                )?;
            }
            LayoutFamily::CustomConstraints { flow_ids } => {
                Self::synthesize_custom(&mut template, section, flow_ids)?;
            }
        }

        template.validate(page_width_pt, page_height_pt, margin_x_pt, margin_y_pt)?;
        Ok(template)
    }

    fn synthesize_single_flow(
        template: &mut LayoutTemplate,
        section: &SectionNode,
        target_flow_id: &FlowId,
    ) -> Result<(), TemplateConstraintError> {
        let id = section
            .flows
            .iter()
            .find(|f| f.id == *target_flow_id)
            .map(|f| f.id.clone())
            .unwrap_or_else(|| {
                section
                    .flows
                    .first()
                    .map(|f| f.id.clone())
                    .unwrap_or_else(FlowId::main)
            });

        let spec = FlowGeometrySpec::new(id, 1).with_role(FlowPlacementRole::Primary);
        template.flow_specs = vec![spec];
        template.nominal_proportions = vec![1.0];
        template.has_l_shape_expansion = false;
        template.has_bottom_band = false;
        Ok(())
    }

    fn synthesize_parallel_columns(
        template: &mut LayoutTemplate,
        section: &SectionNode,
        column_count: usize,
        explicit_proportions: Option<&[f32]>,
        flow_ids: &[FlowId],
    ) -> Result<(), TemplateConstraintError> {
        let count = column_count.max(1);
        let mut raw_props = Vec::with_capacity(count);

        // Determine base proportions
        if let Some(props) = explicit_proportions {
            raw_props.extend_from_slice(props);
        } else if let Some(ref sec_props) = section.column_proportions {
            raw_props.extend_from_slice(sec_props);
        } else {
            // Default 2-column: 60% / 40% if primary exists, or equal
            let equal = 1.0 / count as f32;
            for _ in 0..count {
                raw_props.push(equal);
            }
        }

        // Pad or truncate to match count
        while raw_props.len() < count {
            raw_props.push(1.0 / count as f32);
        }
        raw_props.truncate(count);

        // Normalize proportions
        let total: f32 = raw_props.iter().sum();
        let norm_factor = if total > 0.0 && total.is_finite() {
            1.0 / total
        } else {
            1.0
        };
        let normalized: Vec<f32> = raw_props.iter().map(|&p| p * norm_factor).collect();

        let mut specs = Vec::with_capacity(count);
        for i in 0..count {
            let flow_id = flow_ids
                .get(i)
                .cloned()
                .or_else(|| section.flows.get(i).map(|f| f.id.clone()))
                .unwrap_or_else(|| FlowId::new(format!("column_{}", i + 1)));

            let spec = FlowGeometrySpec::new(flow_id, (i + 1) as u8)
                .with_role(FlowPlacementRole::Column(i))
                .with_width_ratio(normalized[i]);
            specs.push(spec);
        }

        template.flow_specs = specs;
        template.nominal_proportions = normalized;
        template.has_l_shape_expansion = false;
        template.has_bottom_band = false;
        Ok(())
    }

    fn synthesize_tzurat_hadaf(
        template: &mut LayoutTemplate,
        section: &SectionNode,
        primary_flow: &FlowId,
        spine_inner: Option<&FlowId>,
        spine_outer: Option<&FlowId>,
        expansion: Option<&FlowId>,
        has_bottom_band: bool,
    ) -> Result<(), TemplateConstraintError> {
        // Fallback for default Talmud roles if not explicitly declared
        let primary_id = section
            .flows
            .iter()
            .find(|f| f.id == *primary_flow || f.flow_type == FlowType::Main || f.id.0 == "gemara")
            .map(|f| f.id.clone())
            .unwrap_or_else(|| primary_flow.clone());

        let inner_id = spine_inner.cloned().or_else(|| {
            section
                .flows
                .iter()
                .find(|f| f.flow_type == FlowType::CommentA || f.id.0 == "rashi")
                .map(|f| f.id.clone())
        });

        let outer_id = spine_outer.cloned().or_else(|| {
            section
                .flows
                .iter()
                .find(|f| f.flow_type == FlowType::CommentB || f.id.0 == "tosafot")
                .map(|f| f.id.clone())
        });

        let mut specs = Vec::new();
        let mut proportions = Vec::new();

        // Standard classic proportions: Primary 40%, Inner 28%, Outer 32%
        // Or derived from explicit flow width_ratios
        let inner_ratio = section
            .flows
            .iter()
            .find(|f| Some(&f.id) == inner_id.as_ref())
            .and_then(|f| f.width_ratio)
            .unwrap_or(0.28);

        let outer_ratio = section
            .flows
            .iter()
            .find(|f| Some(&f.id) == outer_id.as_ref())
            .and_then(|f| f.width_ratio)
            .unwrap_or(0.32);

        let primary_ratio = section
            .flows
            .iter()
            .find(|f| f.id == primary_id)
            .and_then(|f| f.width_ratio)
            .unwrap_or(0.40);

        let primary_spec = FlowGeometrySpec::new(primary_id.clone(), 1)
            .with_role(FlowPlacementRole::Primary)
            .with_width_ratio(primary_ratio);
        specs.push(primary_spec);
        proportions.push(primary_ratio);

        if let Some(in_id) = inner_id {
            if in_id != primary_id {
                let spec = FlowGeometrySpec::new(in_id, 2)
                    .with_role(FlowPlacementRole::InnerSpine)
                    .with_width_ratio(inner_ratio);
                specs.push(spec);
                proportions.push(inner_ratio);
            }
        }

        if let Some(out_id) = outer_id.clone() {
            if out_id != primary_id && !specs.iter().any(|s| s.flow_id == out_id) {
                let spec = FlowGeometrySpec::new(out_id, 3)
                    .with_role(FlowPlacementRole::OuterMargin)
                    .with_width_ratio(outer_ratio);
                specs.push(spec);
                proportions.push(outer_ratio);
            }
        }

        // Add footnote band if present
        if has_bottom_band {
            let note_flow = section
                .flows
                .iter()
                .find(|f| f.flow_type == FlowType::Footnote || f.id.0.contains("note"))
                .map(|f| f.id.clone())
                .unwrap_or_else(|| FlowId::new("notes"));

            if !specs.iter().any(|s| s.flow_id == note_flow) {
                let note_spec = FlowGeometrySpec::new(note_flow.clone(), 4)
                    .with_role(FlowPlacementRole::BottomBand);
                specs.push(note_spec);
                template.footnote_flow_id = Some(note_flow);
            }
        }

        // Expansion target: Outer margin by default in classic Talmud, or explicit target
        let exp_target = expansion.cloned().or(outer_id);
        template.expansion_flow_id = exp_target;
        template.has_l_shape_expansion = template.expansion_flow_id.is_some();
        template.has_bottom_band = has_bottom_band;
        template.flow_specs = specs;
        template.nominal_proportions = proportions;
        Ok(())
    }

    fn synthesize_footnotes_band(
        template: &mut LayoutTemplate,
        section: &SectionNode,
        primary_flow: &FlowId,
        footnote_flow: &FlowId,
        column_flows: &[FlowId],
    ) -> Result<(), TemplateConstraintError> {
        let cols_count = column_flows.len().max(1);
        let equal_prop = 1.0 / cols_count as f32;
        let mut specs = Vec::new();
        let mut proportions = Vec::new();

        if column_flows.is_empty() {
            let spec = FlowGeometrySpec::new(primary_flow.clone(), 1)
                .with_role(FlowPlacementRole::Primary)
                .with_width_ratio(1.0);
            specs.push(spec);
            proportions.push(1.0);
        } else {
            for (idx, f_id) in column_flows.iter().enumerate() {
                let ratio = section
                    .flows
                    .iter()
                    .find(|f| f.id == *f_id)
                    .and_then(|f| f.width_ratio)
                    .unwrap_or(equal_prop);
                let role = if idx == 0 {
                    FlowPlacementRole::Primary
                } else {
                    FlowPlacementRole::Column(idx)
                };
                let spec = FlowGeometrySpec::new(f_id.clone(), (idx + 1) as u8)
                    .with_role(role)
                    .with_width_ratio(ratio);
                specs.push(spec);
                proportions.push(ratio);
            }
        }

        let fn_spec = FlowGeometrySpec::new(footnote_flow.clone(), (specs.len() + 1) as u8)
            .with_role(FlowPlacementRole::BottomBand);
        specs.push(fn_spec);

        template.flow_specs = specs;
        template.nominal_proportions = proportions;
        template.footnote_flow_id = Some(footnote_flow.clone());
        template.has_bottom_band = true;
        template.has_l_shape_expansion = false;
        Ok(())
    }

    fn synthesize_custom(
        template: &mut LayoutTemplate,
        section: &SectionNode,
        flow_ids: &[FlowId],
    ) -> Result<(), TemplateConstraintError> {
        let count = flow_ids.len().max(section.flows.len()).max(1);
        let equal = 1.0 / count as f32;

        let mut specs = Vec::new();
        let mut proportions = Vec::new();

        for (i, f) in section.flows.iter().enumerate() {
            let ratio = f.width_ratio.unwrap_or(equal);
            let role = f
                .placement_role
                .as_deref()
                .map(|r| match r.to_lowercase().as_str() {
                    "primary" => FlowPlacementRole::Primary,
                    "inner_spine" => FlowPlacementRole::InnerSpine,
                    "outer_margin" => FlowPlacementRole::OuterMargin,
                    "bottom_band" => FlowPlacementRole::BottomBand,
                    _ => FlowPlacementRole::Column(i),
                })
                .unwrap_or_else(|| FlowPlacementRole::Column(i));

            let spec = FlowGeometrySpec::new(f.id.clone(), (i + 1) as u8)
                .with_role(role)
                .with_width_ratio(ratio);

            if role == FlowPlacementRole::BottomBand {
                template.footnote_flow_id = Some(f.id.clone());
                template.has_bottom_band = true;
            } else {
                proportions.push(ratio);
            }
            specs.push(spec);
        }

        template.flow_specs = specs;
        template.nominal_proportions = proportions;
        template.expansion_flow_id = section.expansion_flow_id.clone();
        template.has_l_shape_expansion = template.expansion_flow_id.is_some();
        Ok(())
    }
}

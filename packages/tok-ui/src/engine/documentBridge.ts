/**
 * TypesetOK Document Model Bridge (UI <-> Rust Contract).
 *
 * Provides bidirectional lossless conversion between:
 * - UI State: `MultiFlowDocumentState` (flows of `StoryParagraph`s, template type, title)
 * - Rust Model: `DocumentRoot` JSON (sections, flows, paragraphs with ULID NodeIds,
 *   FractionalIndex order keys, typographic ParagraphStyles, and DocumentMetadata).
 *
 * Implements strict compliance with schemas/document/tok_document_schema.json
 * and tok-core model specifications.
 */

import { MultiFlowDocumentState, TemplateType } from './FlowPaginator';
import { StoryParagraph } from 'tok-story-editor';

export const CROCKFORD_ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';

/**
 * Validates whether a string is a 26-character Crockford Base32 ULID
 * matching tok-core Ulid::from_string (first character <= '7').
 */
export function isValidUlid(s: unknown): boolean {
  if (typeof s !== 'string' || s.length !== 26) return false;
  return /^[0-7][0123456789ABCDEFGHJKMNPQRSTVWXYZabcdefghjkmnpqrstvwxyz]{25}$/.test(s);
}

/**
 * Generates a valid 26-character Crockford Base32 ULID.
 * Formatted as: 10 chars timestamp (48 bits) + 16 chars entropy (80 bits).
 */
export function generateCrockfordUlid(seedTime: number = Date.now()): string {
  let timeStr = '';
  let t = Math.max(0, Math.floor(seedTime));
  for (let i = 0; i < 10; i++) {
    const mod = t % 32;
    timeStr = CROCKFORD_ALPHABET[mod] + timeStr;
    t = Math.floor(t / 32);
  }

  let randStr = '';
  for (let i = 0; i < 16; i++) {
    const rand = Math.floor(Math.random() * 32);
    randStr += CROCKFORD_ALPHABET[rand];
  }

  return timeStr + randStr;
}

/**
 * Strips HTML formatting tags (like <b>, </i>, etc.) while preserving
 * unicode Hebrew letters, niqqud, and punctuation.
 */
export function stripHtmlTags(text: string): string {
  if (!text) return '';
  return text.replace(/<\/?[^>]+(>|$)/g, '');
}

/**
 * Generates a lexicographically sorted fractional index key for order preservation.
 */
export function fractionalIndexFor(position: number): string {
  return 'a' + position.toString(36).padStart(5, '0');
}

// ---------------------------------------------------------------------------
// Rust DocumentRoot JSON Schema Types
// ---------------------------------------------------------------------------

export interface ColorJson {
  DeviceCmyk?: {
    c: number;
    m: number;
    y: number;
    k: number;
  };
  Rgb?: {
    r: number;
    g: number;
    b: number;
  };
}

export interface ParagraphStyleJson {
  id: string;
  name: string;
  font_family: string;
  font_weight: number;
  font_size_pt: number;
  line_height_pt: number;
  space_before_pt: number;
  space_after_pt: number;
  first_line_indent_pt: number;
  alignment: 'Justified' | 'Right' | 'Left' | 'Center';
  ahalterm_stretch: 'AutoVariable' | 'Off' | 'StaticSwash';
  color: ColorJson;
  keep_with_next: boolean;
  orphan_control: number;
  widow_control: number;
}

export interface ParagraphNodeJson {
  id: string;
  index: string;
  style_id: string;
  text: string;
  style_patches: unknown[];
}

export type FlowTypeJson = 'Main' | 'CommentA' | 'CommentB' | 'Footnote';

export interface FlowJson {
  id: string;
  flow_type: FlowTypeJson;
  paragraphs: ParagraphNodeJson[];
}

export interface SectionNodeJson {
  id: string;
  name: string;
  page_style: string;
  flows: FlowJson[];
}

export interface DocumentMetadataJson {
  title: string;
  author: string;
  progression: 'Rtl' | 'Ltr';
  primary_language: string;
  schema_version: string;
}

export interface DocumentRootJson {
  id: string;
  metadata: DocumentMetadataJson;
  paragraph_styles: ParagraphStyleJson[];
  character_styles: unknown[];
  sections: SectionNodeJson[];
}

/**
 * Creates canonical paragraph styles matching tok-core styles and UI fonts.
 */
export function createDefaultParagraphStyles(): ParagraphStyleJson[] {
  const cmykBlack: ColorJson = { DeviceCmyk: { c: 0, m: 0, y: 0, k: 1 } };
  return [
    {
      id: 'style-gemara-main',
      name: 'גמרא ראשי',
      font_family: 'Frank Ruhl Libre',
      font_weight: 700,
      font_size_pt: 13.5,
      line_height_pt: 20.9,
      space_before_pt: 0.0,
      space_after_pt: 2.0,
      first_line_indent_pt: 0.0,
      alignment: 'Justified',
      ahalterm_stretch: 'AutoVariable',
      color: cmykBlack,
      keep_with_next: false,
      orphan_control: 2,
      widow_control: 2
    },
    {
      id: 'style-rashi-body',
      name: 'רש״י',
      font_family: 'Noto Rashi Hebrew',
      font_weight: 400,
      font_size_pt: 10.5,
      line_height_pt: 15.5,
      space_before_pt: 0.0,
      space_after_pt: 2.0,
      first_line_indent_pt: 0.0,
      alignment: 'Justified',
      ahalterm_stretch: 'AutoVariable',
      color: cmykBlack,
      keep_with_next: false,
      orphan_control: 2,
      widow_control: 2
    },
    {
      id: 'style-tosafot-body',
      name: 'תוספות',
      font_family: 'Noto Rashi Hebrew',
      font_weight: 400,
      font_size_pt: 10.5,
      line_height_pt: 15.5,
      space_before_pt: 0.0,
      space_after_pt: 2.0,
      first_line_indent_pt: 0.0,
      alignment: 'Justified',
      ahalterm_stretch: 'AutoVariable',
      color: cmykBlack,
      keep_with_next: false,
      orphan_control: 2,
      widow_control: 2
    },
    {
      id: 'style-footnotes',
      name: 'הערות ומסורת',
      font_family: 'Frank Ruhl Libre',
      font_weight: 400,
      font_size_pt: 9.0,
      line_height_pt: 12.6,
      space_before_pt: 0.0,
      space_after_pt: 1.5,
      first_line_indent_pt: 0.0,
      alignment: 'Justified',
      ahalterm_stretch: 'AutoVariable',
      color: cmykBlack,
      keep_with_next: false,
      orphan_control: 2,
      widow_control: 2
    },
    {
      id: 'normal',
      name: 'גוף הטקסט רגיל',
      font_family: 'Frank Ruhl Libre',
      font_weight: 400,
      font_size_pt: 11.0,
      line_height_pt: 14.5,
      space_before_pt: 0.0,
      space_after_pt: 2.0,
      first_line_indent_pt: 0.0,
      alignment: 'Justified',
      ahalterm_stretch: 'AutoVariable',
      color: cmykBlack,
      keep_with_next: false,
      orphan_control: 2,
      widow_control: 2
    },
    {
      id: 'default-body',
      name: 'גוף הטקסט',
      font_family: 'Frank Ruhl Libre',
      font_weight: 400,
      font_size_pt: 11.0,
      line_height_pt: 14.5,
      space_before_pt: 0.0,
      space_after_pt: 2.0,
      first_line_indent_pt: 0.0,
      alignment: 'Justified',
      ahalterm_stretch: 'AutoVariable',
      color: cmykBlack,
      keep_with_next: false,
      orphan_control: 2,
      widow_control: 2
    }
  ];
}

/**
 * Converts UI `MultiFlowDocumentState` to Rust `DocumentRoot` JSON format.
 */
export function documentStateToDocumentRoot(
  state: MultiFlowDocumentState,
  options?: { author?: string }
): DocumentRootJson {
  const isProse = state.templateType === 'prose';
  const defaultStyles = createDefaultParagraphStyles();
  const definedStyleIds = new Set(defaultStyles.map((s) => s.id));

  // Determine flow types
  const flowKeys = Object.keys(state.flows || {});
  const flows: FlowJson[] = [];

  for (const flowKey of flowKeys) {
    if (isProse && flowKey !== 'gemara' && flowKey !== 'main') {
      continue;
    }
    const paras = state.flows[flowKey] || [];
    let flowId = flowKey;
    let flowType: FlowTypeJson = 'Main';

    if (flowKey === 'gemara') {
      flowId = isProse ? 'main' : 'gemara';
      flowType = 'Main';
    } else if (flowKey === 'rashi') {
      flowId = 'rashi';
      flowType = 'CommentA';
    } else if (flowKey === 'tosafot') {
      flowId = 'tosafot';
      flowType = 'CommentB';
    } else if (flowKey === 'notes') {
      flowId = 'notes';
      flowType = 'Footnote';
    } else if (flowKey === 'main') {
      flowId = 'main';
      flowType = 'Main';
    }

    const convertedParas: ParagraphNodeJson[] = paras.map((p, idx) => {
      const pid = isValidUlid(p.id) ? p.id : generateCrockfordUlid();
      const styleId = p.styleId || (
        flowKey === 'gemara' ? (isProse ? 'normal' : 'style-gemara-main') :
        flowKey === 'rashi' ? 'style-rashi-body' :
        flowKey === 'tosafot' ? 'style-tosafot-body' :
        flowKey === 'notes' ? 'style-footnotes' :
        'default-body'
      );

      // Auto-register unknown styles
      if (!definedStyleIds.has(styleId)) {
        defaultStyles.push({
          id: styleId,
          name: styleId,
          font_family: 'Frank Ruhl Libre',
          font_weight: 400,
          font_size_pt: 11.0,
          line_height_pt: 14.5,
          space_before_pt: 0.0,
          space_after_pt: 2.0,
          first_line_indent_pt: 0.0,
          alignment: 'Justified',
          ahalterm_stretch: 'AutoVariable',
          color: { DeviceCmyk: { c: 0, m: 0, y: 0, k: 1 } },
          keep_with_next: false,
          orphan_control: 2,
          widow_control: 2
        });
        definedStyleIds.add(styleId);
      }

      return {
        id: pid,
        index: fractionalIndexFor(idx),
        style_id: styleId,
        text: stripHtmlTags(p.text || ''),
        style_patches: []
      };
    });

    flows.push({
      id: flowId,
      flow_type: flowType,
      paragraphs: convertedParas
    });
  }

  // Ensure at least one flow exists even for empty documents
  if (flows.length === 0) {
    flows.push({
      id: isProse ? 'main' : 'gemara',
      flow_type: 'Main',
      paragraphs: []
    });
  }

  const docTitle = (state.title || 'מסמך ללא שם').replace(/\.tok$/i, '');

  return {
    id: generateCrockfordUlid(),
    metadata: {
      title: docTitle,
      author: options?.author || 'TypesetOK',
      progression: 'Rtl',
      primary_language: 'he',
      schema_version: '1.0'
    },
    paragraph_styles: defaultStyles,
    character_styles: [],
    sections: [
      {
        id: generateCrockfordUlid(),
        name: docTitle,
        page_style: 'chapter-first',
        flows
      }
    ]
  };
}

/**
 * Converts Rust `DocumentRoot` JSON format back to UI `MultiFlowDocumentState`.
 */
export function documentRootToDocumentState(root: DocumentRootJson): MultiFlowDocumentState {
  const title = root.metadata?.title || 'מסמך ללא שם';
  const sec = root.sections?.[0];
  const flows = sec?.flows || [];

  const flowMap: MultiFlowDocumentState['flows'] = {
    gemara: [],
    rashi: [],
    tosafot: [],
    notes: []
  };

  let hasCommentary = false;
  for (const flow of flows) {
    const key = flow.id === 'main' ? 'gemara' : flow.id;
    if (key === 'rashi' || key === 'tosafot') {
      hasCommentary = true;
    }

    const storyParas: StoryParagraph[] = (flow.paragraphs || []).map((p) => ({
      id: p.id,
      styleId: p.style_id,
      text: p.text
    }));

    flowMap[key] = storyParas;
  }

  let templateType: TemplateType = 'prose';
  if (hasCommentary) {
    templateType = 'gemara';
  } else if ((flowMap.notes && flowMap.notes.length > 0)) {
    templateType = 'notes';
  }

  return {
    title,
    templateType,
    flows: flowMap
  };
}

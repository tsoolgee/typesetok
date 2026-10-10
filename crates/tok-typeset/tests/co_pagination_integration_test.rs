//! End-to-end integration tests for multi-stream co-pagination, active anchor
//! synchronization, and asynchronous non-blocking pagination worker.

use std::sync::Arc;
use tok_typeset::{
    AnchorKey, CoPaginatedChunk, CoPaginatedCommentary, CoPaginationDocument, NonBlockingPaginator,
    PaginationEvent, SpreadSide, SyncRemedy, SynchronizerConfig, TemplateConfig, TypesettingEngine,
    TypesettingEngineConfig,
};

fn create_engine() -> TypesettingEngine {
    TypesettingEngine::new(TypesettingEngineConfig::default())
}

#[test]
fn test_multi_page_talmud_spread_facing_pages_alternation() {
    let engine = create_engine();
    let template = TemplateConfig::default();
    let sync_config = SynchronizerConfig::default();

    // Create a multi-page document with 4 chunks of Gemara and matching commentaries
    let mut main_chunks = Vec::new();
    let mut rashi_items = Vec::new();
    let mut tosafot_items = Vec::new();

    for i in 1..=4 {
        let anchor_name = format!("anchor_{}", i);
        main_chunks.push(CoPaginatedChunk {
            text: format!(
                "פסקה ראשית מספר {} מאימתי קורין את שמע בערבית משעה שהכהנים נכנסים לאכול בתרומתן עד סוף האשמורה הראשונה",
                i
            ),
            anchors: vec![(AnchorKey(anchor_name.clone()), "קורין".to_string())],
        });

        // Add enough commentary lines to push across multiple pages
        let rashi_text = (0..15)
            .map(|k| format!("רש\"י {} שורה {}: פירוש רש\"י מפורט הדן בדיני תרומה וזמני קריאת שמע של ערבית עד עמוד השחר וחצות לילה", i, k))
            .collect::<Vec<_>>()
            .join(" ");
        rashi_items.push(CoPaginatedCommentary {
            target_anchor: AnchorKey(anchor_name.clone()),
            text: rashi_text,
        });

        let tosafot_text = (0..15)
            .map(|k| format!("תוספות {} שורה {}: פירש הקונטרס דאקרא קאי ותימה דהא תנן לקמן בשחר מברך שתים לפניה", i, k))
            .collect::<Vec<_>>()
            .join(" ");
        tosafot_items.push(CoPaginatedCommentary {
            target_anchor: AnchorKey(anchor_name.clone()),
            text: tosafot_text,
        });
    }

    let doc = CoPaginationDocument {
        main_chunks,
        rashi_items,
        tosafot_items,
    };

    let result = engine.typeset_co_paginated_document(&template, &sync_config, &doc);
    assert!(
        result.pages.len() >= 2,
        "Expected at least 2 pages, got {}",
        result.pages.len()
    );

    // Verify Recto / Verso alternation
    for (idx, page) in result.pages.iter().enumerate() {
        let expected_recto = (idx + 1) % 2 == 1;
        let expected_side = if expected_recto {
            SpreadSide::Recto
        } else {
            SpreadSide::Verso
        };
        assert_eq!(
            page.math.side, expected_side,
            "Page {} recto/verso mismatch",
            page.page_number
        );

        let rashi_col = page
            .math
            .columns
            .iter()
            .find(|c| c.stream_id == "rashi")
            .unwrap();
        let tosafot_col = page
            .math
            .columns
            .iter()
            .find(|c| c.stream_id == "tosafot")
            .unwrap();

        // Recto: inner margin is on left (spine is left for Hebrew), Rashi is inner (left)
        // Verso: inner margin is on right (spine is right for Hebrew), Rashi is inner (right)
        if expected_recto {
            assert_eq!(page.math.margin_left_pt, template.margin_inner_pt);
            assert_eq!(page.math.margin_right_pt, template.margin_outer_pt);
            assert!(
                rashi_col.x_pt < tosafot_col.x_pt,
                "On Recto, Rashi (inner) must be to the left of Tosafot (outer)"
            );
        } else {
            assert_eq!(page.math.margin_left_pt, template.margin_outer_pt);
            assert_eq!(page.math.margin_right_pt, template.margin_inner_pt);
            assert!(
                tosafot_col.x_pt < rashi_col.x_pt,
                "On Verso, Tosafot (outer) must be to the left of Rashi (inner)"
            );
        }

        // Verify page layout frames
        assert_eq!(
            page.page_layout.frames.len(),
            3,
            "Each page must have 3 flow frames (gemara, rashi, tosafot)"
        );
    }
}

#[test]
fn test_active_synchronization_spring_remedy() {
    let engine = create_engine();
    let template = TemplateConfig::default();
    let sync_config = SynchronizerConfig {
        max_vertical_spring_pt: 100.0,
        max_tolerable_lag_pt: 14.0,
        grid_step_pt: None, // Continuous spring alignment
    };

    let doc = CoPaginationDocument {
        main_chunks: vec![
            CoPaginatedChunk {
                text: "שורת פתיחה ראשונה ללא עוגנים כדי לייצר מרחק אנכי משמעותי במרכז הדף עבור הגמרא והמשנה".to_string(),
                anchors: vec![],
            },
            CoPaginatedChunk {
                text: "ועכשיו מגיע עוגן ראשון הנמצא בשורה נמוכה יחסית במרכז".to_string(),
                anchors: vec![(AnchorKey("a1".to_string()), "עוגן".to_string())],
            },
        ],
        rashi_items: vec![CoPaginatedCommentary {
            target_anchor: AnchorKey("a1".to_string()),
            text: "עוגן: פירוש רש\"י המתחיל מוקדם".to_string(),
        }],
        tosafot_items: vec![],
    };

    let result = engine.typeset_co_paginated_document(&template, &sync_config, &doc);
    assert!(!result.pages.is_empty());
    let page0 = &result.pages[0];

    // Find sync result for anchor "a1"
    let sync_a1 = page0.sync_results.iter().find(|s| s.anchor_key.0 == "a1");
    assert!(
        sync_a1.is_some(),
        "Anchor a1 must have a synchronization record"
    );

    let s = sync_a1.unwrap();
    match s.remedy {
        SyncRemedy::VerticalSpringApplied { spring_pt } => {
            assert!(spring_pt > 0.0, "Spring pt must be positive");
            // Baseline lag after remedy must be within 1.0 pt
            assert!(
                (s.adjusted_y_pt - s.gemara_y_pt).abs() <= 1.0,
                "Effective commentary baseline must match Gemara anchor baseline within 1.0pt"
            );
        }
        SyncRemedy::TolerableLag { lag_pt } => {
            assert!(lag_pt.abs() <= sync_config.max_tolerable_lag_pt);
        }
        _ => {}
    }
}

#[test]
fn test_commentary_overflow_splits_gemara_and_carries_over() {
    let engine = create_engine();
    let template = TemplateConfig::default();
    let sync_config = SynchronizerConfig::default();

    // Gemara has 2 chunks. Chunk 1 has massive commentary that exceeds page height (content_height = ~770pt)
    let massive_rashi = (0..60)
        .map(|idx| {
            format!(
                "רש\"י שורה {} פירוש ארוך מאוד הגודש וממלא את העמוד לחלוטין",
                idx
            )
        })
        .collect::<Vec<_>>()
        .join(" ");

    let doc = CoPaginationDocument {
        main_chunks: vec![
            CoPaginatedChunk {
                text: "גמרא קטע א מאימתי קורין את שמע בערבית משעה שהכהנים נכנסים לאכול".to_string(),
                anchors: vec![(AnchorKey("k1".to_string()), "קורין".to_string())],
            },
            CoPaginatedChunk {
                text: "גמרא קטע ב תנו רבנן מעשה ברבי אליעזר שהיה מסב בבני ברק".to_string(),
                anchors: vec![(AnchorKey("k2".to_string()), "רבנן".to_string())],
            },
        ],
        rashi_items: vec![
            CoPaginatedCommentary {
                target_anchor: AnchorKey("k1".to_string()),
                text: massive_rashi,
            },
            CoPaginatedCommentary {
                target_anchor: AnchorKey("k2".to_string()),
                text: "רבנן: פירוש קצר על קטע ב".to_string(),
            },
        ],
        tosafot_items: vec![],
    };

    let result = engine.typeset_co_paginated_document(&template, &sync_config, &doc);

    // Multi-page splitting must have carried over chunk 2 and subsequent commentary to page 2
    assert!(
        result.pages.len() >= 2,
        "Overflowing commentary must force multi-page split"
    );
    assert!(result.total_gemara_lines > 0);
    assert!(result.total_rashi_lines > 30);
}

#[test]
fn test_async_paginator_incremental_streaming_and_cancellation() {
    let engine = Arc::new(create_engine());
    let template = TemplateConfig::default();
    let sync_config = SynchronizerConfig::default();

    // 1. Verify incremental streaming
    let doc = CoPaginationDocument {
        main_chunks: vec![CoPaginatedChunk {
            text: "משנה ראשונה ברכות פרק ראשון".to_string(),
            anchors: vec![(AnchorKey("m1".to_string()), "משנה".to_string())],
        }],
        rashi_items: vec![CoPaginatedCommentary {
            target_anchor: AnchorKey("m1".to_string()),
            text: "רש\"י על המשנה".to_string(),
        }],
        tosafot_items: vec![],
    };

    let (rx, _token) = NonBlockingPaginator::paginate_async(
        template.clone(),
        sync_config.clone(),
        doc.clone(),
        engine.clone(),
    );

    let mut streamed_pages = Vec::new();
    let mut got_finished = false;

    while let Ok(event) = rx.recv() {
        match event {
            PaginationEvent::PageReady(page) => {
                streamed_pages.push(page.page_number);
            }
            PaginationEvent::Finished(res) => {
                got_finished = true;
                assert_eq!(res.pages.len(), streamed_pages.len());
            }
            _ => {}
        }
    }

    assert!(!streamed_pages.is_empty(), "Must stream pages");
    assert!(got_finished, "Must signal finished");

    // 2. Verify cancellation
    let (rx2, token2) = NonBlockingPaginator::paginate_async(template, sync_config, doc, engine);
    // Cancel immediately
    token2.cancel();

    let mut got_cancelled = false;
    while let Ok(event) = rx2.recv() {
        if let PaginationEvent::Cancelled = event {
            got_cancelled = true;
            break;
        }
    }
    assert!(got_cancelled, "Worker must acknowledge cancellation token");
}

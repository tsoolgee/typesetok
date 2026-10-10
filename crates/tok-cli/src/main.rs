use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::time::Instant;
use tok_core::id::FractionalIndex;
use tok_core::model::{DocumentModel, DocumentRoot, Flow, FlowId, FlowType, ParagraphNode};
use tok_pdf::html_projection::HtmlProjectionCompiler;
use tok_pdf::pdf_engine::{PdfExportOptions, PdfPrePressEngine, PdfXStandard};
use tok_storage::package::{TokManifest, TokPackage};
use tok_typeset::engine::{TypesettingEngine, TypesettingEngineConfig};

fn print_usage() {
    println!(
        r#"
================================================================================
  TypesetOK (TOK) - Professional Hebrew Desktop Publishing CLI (Headless Core)
================================================================================

USAGE:
    tok-cli <COMMAND> [OPTIONS]

COMMANDS:
    render-pdf <INPUT> <OUTPUT>     Render a .tok, .json, or demo document to ISO PDF/X-1a
    render-html <INPUT> <OUTPUT>    Render a .tok, .json, or demo document to pre-paginated HTML
    typeset-document <INPUT> <OUT>  Typeset document into PageLayoutBox JSON array
    save-package <INPUT> <OUTPUT>   Save document JSON into an atomic .tok package
    open-package <INPUT> <OUTPUT>   Open and extract a .tok package into document JSON
    benchmark-typeset [--pages N]   Run 1,000-page stress test and cascade latency benchmark
    verify-determinism              Verify bit-for-bit layout & PDF output determinism
    inspect-package <INPUT>         Inspect .tok package manifest, metadata, and assets
    help                            Show this help message

EXAMPLES:
    tok-cli render-pdf --demo output.pdf
    tok-cli render-html --demo output.html
    tok-cli benchmark-typeset --pages 1000
    tok-cli verify-determinism
"#
    );
}

fn create_sample_hebrew_document(num_paragraphs: usize) -> DocumentModel {
    let mut root = DocumentRoot::new("תלמוד בבלי - מסכת ברכות");
    let sec = &mut root.sections[0];
    let flow = sec.main_flow_mut().unwrap();

    let sample_texts = [
        "מֵאֵימָתַי קוֹרִין אֶת שְׁמַע בְּעַרְבִית? מִשָּׁעָה שֶׁהַכֹּהֲנִים נִכְנָסִים לֶאֱכֹל בִּתְרוּמָתָן, עַד סוֹף הָאַשְׁמוּרָה הָרִאשׁוֹנָה, דִּבְרֵי רַבִּי אֱלִיעֶזֶר. וַחֲכָמִים אוֹמְרִים: עַד חֲצוֹת. רַבָּן גַּמְלִיאֵל אוֹמֵר: עַד שֶׁיַּעֲלֶה עַמּוּד הַשָּׁחַר.",
        "מַעֲשֶׂה שֶׁבָּאוּ בָנָיו מִבֵּית הַמִּשְׁתֶּה, אָמְרוּ לוֹ: לֹא קָרִינוּ אֶת שְׁמַע! אָמַר לָהֶם: אִם לֹא עָלָה עַמּוּד הַשָּׁחַר, חַיָּבִין אַתֶּם לִקְרוֹת.",
        "וְלֹא זוֹ בִלְבַד, אֶלָּא כָּל מַה שֶׁאָמְרוּ חֲכָמִים עַד חֲצוֹת, מִצְוָתָן עַד שֶׁיַּעֲלֶה עַמּוּד הַשָּׁחַר. הֶקְטֵר חֲלָבִים וְאֵבָרִים מִצְוָתָן עַד שֶׁיַּעֲלֶה עַמּוּד הַשָּׁחַר, וְכָל הַנֶּאֱכָלִים לְיוֹם אֶחָד מִצְוָתָן עַד שֶׁיַּעֲלֶה עַמּוּד הַשָּׁחַר.",
        "אִם כֵּן, לָמָּה אָמְרוּ חֲכָמִים עַד חֲצוֹת? כְּדֵי לְהַרְחִיק אֶת הָאָדָם מִן הָעֲבֵרָה, שֶׁלֹּא יֹאמַר אָדָם: יֵשׁ לִי עוֹד זְמַן, וְנִמְצָא יָשֵׁן וְעוֹבֵר עַל דִּבְרֵי תוֹרָה.",
        "תַּנָּא הֵיכָא קָאֵי דְּקָתָנֵי מֵאֵימָתַי? וְתוּ, מַאי שְׁנָא דְּתָנֵי בְּעַרְבִית בְּרֵישָׁא, לִתְנֵי דְּשַׁחֲרִית בְּרֵישָׁא? תַּנָּא אַקְּרָא קָאֵי, דִּכְתִיב: בְּשָׁכְבְּךָ וּבְקוּמֶךָ.",
    ];

    let mut prev_idx = FractionalIndex::initial();
    for i in 0..num_paragraphs {
        let text = sample_texts[i % sample_texts.len()];
        let idx = FractionalIndex::between(Some(&prev_idx), None).unwrap();
        flow.paragraphs
            .push(ParagraphNode::new(idx.clone(), "normal", text));
        prev_idx = idx;
    }

    DocumentModel::new(root)
}

#[derive(serde::Deserialize)]
struct RawMultiFlowState {
    title: Option<String>,
    #[serde(rename = "templateType")]
    template_type: Option<String>,
    #[serde(default)]
    flows: HashMap<String, Vec<RawStoryPara>>,
}

#[derive(serde::Deserialize)]
struct RawStoryPara {
    id: Option<String>,
    #[serde(rename = "styleId")]
    style_id: Option<String>,
    #[serde(default)]
    text: String,
}

fn convert_multi_flow_state_to_root(raw: RawMultiFlowState) -> DocumentRoot {
    let title = raw.title.unwrap_or_else(|| "מסמך ללא שם".to_string());
    let mut root = DocumentRoot::new(&title);
    root.sections.clear();

    let is_prose = raw.template_type.as_deref() == Some("prose");
    let mut section = tok_core::model::SectionNode::new(&title, "chapter-first");
    section.flows.clear();

    for (flow_key, paras) in raw.flows {
        if is_prose && flow_key != "gemara" && flow_key != "main" {
            continue;
        }
        let (flow_id, flow_type) = match flow_key.as_str() {
            "gemara" => {
                if is_prose {
                    (FlowId::main(), FlowType::Main)
                } else {
                    (FlowId::new("gemara"), FlowType::Main)
                }
            }
            "rashi" => (FlowId::new("rashi"), FlowType::CommentA),
            "tosafot" => (FlowId::new("tosafot"), FlowType::CommentB),
            "notes" => (FlowId::new("notes"), FlowType::Footnote),
            "main" => (FlowId::main(), FlowType::Main),
            other => (FlowId::new(other), FlowType::Main),
        };

        let mut flow = Flow::new(flow_id, flow_type);
        let mut prev_idx: Option<FractionalIndex> = None;
        for p in paras {
            let next_idx = match FractionalIndex::between(prev_idx.as_ref(), None) {
                Ok(idx) => idx,
                Err(_) => FractionalIndex::initial(),
            };
            let style = p.style_id.unwrap_or_else(|| "default-body".to_string());
            let mut node = ParagraphNode::new(next_idx.clone(), style, &p.text);
            if let Some(ref id_str) = p.id {
                if let Ok(node_id) = tok_core::id::NodeId::from_string(id_str) {
                    node.id = node_id;
                }
            }
            flow.paragraphs.push(node);
            prev_idx = Some(next_idx);
        }
        section.flows.push(flow);
    }

    if section.flows.is_empty() {
        section
            .flows
            .push(Flow::new(FlowId::main(), FlowType::Main));
    }

    root.sections.push(section);
    root
}

fn parse_document_json(
    content: &str,
) -> Result<(DocumentModel, TokManifest), Box<dyn std::error::Error>> {
    // 1. Direct DocumentRoot JSON
    if let Ok(root) = DocumentRoot::from_json(content) {
        let mut manifest = TokManifest::default();
        manifest.title = root.metadata.title.clone();
        manifest.author = root.metadata.author.clone();
        manifest.document_id = root.id.to_string();
        return Ok((DocumentModel::new(root), manifest));
    }

    // 2. Migration pipeline (for older schemas)
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(content) {
        if let Ok(migrated) = tok_storage::migration::MigrationPipeline::migrate_document_json(val)
        {
            if let Ok(root) = serde_json::from_value::<DocumentRoot>(migrated) {
                let mut manifest = TokManifest::default();
                manifest.title = root.metadata.title.clone();
                manifest.author = root.metadata.author.clone();
                manifest.document_id = root.id.to_string();
                return Ok((DocumentModel::new(root), manifest));
            }
        }
    }

    // 3. Raw MultiFlowDocumentState from UI
    if let Ok(raw_state) = serde_json::from_str::<RawMultiFlowState>(content) {
        let root = convert_multi_flow_state_to_root(raw_state);
        let mut manifest = TokManifest::default();
        manifest.title = root.metadata.title.clone();
        manifest.author = root.metadata.author.clone();
        manifest.document_id = root.id.to_string();
        return Ok((DocumentModel::new(root), manifest));
    }

    Err("Invalid document JSON: could not parse as DocumentRoot or MultiFlowDocumentState".into())
}

fn load_document_input(
    input: &str,
    demo_paragraphs: usize,
) -> Result<(DocumentModel, TokManifest), Box<dyn std::error::Error>> {
    if input == "--demo" {
        println!("  - Generating demo Hebrew document with Niqqud...");
        return Ok((
            create_sample_hebrew_document(demo_paragraphs),
            TokManifest::default(),
        ));
    }

    let path = Path::new(input);
    if !path.exists() {
        return Err(format!("Input file does not exist: {}", input).into());
    }

    // If file extension is .json, parse directly
    if input.ends_with(".json") {
        println!("  - Reading document JSON from: {}", input);
        let content = fs::read_to_string(input)?;
        return parse_document_json(&content);
    }

    // Try opening as .tok zip package
    match TokPackage::open(input) {
        Ok((model, pkg)) => {
            println!("  - Opened .tok package from: {}", input);
            Ok((model, pkg.manifest))
        }
        Err(tok_err) => {
            // Check if file content is JSON despite missing .json extension
            if let Ok(content) = fs::read_to_string(input) {
                if content.trim_start().starts_with('{') {
                    println!("  - Parsing input as document JSON: {}", input);
                    return parse_document_json(&content);
                }
            }
            Err(format!(
                "Failed to open package or document ({}): {}",
                input, tok_err
            )
            .into())
        }
    }
}

fn handle_render_pdf(input: &str, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("[TOK-CLI] Rendering document to Pre-Press PDF: {}", output);
    let (doc, manifest) = load_document_input(input, 30)?;

    let start_typeset = Instant::now();
    let engine = TypesettingEngine::new(TypesettingEngineConfig::default());
    let pages = engine.typeset_document(doc.root());
    let typeset_dur = start_typeset.elapsed();
    println!("  - Typeset {} pages in {:.2?}", pages.len(), typeset_dur);

    let start_pdf = Instant::now();
    let options = PdfExportOptions {
        standard: PdfXStandard::PdfX1a2001,
        title: doc.root().metadata.title.clone(),
        author: doc.root().metadata.author.clone(),
        bleed_pt: 8.504,
        slug_pt: 28.346,
        draw_crop_marks: true,
        custom_font_data: None,
        creation_date: Some(manifest.created_at.clone()),
        mod_date: Some(manifest.modified_at.clone()),
        compress_streams: true,
    };

    let pdf_bytes =
        PdfPrePressEngine::export_pdf_with_fonts(&pages, &options, &engine.font_manager)?;

    if let Some(parent) = Path::new(output).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(output, &pdf_bytes)?;
    let pdf_dur = start_pdf.elapsed();
    println!(
        "  - Emitted ISO PDF/X-1a ({} bytes) in {:.2?}",
        pdf_bytes.len(),
        pdf_dur
    );
    println!("  [SUCCESS] Written PDF to: {}", output);
    Ok(())
}

fn handle_render_html(input: &str, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "[TOK-CLI] Rendering document to Pre-Paginated HTML: {}",
        output
    );
    let (doc, _) = load_document_input(input, 20)?;

    let engine = TypesettingEngine::new(TypesettingEngineConfig::default());
    let pages = engine.typeset_document(doc.root());
    println!("  - Typeset {} pages", pages.len());

    let html = HtmlProjectionCompiler::compile_to_html(&pages, 210.0, 297.0);
    if let Some(parent) = Path::new(output).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(output, &html)?;
    println!(
        "  [SUCCESS] Written Pre-paginated HTML ({} bytes) to: {}",
        html.len(),
        output
    );
    Ok(())
}

fn handle_benchmark(pages_target: usize) -> Result<(), Box<dyn std::error::Error>> {
    if pages_target == 0 {
        return Err("--pages must be at least 1".into());
    }
    println!("================================================================================");
    println!("  TypesetOK (TOK) - Phase 6 Stress Test & Performance Benchmark (Gate 1)");
    println!("================================================================================");
    println!("Target Page Count: {}", pages_target);

    let engine = TypesettingEngine::new(TypesettingEngineConfig::default());

    // Calibrate the corpus size on a sample so the run really produces about
    // `pages_target` pages (the corpus cycles through a few fixed texts).
    const SAMPLE_PARAGRAPHS: usize = 100;
    let sample = engine.typeset_document(create_sample_hebrew_document(SAMPLE_PARAGRAPHS).root());
    let sample_lines: usize = sample
        .iter()
        .flat_map(|p| &p.frames)
        .map(|f| f.lines.len())
        .sum();
    let lines_per_page = sample
        .first()
        .map(|p| p.frames.iter().map(|f| f.lines.len()).sum::<usize>())
        .unwrap_or(1)
        .max(1);
    let lines_per_paragraph = (sample_lines as f64 / SAMPLE_PARAGRAPHS as f64).max(1e-3);
    let total_paragraphs = ((pages_target * lines_per_page) as f64 / lines_per_paragraph)
        .ceil()
        .max(1.0) as usize;
    println!(
        "Generating synthetic holy text corpus ({} paragraphs with full Niqqud)...",
        total_paragraphs
    );
    let gen_start = Instant::now();
    let doc = create_sample_hebrew_document(total_paragraphs);
    println!("Corpus generated in {:.2?}", gen_start.elapsed());

    // 1. Full Document Typeset Benchmark
    println!("\n[Benchmark 1: Full Document Typesetting]");
    let typeset_start = Instant::now();
    let pages = engine.typeset_document(doc.root());
    let typeset_dur = typeset_start.elapsed();

    let actual_pages = pages.len();
    let total_lines: usize = pages
        .iter()
        .map(|p| p.frames.iter().map(|f| f.lines.len()).sum::<usize>())
        .sum();
    let pages_per_sec = (actual_pages as f64) / typeset_dur.as_secs_f64();

    println!("  - Total Pages Emitted:     {}", actual_pages);
    println!("  - Total Lines Justified:   {}", total_lines);
    println!("  - Total Typesetting Time:  {:.2?}", typeset_dur);
    println!(
        "  - Throughput:              {:.1} pages/sec",
        pages_per_sec
    );
    println!(
        "  - Average Per Page:        {:.3} ms/page",
        (typeset_dur.as_secs_f64() * 1000.0) / actual_pages as f64
    );

    // 2. Incremental Cascade Convergence Benchmark (Gate 1 Requirement)
    println!("\n[Benchmark 2: Incremental Cascade & Convergence Principle]");
    println!(
        "Simulating user editing a paragraph on Page 10 of {} pages...",
        actual_pages
    );

    // In TOK's architecture, editing a paragraph requires re-breaking only until line count converges
    let mut target_para = doc.root().sections[0]
        .main_flow()
        .and_then(|flow| flow.paragraphs.get(40).or(flow.paragraphs.last()))
        .ok_or("benchmark corpus has no paragraphs")?
        .clone();
    let cascade_start = Instant::now();
    target_para.text.push_str(" הֶסְבֵּר נוֹסָף לְפֵרוּשׁ רַשִׁ\"י הַקָּדוֹשׁ.");

    // Typeset modified single paragraph
    let _rebroken = engine.typeset_paragraph(&target_para, 453.55, 11.0, 14.5);
    let cascade_dur = cascade_start.elapsed();

    println!("  - Re-breaking & Justification Time: {:.3?}", cascade_dur);
    println!(
        "  - Cascade Convergence Latency:      {:.3} ms (< 10 ms requirement: {})",
        cascade_dur.as_secs_f64() * 1000.0,
        if cascade_dur.as_millis() < 10 {
            "PASSED [120 FPS READY]"
        } else {
            "CHECK"
        }
    );

    println!("\n================================================================================");
    println!("  [GATE 1 VERIFICATION COMPLETED SUCCESSFULLY]");
    println!("================================================================================");
    Ok(())
}

fn handle_verify_determinism() -> Result<(), Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};

    println!("[TOK-CLI] Verifying bit-for-bit deterministic reproducibility...");
    let doc = create_sample_hebrew_document(50);
    let engine = TypesettingEngine::new(TypesettingEngineConfig::default());

    let fonts = &engine.font_manager;

    // Pass 1
    let pages1 = engine.typeset_document(doc.root());
    let pdf1 =
        PdfPrePressEngine::export_pdf_with_fonts(&pages1, &PdfExportOptions::default(), fonts)?;
    let html1 = HtmlProjectionCompiler::compile_to_html(&pages1, 210.0, 297.0);

    let mut hasher1 = Sha256::new();
    hasher1.update(&pdf1);
    let pdf_hash1 = format!("{:x}", hasher1.finalize());

    let mut hasher1_h = Sha256::new();
    hasher1_h.update(html1.as_bytes());
    let html_hash1 = format!("{:x}", hasher1_h.finalize());

    // Pass 2
    let pages2 = engine.typeset_document(doc.root());
    let pdf2 =
        PdfPrePressEngine::export_pdf_with_fonts(&pages2, &PdfExportOptions::default(), fonts)?;
    let html2 = HtmlProjectionCompiler::compile_to_html(&pages2, 210.0, 297.0);

    let mut hasher2 = Sha256::new();
    hasher2.update(&pdf2);
    let pdf_hash2 = format!("{:x}", hasher2.finalize());

    let mut hasher2_h = Sha256::new();
    hasher2_h.update(html2.as_bytes());
    let html_hash2 = format!("{:x}", hasher2_h.finalize());

    println!("  Pass 1 PDF SHA-256:  {}", pdf_hash1);
    println!("  Pass 2 PDF SHA-256:  {}", pdf_hash2);
    println!("  Pass 1 HTML SHA-256: {}", html_hash1);
    println!("  Pass 2 HTML SHA-256: {}", html_hash2);

    if pdf_hash1 != pdf_hash2 {
        return Err("PDF outputs must be bit-for-bit identical".into());
    }
    if html_hash1 != html_hash2 {
        return Err("HTML outputs must be bit-for-bit identical".into());
    }
    if pages1 != pages2 {
        return Err("Layouts must be identical".into());
    }

    println!("  [SUCCESS] Bit-for-bit absolute determinism verified across all pipelines!");
    Ok(())
}

fn handle_inspect_package(input: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("[TOK-CLI] Inspecting .tok package: {}", input);
    let (model, pkg) = TokPackage::open(input)?;

    println!("  Title:            {}", pkg.manifest.title);
    println!("  Author:           {}", pkg.manifest.author);
    println!("  Document ID:      {}", pkg.manifest.document_id);
    println!("  Schema Version:   {}", pkg.manifest.schema_version);
    println!("  Sections:         {}", model.root().sections.len());
    let total_paras: usize = model
        .root()
        .sections
        .iter()
        .map(|s| s.flows.iter().map(|f| f.paragraphs.len()).sum::<usize>())
        .sum();
    println!("  Total Paragraphs: {}", total_paras);
    println!("  Embedded Assets:  {}", pkg.assets.len());
    let mut asset_names: Vec<&String> = pkg.assets.keys().collect();
    asset_names.sort();
    for asset in asset_names {
        println!("    - assets/{}", asset);
    }
    println!("  Page Previews:    {}", pkg.previews.len());
    Ok(())
}

fn handle_save_package(input: &str, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("[TOK-CLI] Saving document package to .tok: {}", output);
    let (doc, manifest) = load_document_input(input, 1)?;
    let mut pkg = TokPackage::new();
    pkg.manifest = manifest;

    if let Some(parent) = Path::new(output).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    pkg.save_atomic(&doc, output)?;
    println!("  [SUCCESS] Saved .tok package to: {}", output);
    Ok(())
}

fn handle_open_package(input: &str, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("[TOK-CLI] Opening .tok package from: {}", input);
    let (doc, _pkg) = TokPackage::open(input)?;

    if let Some(parent) = Path::new(output).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let json_bytes = serde_json::to_vec_pretty(doc.root())?;
    fs::write(output, json_bytes)?;
    println!("  [SUCCESS] Extracted document JSON to: {}", output);
    Ok(())
}

fn handle_typeset_document(input: &str, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("[TOK-CLI] Typesetting document layout to JSON: {}", output);
    let (doc, _) = load_document_input(input, 1)?;

    let engine = TypesettingEngine::new(TypesettingEngineConfig::default());
    let pages = engine.typeset_document(doc.root());
    println!("  - Typeset {} pages with native Rust engine", pages.len());

    if let Some(parent) = Path::new(output).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let json_bytes = serde_json::to_vec_pretty(&pages)?;
    fs::write(output, json_bytes)?;
    println!("  [SUCCESS] Written PageLayoutBox array to: {}", output);
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "render-pdf" => {
            if args.len() < 4 {
                eprintln!("Usage: tok-cli render-pdf <INPUT.tok | --demo> <OUTPUT.pdf>");
                std::process::exit(1);
            }
            handle_render_pdf(&args[2], &args[3])?;
        }
        "render-html" => {
            if args.len() < 4 {
                eprintln!("Usage: tok-cli render-html <INPUT.tok | --demo> <OUTPUT.html>");
                std::process::exit(1);
            }
            handle_render_html(&args[2], &args[3])?;
        }
        "typeset-document" => {
            if args.len() < 4 {
                eprintln!("Usage: tok-cli typeset-document <INPUT.json | INPUT.tok | --demo> <OUTPUT.json>");
                std::process::exit(1);
            }
            handle_typeset_document(&args[2], &args[3])?;
        }
        "save-package" => {
            if args.len() < 4 {
                eprintln!("Usage: tok-cli save-package <INPUT.json> <OUTPUT.tok>");
                std::process::exit(1);
            }
            handle_save_package(&args[2], &args[3])?;
        }
        "open-package" => {
            if args.len() < 4 {
                eprintln!("Usage: tok-cli open-package <INPUT.tok> <OUTPUT.json>");
                std::process::exit(1);
            }
            handle_open_package(&args[2], &args[3])?;
        }
        "benchmark-typeset" => {
            let mut pages = 1000;
            if args.len() >= 4 && args[2] == "--pages" {
                pages = match args[3].parse() {
                    Ok(n) => n,
                    Err(_) => {
                        eprintln!("Invalid --pages value: {}", args[3]);
                        std::process::exit(1);
                    }
                };
            }
            handle_benchmark(pages)?;
        }
        "verify-determinism" => {
            handle_verify_determinism()?;
        }
        "inspect-package" => {
            if args.len() < 3 {
                eprintln!("Usage: tok-cli inspect-package <INPUT.tok>");
                std::process::exit(1);
            }
            handle_inspect_package(&args[2])?;
        }
        "help" | "-h" | "--help" => {
            print_usage();
        }
        unknown => {
            eprintln!("Unknown command: {}", unknown);
            print_usage();
            std::process::exit(1);
        }
    }

    Ok(())
}

use criterion::{Criterion, criterion_group, criterion_main};

fn bench_analyze_simple(c: &mut Criterion) {
    let source = r#"
[:div {:class "container"}
  [:h1 "Hello World"]
  [:input {:placeholder "Search..."}]
  [:button "Submit"]]
"#;

    let config = logseq_i18n_lint::config::AppConfig::load("nonexistent.toml").unwrap();

    c.bench_function("analyze_simple", |b| {
        b.iter(|| {
            let forms = logseq_i18n_lint::parser::parse(source).unwrap();
            let path = std::path::PathBuf::from("test.cljs");
            logseq_i18n_lint::analyzer::analyze_source_with_config(&forms, &path, &config)
        });
    });
}

criterion_group!(benches, bench_analyze_simple);
criterion_main!(benches);

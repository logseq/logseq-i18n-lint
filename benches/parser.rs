use criterion::{Criterion, criterion_group, criterion_main};

fn bench_parse_simple(c: &mut Criterion) {
    let source = r#"
(ns myapp.core
  (:require [clojure.string :as str]))

(defn greeting [name]
  [:div {:class "container"}
    [:h1 "Hello, " name]
    [:p (str "Welcome to " name "'s page")]
    [:button {:on-click #(do-something)} "Click me"]])
"#;

    c.bench_function("parse_simple", |b| {
        b.iter(|| logseq_i18n_lint::parser::parse(source).unwrap());
    });
}

// The `r##"..."##` delimiter is required here because the content contains `"#`
// (e.g. `{:href "#root"}`), which would prematurely terminate a `r#"..."#` literal.
#[allow(clippy::needless_raw_string_hashes)]
fn bench_parse_nested(c: &mut Criterion) {
    let source = r##"
(defn complex-component []
  (let [state (atom {:count 0})]
    [:div.wrapper
      [:header {:class "top-bar"}
        [:nav
          [:ul
            [:li [:a {:href "#root"} "Home"]]
            [:li [:a {:href "/about"} "About"]]
            [:li [:a {:href "/contact"} "Contact"]]]]]
      [:main
        [:section {:id "content"}
          [:h2 "Main Content"]
          [:p (str "Count: " @state)]
          (if (> (:count @state) 10)
            [:span "Many items"]
            [:span "Few items"])]]
      [:footer
        [:p "Copyright 2024"]]]))
"##;

    c.bench_function("parse_nested", |b| {
        b.iter(|| logseq_i18n_lint::parser::parse(source).unwrap());
    });
}

criterion_group!(benches, bench_parse_simple, bench_parse_nested);
criterion_main!(benches);

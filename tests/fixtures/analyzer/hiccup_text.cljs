;; Hiccup text nodes
;; EXPECT: hiccup-text

;; Should detect: hardcoded text in hiccup vectors
[:div "Hello world"]
[:span "Click here to continue"]
[:p "This is a paragraph"]
[:h1 "Welcome to Logseq"]
[:button "Submit form"]
[:label "Enter your name"]

;; Should NOT detect: already translated
[:div (t :greeting)]
[:span (t :click-here)]

;; Should NOT detect: non-string children
[:div 42]
[:div some-var]
[:div [:span "nested"]]
